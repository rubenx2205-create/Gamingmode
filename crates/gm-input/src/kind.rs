//! De que marca es el mando conectado?
//!
//! XInput normaliza cualquier mando al mismo esquema de botones (A/B/X/Y), asi
//! que no hay forma de preguntarselo a XInput directamente: un DualSense y un
//! mando de Xbox reportan exactamente los mismos campos. Para saber la marca
//! real hay que bajar a Raw Input y leer el VID (identificador de fabricante)
//! del dispositivo HID, que es justo lo que hace el resto de shells de salon
//! para mostrar los iconos correctos (✕○□△ en vez de ABXY).

/// Marca del mando conectado, a efectos de que boton ensenar en pantalla.
/// Cualquier fabricante que no sea Sony se trata como "Xbox": es el esquema de
/// referencia de XInput y el que copian casi todos los mandos genericos.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PadKind {
    Xbox,
    PlayStation,
}

/// VID de Sony Interactive Entertainment. Cubre DualShock 4 y DualSense: ambos
/// se anuncian con este mismo identificador de fabricante.
const VENDOR_SONY: u32 = 0x054C;

#[cfg(windows)]
pub use imp::detect_pad_kind;

#[cfg(not(windows))]
pub fn detect_pad_kind() -> Option<PadKind> {
    None
}

#[cfg(windows)]
mod imp {
    use std::ffi::c_void;
    use std::mem::size_of;

    use windows::Win32::Foundation::HANDLE;
    use windows::Win32::UI::Input::{
        GetRawInputDeviceInfoW, GetRawInputDeviceList, RAWINPUTDEVICELIST, RIDI_DEVICEINFO, RID_DEVICE_INFO,
        RIM_TYPEHID,
    };

    use super::{PadKind, VENDOR_SONY};

    /// Pagina y uso de USB HID para "mando de juego" (Generic Desktop /
    /// Joystick o Gamepad). Sin este filtro se colarian teclados, raton o
    /// lectores de huella, que tambien aparecen como dispositivos HID.
    const USAGE_PAGE_GENERIC_DESKTOP: u16 = 0x01;
    const USAGE_JOYSTICK: u16 = 0x04;
    const USAGE_GAMEPAD: u16 = 0x05;

    pub fn detect_pad_kind() -> Option<PadKind> {
        let devices = list_devices()?;
        let mut found_any = false;
        for device in devices {
            if device.dwType != RIM_TYPEHID {
                continue;
            }
            let Some((vendor, usage_page, usage)) = hid_identity(device.hDevice) else { continue };
            if usage_page != USAGE_PAGE_GENERIC_DESKTOP || (usage != USAGE_JOYSTICK && usage != USAGE_GAMEPAD) {
                continue;
            }
            found_any = true;
            if vendor == VENDOR_SONY {
                // Un DualSense/DualShock manda por delante de cualquier otra
                // cosa conectada: si hay uno, es con el que se esta jugando.
                return Some(PadKind::PlayStation);
            }
        }
        found_any.then_some(PadKind::Xbox)
    }

    /// Enumera los dispositivos de entrada en bruto. Devuelve `None` si la
    /// API falla; una lista vacia (sin mandos ni teclados de por medio) es un
    /// resultado valido y se devuelve como `Some(vec![])`.
    fn list_devices() -> Option<Vec<RAWINPUTDEVICELIST>> {
        unsafe {
            let mut count = 0u32;
            let size = size_of::<RAWINPUTDEVICELIST>() as u32;
            if GetRawInputDeviceList(None, &mut count, size) == u32::MAX {
                return None;
            }
            if count == 0 {
                return Some(Vec::new());
            }

            // La lista puede crecer entre la primera llamada (para contar) y
            // esta; se reserva margen y se reintenta una vez si aun asi no
            // alcanza.
            for _ in 0..2 {
                let mut buffer = vec![RAWINPUTDEVICELIST::default(); count as usize + 8];
                let mut actual = buffer.len() as u32;
                let copied = GetRawInputDeviceList(Some(buffer.as_mut_ptr()), &mut actual, size);
                if copied == u32::MAX {
                    count = actual; // demasiado pequeno: se reintenta con el nuevo tamano
                    continue;
                }
                buffer.truncate(copied as usize);
                return Some(buffer);
            }
            None
        }
    }

    /// VID y pagina/uso HID de un dispositivo de entrada, si es de tipo HID.
    fn hid_identity(handle: HANDLE) -> Option<(u32, u16, u16)> {
        unsafe {
            let mut info = RID_DEVICE_INFO { cbSize: size_of::<RID_DEVICE_INFO>() as u32, ..Default::default() };
            let mut size = info.cbSize;
            let result =
                GetRawInputDeviceInfoW(handle, RIDI_DEVICEINFO, Some(&mut info as *mut _ as *mut c_void), &mut size);
            if result == u32::MAX || info.dwType != RIM_TYPEHID {
                return None;
            }
            let hid = info.Anonymous.hid;
            Some((hid.dwVendorId, hid.usUsagePage, hid.usUsage))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn el_vid_de_sony_es_el_documentado() {
        // 0x054C es el VID publico de Sony Interactive Entertainment; si esto
        // cambia algun dia, la deteccion de PlayStation deja de funcionar.
        assert_eq!(VENDOR_SONY, 1356);
    }

    #[cfg(not(windows))]
    #[test]
    fn fuera_de_windows_no_hay_deteccion() {
        assert_eq!(detect_pad_kind(), None);
    }
}
