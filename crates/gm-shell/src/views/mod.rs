//! Dibujado. Cada vista vive en su propio modulo y todas comparten la barra
//! superior, la leyenda de botones de abajo y los avisos.

mod catalog;
mod detail;
mod downloads;
mod explorer;
mod library;
mod quick;
mod resources;
mod settings;

pub use settings::SETTINGS_ROWS;

use egui::{Align, Align2, Color32, Frame, Margin, Rect, RichText, Rounding, Sense, Stroke, Vec2};

use crate::app::{App, ToastKind, View};
use crate::theme::{self, PALETTE};

impl App {
    pub fn draw(&mut self, ctx: &egui::Context) {
        self.top_bar(ctx);
        self.bottom_bar(ctx);

        egui::CentralPanel::default()
            .frame(Frame::none().fill(PALETTE.bg).inner_margin(Margin::symmetric(28.0, 18.0)))
            .show(ctx, |ui| match self.view {
                View::Library => self.library_view_ui(ui),
                View::GameDetail => self.detail_ui(ui),
                View::Catalog => self.catalog_ui(ui),
                View::CatalogEntries => self.entries_ui(ui),
                View::Downloads => self.downloads_ui(ui),
                View::Resources => self.resources_ui(ui),
                View::Settings => self.settings_ui(ui),
                View::Quick => self.quick_ui(ui),
                View::Browser => self.explorer_ui(ui),
            });

        self.toasts_ui(ctx);
    }

