//! Representacion de un mando, independiente del backend.

/// Mascaras de botones de XInput. Se replican aqui para que el tipo
/// `PadSnapshot` sea util tambien en las pruebas fuera de Windows.
pub mod button {
    pub const DPAD_UP: u16 = 0x0001;
    pub const DPAD_DOWN: u16 = 0x0002;
    pub const DPAD_LEFT: u16 = 0x0004;
    pub const DPAD_RIGHT: u16 = 0x0008;
    pub const START: u16 = 0x0010;
    pub const BACK: u16 = 0x0020;
    pub const LEFT_THUMB: u16 = 0x0040;
    pub const RIGHT_THUMB: u16 = 0x0080;
    pub const LEFT_SHOULDER: u16 = 0x0100;
    pub const RIGHT_SHOULDER: u16 = 0x0200;
    /// Solo lo reporta `XInputGetStateEx` (ordinal 100).
    pub const GUIDE: u16 = 0x0400;
    pub const A: u16 = 0x1000;
    pub const B: u16 = 0x2000;
    pub const X: u16 = 0x4000;
    pub const Y: u16 = 0x8000;
}

/// Estado de un mando en un instante dado, ya normalizado a -1.0..1.0.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct PadSnapshot {
    /// Contador de paquetes de XInput: si no cambia, nada se ha movido y se
    /// puede saltar el procesado entero.
    pub packet: u32,
    pub buttons: u16,
    pub left_trigger: f32,
    pub right_trigger: f32,
    pub left_x: f32,
    pub left_y: f32,
    pub right_x: f32,
    pub right_y: f32,
}

impl PadSnapshot {
    pub fn pressed(&self, mask: u16) -> bool {
        self.buttons & mask != 0
    }

    /// Hay algo fuera de reposo? Sirve para decidir si el shell sale del modo
    /// de bajo consumo.
    pub fn is_idle(&self, deadzone: f32, trigger_threshold: f32) -> bool {
        self.buttons == 0
            && self.left_trigger < trigger_threshold
            && self.right_trigger < trigger_threshold
            && magnitude(self.left_x, self.left_y) < deadzone
            && magnitude(self.right_x, self.right_y) < deadzone
    }
}

pub fn magnitude(x: f32, y: f32) -> f32 {
    (x * x + y * y).sqrt()
}

/// Zona muerta radial con reescalado: al salir de la zona muerta el valor
/// arranca en 0.0 y no da el salto brusco de una zona muerta por eje.
pub fn apply_deadzone(x: f32, y: f32, deadzone: f32) -> (f32, f32) {
    let mag = magnitude(x, y);
    if mag <= deadzone || mag == 0.0 {
        return (0.0, 0.0);
    }
    let scaled = ((mag - deadzone) / (1.0 - deadzone)).min(1.0);
    (x / mag * scaled, y / mag * scaled)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn la_zona_muerta_arranca_en_cero_y_satura_en_uno() {
        assert_eq!(apply_deadzone(0.2, 0.0, 0.35), (0.0, 0.0));
        let (x, _) = apply_deadzone(0.35001, 0.0, 0.35);
        assert!(x > 0.0 && x < 0.001, "x = {x}");
        let (x, _) = apply_deadzone(1.0, 0.0, 0.35);
        assert!((x - 1.0).abs() < 1e-5);
    }

    #[test]
    fn la_zona_muerta_es_radial_no_por_eje() {
        // 0.3 en cada eje da magnitud 0.42: en diagonal si debe registrarse.
        let (x, y) = apply_deadzone(0.3, 0.3, 0.35);
        assert!(x > 0.0 && y > 0.0);
    }
}
