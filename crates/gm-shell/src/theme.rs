//! Paleta y estilo del shell. Pensado para verse a dos metros de una tele:
//! tipografia grande, contraste alto y foco imposible de perder de vista.
//!
//! Deliberadamente monocromo: blanco, negro y grises, sin acento de color de
//! ninguna marca ni lanzador. Hay una variante clara y una oscura; el estado
//! (icono, texto, orden de brillo) lleva el significado, no el tono.

use std::sync::atomic::{AtomicBool, Ordering};

use egui::{Color32, FontFamily, FontId, Rounding, Stroke, TextStyle, Vec2};

use gm_core::config::Theme as ThemeSetting;

#[derive(Debug, Clone, Copy)]
pub struct Palette {
    pub bg: Color32,
    pub surface: Color32,
    pub surface_alt: Color32,
    /// Foco, seleccion, texto destacado: el extremo de la escala (blanco en
    /// oscuro, negro en claro).
    pub accent: Color32,
    /// Relleno de la seleccion; mucho mas suave que `accent`.
    pub accent_dim: Color32,
    pub text: Color32,
    pub text_dim: Color32,
    /// Sin tono propio: se distingue de `warn`/`bad` por brillo, no por color,
    /// y siempre va acompanado de un icono (✔) que lleva el significado real.
    pub good: Color32,
    pub warn: Color32,
    /// El extremo de la escala, igual que `accent`: es lo que mas debe llamar
    /// la atencion en pantalla.
    pub bad: Color32,
}

const DARK: Palette = Palette {
    bg: Color32::from_rgb(0x0a, 0x0a, 0x0a),
    surface: Color32::from_rgb(0x16, 0x16, 0x16),
    surface_alt: Color32::from_rgb(0x23, 0x23, 0x23),
    accent: Color32::from_rgb(0xff, 0xff, 0xff),
    accent_dim: Color32::from_rgb(0x2e, 0x2e, 0x2e),
    text: Color32::from_rgb(0xf2, 0xf2, 0xf2),
    text_dim: Color32::from_rgb(0x8a, 0x8a, 0x8a),
    good: Color32::from_rgb(0xd6, 0xd6, 0xd6),
    warn: Color32::from_rgb(0x9e, 0x9e, 0x9e),
    bad: Color32::from_rgb(0xff, 0xff, 0xff),
};

const LIGHT: Palette = Palette {
    bg: Color32::from_rgb(0xfa, 0xfa, 0xfa),
    surface: Color32::from_rgb(0xef, 0xef, 0xef),
    surface_alt: Color32::from_rgb(0xe0, 0xe0, 0xe0),
    accent: Color32::from_rgb(0x00, 0x00, 0x00),
    accent_dim: Color32::from_rgb(0xd6, 0xd6, 0xd6),
    text: Color32::from_rgb(0x10, 0x10, 0x10),
    text_dim: Color32::from_rgb(0x6b, 0x6b, 0x6b),
    good: Color32::from_rgb(0x33, 0x33, 0x33),
    warn: Color32::from_rgb(0x5e, 0x5e, 0x5e),
    bad: Color32::from_rgb(0x00, 0x00, 0x00),
};

/// Modo actual, compartido por toda la interfaz. Vive fuera de `App` porque
/// las funciones de dibujado de las vistas son libres (`fn(&mut Ui, ...)`) y
/// pedirles el estado por parametro en cada una habria significado tocar
/// decenas de firmas para un unico bit de informacion.
static LIGHT_MODE: AtomicBool = AtomicBool::new(false);

pub fn set_theme(theme: ThemeSetting) {
    LIGHT_MODE.store(theme == ThemeSetting::Light, Ordering::Relaxed);
}

pub fn current_theme() -> ThemeSetting {
    if LIGHT_MODE.load(Ordering::Relaxed) {
        ThemeSetting::Light
    } else {
        ThemeSetting::Dark
    }
}

/// Paleta activa ahora mismo. Barata de llamar: es una copia de una constante.
pub fn pal() -> Palette {
    if LIGHT_MODE.load(Ordering::Relaxed) {
        LIGHT
    } else {
        DARK
    }
}

/// Tamano de las caratulas de la rejilla.
pub const CARD: Vec2 = Vec2::new(190.0, 268.0);
pub const CARD_ROUNDING: f32 = 10.0;

/// Aplica la paleta y la tipografia al estilo de egui. Se llama al arrancar y
/// otra vez cada vez que se cambia de tema, para que el cambio se note en el
/// acto sin reiniciar el shell.
pub fn install(ctx: &egui::Context, ui_scale: f32) {
    let palette = pal();
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
    visuals.dark_mode = current_theme() == ThemeSetting::Dark;
    visuals.panel_fill = palette.bg;
    visuals.window_fill = palette.surface;
    visuals.extreme_bg_color = palette.bg;
    visuals.override_text_color = Some(palette.text);
    visuals.selection.bg_fill = palette.accent_dim;
    visuals.selection.stroke = Stroke::new(1.0, palette.accent);
    visuals.widgets.noninteractive.bg_fill = palette.surface;
    visuals.widgets.inactive.bg_fill = palette.surface_alt;
    visuals.widgets.hovered.bg_fill = palette.surface_alt;
    visuals.widgets.active.bg_fill = palette.accent_dim;
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

/// Gris estable derivado de un texto: la caratula generada de dos juegos
/// distintos no se confunde, pero la paleta se mantiene monocroma. Sirve de
/// relleno mientras no hay caratula real (o si no se encuentra ninguna).
pub fn shade_for(text: &str) -> Color32 {
    let mut hash: u32 = 2166136261;
    for byte in text.as_bytes() {
        hash ^= *byte as u32;
        hash = hash.wrapping_mul(16777619);
    }
    // Una banda de grises que se distingue tanto del fondo oscuro como del
    // claro: ni se hunde en uno ni se lava en el otro.
    let level = 0x46 + (hash % 0x28) as u8;
    Color32::from_gray(level)
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
    fn el_tono_generado_es_estable_y_distinto_por_titulo() {
        assert_eq!(shade_for("Doom"), shade_for("Doom"));
        assert_ne!(shade_for("Doom"), shade_for("Quake"));
    }

    #[test]
    fn el_tono_generado_es_siempre_gris() {
        // Monocromo de verdad: sin importar el hash, r == g == b.
        for title in ["Doom", "Quake", "Halo", "Tetris", ""] {
            let color = shade_for(title);
            assert_eq!(color.r(), color.g());
            assert_eq!(color.g(), color.b());
        }
    }

    #[test]
    fn cambiar_de_tema_se_nota_al_momento() {
        set_theme(ThemeSetting::Dark);
        assert_eq!(current_theme(), ThemeSetting::Dark);
        assert_eq!(pal().bg, DARK.bg);

        set_theme(ThemeSetting::Light);
        assert_eq!(current_theme(), ThemeSetting::Light);
        assert_eq!(pal().bg, LIGHT.bg);

        // Se deja como estaba para no interferir con el resto de pruebas.
        set_theme(ThemeSetting::Dark);
    }
}