    fn top_bar(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::top("barra_superior")
            .exact_height(66.0)
            .frame(Frame::none().fill(PALETTE.surface).inner_margin(Margin::symmetric(28.0, 12.0)))
            .show(ctx, |ui| {
                ui.horizontal_centered(|ui| {
                    ui.label(RichText::new("MODO JUEGO").size(22.0).strong().color(PALETTE.accent));
                    ui.add_space(18.0);
                    ui.label(RichText::new(self.view_title()).size(20.0).color(PALETTE.text_dim));

                    ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
                        // Estado del optimizador.
                        let (label, color) = if self.power_busy() {
                            ("trabajando...", PALETTE.warn)
                        } else if self.power_engaged {
                            ("optimizado", PALETTE.good)
                        } else {
                            ("normal", PALETTE.text_dim)
                        };
                        chip(ui, label, color);

                        if self.memory.total > 0 {
                            let used = gm_core::util::format_bytes(self.memory.used());
                            let total = gm_core::util::format_bytes(self.memory.total);
                            let load = self.memory.used() as f32 / self.memory.total as f32;
                            let color = if load > 0.9 {
                                PALETTE.bad
                            } else if load > 0.75 {
                                PALETTE.warn
                            } else {
                                PALETTE.text_dim
                            };
                            chip(ui, &format!("RAM {used} / {total}"), color);
                        }

                        let pads = self.input.connected_pads();
                        let color = if pads > 0 { PALETTE.good } else { PALETTE.text_dim };
                        chip(ui, &format!("{pads} mando(s)"), color);

                        if let Some(session) = self.session.as_ref() {
                            chip(
                                ui,
                                &format!(
                                    "jugando: {} ({})",
                                    session.title,
                                    gm_core::util::format_playtime(session.elapsed_secs().max(60))
                                ),
                                PALETTE.accent,
                            );
                        }
                    });
                });
            });
    }

    fn bottom_bar(&mut self, ctx: &egui::Context) {
        let hints: &[(&str, &str)] = match self.view {
            View::Library => &[
                ("A", "Ver"),
                ("B", "Limpiar busqueda"),
                ("X", "Anadir .exe"),
                ("Y", "Favorito"),
                ("LB", "Orden"),
                ("RB", "Catalogo"),
                ("Select", "Buscar"),
                ("Start", "Menu"),
            ],
            View::GameDetail => &[("A", "Jugar"), ("B", "Volver"), ("X", "Quitar"), ("Y", "Favorito")],
            View::Catalog => &[("A", "Abrir"), ("B", "Volver"), ("X", "Carpeta del catalogo"), ("RB", "Descargas")],
            View::CatalogEntries => &[
                ("A", "Descargar"),
                ("B", "Volver"),
                ("X", "Anadir ROM local"),
                ("Select", "Buscar"),
                ("RB", "Descargas"),
            ],
            View::Downloads => &[("B", "Volver"), ("X", "Cancelar"), ("Y", "Limpiar terminadas")],
            View::Resources => &[
                ("A", "Marcar/desmarcar servicio"),
                ("B", "Volver"),
                ("X", "Volver a medir"),
                ("Y", "Seleccion recomendada"),
            ],
            View::Settings => &[("A/Der", "Cambiar"), ("Izq", "Atras"), ("B", "Guardar y volver")],
            View::Quick => &[("A", "Elegir"), ("B", "Cerrar")],
            View::Browser => &[("A", "Abrir"), ("B", "Subir"), ("Y", "Usar esta carpeta")],
        };

        egui::TopBottomPanel::bottom("barra_inferior")
            .exact_height(52.0)
            .frame(Frame::none().fill(PALETTE.surface).inner_margin(Margin::symmetric(28.0, 8.0)))
            .show(ctx, |ui| {
                ui.horizontal_centered(|ui| {
                    for (button, label) in hints {
                        button_hint(ui, button, label);
                        ui.add_space(14.0);
                    }
                });
            });
    }

    fn toasts_ui(&mut self, ctx: &egui::Context) {
        if self.toasts.is_empty() {
            return;
        }
        egui::Area::new(egui::Id::new("avisos"))
            .anchor(Align2::RIGHT_BOTTOM, Vec2::new(-28.0, -70.0))
            .interactable(false)
            .show(ctx, |ui| {
                for toast in &self.toasts {
                    let color = match toast.kind {
                        ToastKind::Info => PALETTE.accent,
                        ToastKind::Good => PALETTE.good,
                        ToastKind::Bad => PALETTE.bad,
                    };
                    Frame::none()
                        .fill(PALETTE.surface_alt)
                        .rounding(Rounding::same(8.0))
                        .stroke(Stroke::new(1.0, color))
                        .inner_margin(Margin::symmetric(16.0, 10.0))
                        .show(ui, |ui| {
                            ui.set_max_width(520.0);
                            ui.label(RichText::new(&toast.text).color(PALETTE.text));
                        });
                    ui.add_space(8.0);
                }
            });
    }

    fn view_title(&self) -> String {
        match self.view {
            View::Library => format!("Biblioteca ({})", self.library_view.len()),
            View::GameDetail => self.focused_game().map(|g| g.title.clone()).unwrap_or_default(),
            View::Catalog => format!("Catalogo ({} plataformas)", self.catalog.platforms().len()),
            View::CatalogEntries => {
                self.open_platform.as_deref().map(gm_catalog::display_name).unwrap_or_else(|| "Catalogo".to_string())
            }
            View::Downloads => "Descargas".to_string(),
            View::Resources => "Recursos del sistema".to_string(),
            View::Settings => "Ajustes".to_string(),
            View::Quick => "Panel rapido".to_string(),
            View::Browser => "Explorador".to_string(),
        }
    }
}

// ----------------------------------------------------------------------
// Widgets compartidos
// ----------------------------------------------------------------------

pub fn chip(ui: &mut egui::Ui, text: &str, color: Color32) {
    Frame::none()
        .fill(PALETTE.surface_alt)
        .rounding(Rounding::same(14.0))
        .inner_margin(Margin::symmetric(12.0, 5.0))
        .show(ui, |ui| {
            ui.label(RichText::new(text).size(15.0).color(color));
        });
    ui.add_space(10.0);
}

pub fn button_hint(ui: &mut egui::Ui, button: &str, label: &str) {
    Frame::none()
        .fill(PALETTE.accent_dim)
        .rounding(Rounding::same(10.0))
        .inner_margin(Margin::symmetric(9.0, 3.0))
        .show(ui, |ui| {
            ui.label(RichText::new(button).size(14.0).strong().color(PALETTE.text));
        });
    ui.add_space(6.0);
    ui.label(RichText::new(label).size(15.0).color(PALETTE.text_dim));
}

