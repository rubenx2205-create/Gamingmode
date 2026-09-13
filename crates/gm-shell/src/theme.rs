//! Paleta y estilo del shell. Pensado para verse a dos metros de una tele:
//! tipografia grande, contraste alto y foco imposible de perder de vista.

use egui::{Color32, FontFamily, FontId, Rounding, Stroke, TextStyle, Vec2};

pub struct Palette {
    pub bg: Color32,
    pub surface: Color32,
    pub surface_alt: Color32,
    pub accent: Color32,
    pub accent_dim: Color32,
    pub text: Color32,
    pub text_dim: Color32,
    pub good: Color32,
    pub warn: Color32,
    pub bad: Color32,
}

pub const PALETTE: Palette = Palette {
    bg: Color32::from_rgb(0x0b, 0x0e, 0x14),
    surface: Color32::from_rgb(0x15, 0x1a, 0x25),
    surface_alt: Color32::from_rgb(0x1e, 0x25, 0x33),
    accent: Color32::from_rgb(0x4c, 0xc2, 0xff),
    accent_dim: Color32::from_rgb(0x1d, 0x4e, 0x6b),
    text: Color32::from_rgb(0xe8, 0xed, 0xf5),
    text_dim: Color32::from_rgb(0x8d, 0x9a, 0xb0),
    good: Color32::from_rgb(0x5d, 0xd6, 0x8a),
    warn: Color32::from_rgb(0xf2, 0xc3, 0x4b),
    bad: Color32::from_rgb(0xf2, 0x6b, 0x6b),
};

/// Tamano de las caratulas de la rejilla.
pub const CARD: Vec2 = Vec2::new(190.0, 268.0);
pub const CARD_ROUNDING: f32 = 10.0;

pub fn install(ctx: &egui::Context, ui_scale: f32) {
    let mut style = (*ctx.style()).clone();

    style.text_styles = [
        (TextStyle::Heading, FontId::new(34.0, FontFamily::Proportional)),
        (TextStyle::Body, FontId::new(18.0, FontFamily::Proportional)),
        (TextStyle::Button, FontId::new(19.0, FontFamily::Proportional)),
        (TextStyle::Small, FontId::new(15.0, FontFamily::Proportional)),
        (TextStyle::Monospace, FontId::new(15.0, FontFamily::Monospace)),
    ]
    .into();

    let visuals = &mut style.visuals;
    visuals.dark_mode = true;
    visuals.panel_fill = PALETTE.bg;
    visuals.window_fill = PALETTE.surface;
    visuals.extreme_bg_color = PALETTE.bg;
    visuals.override_text_color = Some(PALETTE.text);
    visuals.selection.bg_fill = PALETTE.accent_dim;
    visuals.selection.stroke = Stroke::new(1.0, PALETTE.accent);
    visuals.widgets.noninteractive.bg_fill = PALETTE.surface;
    visuals.widgets.inactive.bg_fill = PALETTE.surface_alt;
    visuals.widgets.hovered.bg_fill = PALETTE.surface_alt;
    visuals.widgets.active.bg_fill = PALETTE.accent_dim;
    visuals.widgets.inactive.rounding = Rounding::same(6.0);
    visuals.widgets.hovered.rounding = Rounding::same(6.0);
    visuals.widgets.active.rounding = Rounding::same(6.0);
    visuals.window_rounding = Rounding::same(12.0);

    style.spacing.item_spacing = Vec2::new(14.0, 12.0);
    style.spacing.button_padding = Vec2::new(16.0, 10.0);
    style.spacing.scroll.bar_width = 10.0;

    ctx.set_style(style);
    ctx.set_zoom_factor(ui_scale.clamp(0.6, 2.0));
}

/// Color estable derivado de un texto: da a cada juego sin caratula un tono
/// propio en vez de una fila de rectangulos grises iguales.
pub fn color_for(text: &str) -> Color32 {
    let mut hash: u32 = 2166136261;
    for byte in text.as_bytes() {
        hash ^= *byte as u32;
        hash = hash.wrapping_mul(16777619);
    }
    let hue = (hash % 360) as f32 / 360.0;
    hsv_to_color(hue, 0.45, 0.42)
}

fn hsv_to_color(h: f32, s: f32, v: f32) -> Color32 {
    let i = (h * 6.0).floor();
    let f = h * 6.0 - i;
    let p = v * (1.0 - s);
    let q = v * (1.0 - f * s);
    let t = v * (1.0 - (1.0 - f) * s);
    let (r, g, b) = match (i as i32) % 6 {
        0 => (v, t, p),
        1 => (q, v, p),
        2 => (p, v, t),
        3 => (p, q, v),
        4 => (t, p, v),
        _ => (v, p, q),
    };
    Color32::from_rgb((r * 255.0) as u8, (g * 255.0) as u8, (b * 255.0) as u8)
}

/// Iniciales para la caratula generada (maximo dos letras).
pub fn initials(title: &str) -> String {
    title
        .split_whitespace()
        .filter(|word| word.chars().next().is_some_and(|c| c.is_alphanumeric()))
        .take(2)
        .filter_map(|word| word.chars().next())
        .flat_map(|c| c.to_uppercase())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn las_iniciales_salen_del_titulo() {
        assert_eq!(initials("Super Mario World"), "SM");
        assert_eq!(initials("Doom"), "D");
        assert_eq!(initials(""), "");
        assert_eq!(initials("- Halo"), "H");
    }

    #[test]
    fn el_color_generado_es_estable_y_distinto_por_titulo() {
        assert_eq!(color_for("Doom"), color_for("Doom"));
        assert_ne!(color_for("Doom"), color_for("Quake"));
    }
}
