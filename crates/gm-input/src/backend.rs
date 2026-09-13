//! Backend de mandos. En Windows habla directamente con XInput cargando la DLL
//! a mano; en el resto de plataformas es un stub para que el workspace compile
//! y se pueda testear la logica de navegacion.

use crate::pad::PadSnapshot;

pub const MAX_PADS: usize = 4;

pub trait PadBackend: Send {
    /// Lee una ranura. `None` significa "no hay mando conectado ahi".
    fn read(&mut self, index: u32) -> Option<PadSnapshot>;
    fn set_rumble(&mut self, index: u32, low: u16, high: u16);
    /// Nombre del backend para diagnosticos en la interfaz.
    fn describe(&self) -> String;
}

#[cfg(windows)]
pub use windows_xinput::XInputBackend as PlatformBackend;

#[cfg(not(windows))]
pub use stub::StubBackend as PlatformBackend;

#[cfg(not(windows))]
mod stub {
    use super::*;

    /// Sin mandos: la navegacion por teclado sigue funcionando.
    pub struct StubBackend;

    impl StubBackend {
        pub fn load(_capture_guide: bool) -> Option<Self> {
            Some(StubBackend)
        }

        pub fn guide_supported(&self) -> bool {
            false
        }
    }

    impl PadBackend for StubBackend {
        fn read(&mut self, _index: u32) -> Option<PadSnapshot> {
            None
        }
        fn set_rumble(&mut self, _index: u32, _low: u16, _high: u16) {}
        fn describe(&self) -> String {
            "sin backend de mandos (no es Windows)".to_string()
        }
    }
}

#[cfg(windows)]
mod windows_xinput {
    use super::*;
    use windows::core::{s, PCSTR, PCWSTR};
    use windows::Win32::System::LibraryLoader::{GetProcAddress, LoadLibraryW};

    const ERROR_SUCCESS: u32 = 0;
    /// Ordinal no documentado de `XInputGetStateEx`, la unica via para leer el
    /// boton central del mando; es la via que usan todos los shells de salon.
    const ORDINAL_GET_STATE_EX: usize = 100;

    #[repr(C)]
    #[derive(Default, Clone, Copy)]
    struct XInputGamepad {
        buttons: u16,
        left_trigger: u8,
        right_trigger: u8,
        thumb_lx: i16,
        thumb_ly: i16,
        thumb_rx: i16,
        thumb_ry: i16,
    }

    #[repr(C)]
    #[derive(Default, Clone, Copy)]
    struct XInputStateRaw {
        packet_number: u32,
        gamepad: XInputGamepad,
    }

    #[repr(C)]
    #[derive(Default, Clone, Copy)]
    struct XInputVibration {
        left_motor: u16,
        right_motor: u16,
    }

    type GetStateFn = unsafe extern "system" fn(u32, *mut XInputStateRaw) -> u32;
    type SetStateFn = unsafe extern "system" fn(u32, *mut XInputVibration) -> u32;

    pub struct XInputBackend {
        get_state: GetStateFn,
        set_state: Option<SetStateFn>,
        dll: &'static str,
        guide: bool,
    }

    impl XInputBackend {
        /// Recorre las versiones de XInput de mas nueva a mas vieja. 1_4 viene
        /// con Windows 8+, 1_3 con el runtime de DirectX y 9_1_0 esta en todas
        /// partes pero no expone ni el boton Guia ni los gatillos completos.
        pub fn load(capture_guide: bool) -> Option<Self> {
            for dll in ["xinput1_4.dll", "xinput1_3.dll", "xinput9_1_0.dll"] {
                let wide: Vec<u16> = dll.encode_utf16().chain(std::iter::once(0)).collect();
                let module = match unsafe { LoadLibraryW(PCWSTR(wide.as_ptr())) } {
                    Ok(m) if !m.is_invalid() => m,
                    _ => continue,
                };

                let mut guide = false;
                let mut addr = None;
                if capture_guide {
                    // GetProcAddress acepta un ordinal en el campo del nombre
                    // cuando el puntero cabe en 16 bits.
                    addr = unsafe { GetProcAddress(module, PCSTR(ORDINAL_GET_STATE_EX as *const u8)) };
                    guide = addr.is_some();
                }
                if addr.is_none() {
                    addr = unsafe { GetProcAddress(module, s!("XInputGetState")) };
                }
                let Some(addr) = addr else { continue };

                let get_state: GetStateFn = unsafe { std::mem::transmute(addr) };
                let set_state: Option<SetStateFn> = unsafe { GetProcAddress(module, s!("XInputSetState")) }
                    .map(|p| unsafe { std::mem::transmute::<_, SetStateFn>(p) });

                log::info!("XInput cargado desde {dll} (boton Guia: {guide})");
                return Some(Self { get_state, set_state, dll, guide });
            }
            log::warn!("no se encontro ninguna DLL de XInput; solo habra teclado");
            None
        }

        pub fn guide_supported(&self) -> bool {
            self.guide
        }
    }

    fn axis(value: i16) -> f32 {
        // i16::MIN no tiene positivo simetrico; se recorta para no pasar de -1.
        (value as f32 / 32767.0).clamp(-1.0, 1.0)
    }

    impl PadBackend for XInputBackend {
        fn read(&mut self, index: u32) -> Option<PadSnapshot> {
            let mut raw = XInputStateRaw::default();
            let result = unsafe { (self.get_state)(index, &mut raw) };
            if result != ERROR_SUCCESS {
                return None;
            }
            Some(PadSnapshot {
                packet: raw.packet_number,
                buttons: raw.gamepad.buttons,
                left_trigger: raw.gamepad.left_trigger as f32 / 255.0,
                right_trigger: raw.gamepad.right_trigger as f32 / 255.0,
                left_x: axis(raw.gamepad.thumb_lx),
                left_y: axis(raw.gamepad.thumb_ly),
                right_x: axis(raw.gamepad.thumb_rx),
                right_y: axis(raw.gamepad.thumb_ry),
            })
        }

        fn set_rumble(&mut self, index: u32, low: u16, high: u16) {
            if let Some(set_state) = self.set_state {
                let mut vibration = XInputVibration { left_motor: low, right_motor: high };
                unsafe {
                    let _ = set_state(index, &mut vibration);
                }
            }
        }

        fn describe(&self) -> String {
            format!("XInput ({}{})", self.dll, if self.guide { ", con boton Guia" } else { "" })
        }
    }
}