/// Recuadro de foco: el usuario tiene que saber donde esta sin pensarlo.
pub fn focus_ring(ui: &egui::Ui, rect: Rect) {
    ui.painter().rect_stroke(
        rect.expand(3.0),
        Rounding::same(theme::CARD_ROUNDING + 3.0),
        Stroke::new(3.0, PALETTE.accent),
    );
}

/// Caratula generada: color estable derivado del titulo e iniciales grandes.
pub fn cover_tile(ui: &mut egui::Ui, rect: Rect, title: &str, subtitle: &str, focused: bool) {
    let painter = ui.painter();
    painter.rect_filled(rect, Rounding::same(theme::CARD_ROUNDING), theme::color_for(title));

    // Franja inferior para el texto, legible sobre cualquier tono.
    let strip = Rect::from_min_max(egui::pos2(rect.left(), rect.bottom() - 76.0), rect.max);
    painter.rect_filled(
        strip,
        Rounding { nw: 0.0, ne: 0.0, sw: theme::CARD_ROUNDING, se: theme::CARD_ROUNDING },
        Color32::from_black_alpha(190),
    );

    painter.text(
        rect.center() - Vec2::new(0.0, 26.0),
        Align2::CENTER_CENTER,
        theme::initials(title),
        egui::FontId::proportional(58.0),
        Color32::from_white_alpha(210),
    );
    painter.text(
        egui::pos2(rect.left() + 12.0, rect.bottom() - 52.0),
        Align2::LEFT_TOP,
        ellipsize(title, 22),
        egui::FontId::proportional(17.0),
        PALETTE.text,
    );
    if !subtitle.is_empty() {
        painter.text(
            egui::pos2(rect.left() + 12.0, rect.bottom() - 28.0),
            Align2::LEFT_TOP,
            ellipsize(subtitle, 26),
            egui::FontId::proportional(14.0),
            PALETTE.text_dim,
        );
    }
    if focused {
        focus_ring(ui, rect);
    }
}

/// Fila de lista con foco, usada en catalogo, descargas y explorador.
pub fn list_row(ui: &mut egui::Ui, focused: bool, height: f32, add: impl FnOnce(&mut egui::Ui)) -> egui::Response {
    let width = ui.available_width();
    let (rect, response) = ui.allocate_exact_size(Vec2::new(width, height), Sense::click());
    let fill = if focused { PALETTE.accent_dim } else { PALETTE.surface };
    ui.painter().rect_filled(rect, Rounding::same(8.0), fill);
    if focused {
        ui.painter().rect_stroke(rect, Rounding::same(8.0), Stroke::new(2.0, PALETTE.accent));
    }
    let mut child = ui.new_child(
        egui::UiBuilder::new().max_rect(rect.shrink2(Vec2::new(16.0, 8.0))).layout(egui::Layout::top_down(Align::Min)),
    );
    add(&mut child);
    response
}

pub fn ellipsize(text: &str, max_chars: usize) -> String {
    let count = text.chars().count();
    if count <= max_chars {
        return text.to_string();
    }
    let mut out: String = text.chars().take(max_chars.saturating_sub(1)).collect();
    out.push('…');
    out
}

/// Devuelve true la primera vez que el foco cambia a `focus`, para desplazar la
/// lista solo entonces y no pelearse con la rueda del raton.
pub fn focus_changed(ctx: &egui::Context, key: &str, focus: usize) -> bool {
    let id = egui::Id::new(key);
    let previous: Option<usize> = ctx.data(|data| data.get_temp(id));
    if previous != Some(focus) {
        ctx.data_mut(|data| data.insert_temp(id, focus));
        true
    } else {
        false
    }
}

pub fn empty_state(ui: &mut egui::Ui, title: &str, hint: &str) {
    ui.add_space(80.0);
    ui.vertical_centered(|ui| {
        ui.label(RichText::new(title).size(26.0).color(PALETTE.text));
        ui.add_space(10.0);
        ui.label(RichText::new(hint).size(18.0).color(PALETTE.text_dim));
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn el_recorte_de_texto_respeta_los_caracteres_multibyte() {
        assert_eq!(ellipsize("Pokemon", 20), "Pokemon");
        assert_eq!(ellipsize("Edición Española Larga", 10), "Edición E…");
    }
}
