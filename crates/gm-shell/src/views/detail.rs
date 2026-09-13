//! Ficha de un juego.

use egui::{RichText, Rounding, Sense, Vec2};

use super::{cover_tile, empty_state};
use crate::app::App;
use crate::theme::{CARD, PALETTE};

impl App {
    pub(super) fn detail_ui(&mut self, ui: &mut egui::Ui) {
        let Some(game) = self.focused_game().cloned() else {
            empty_state(ui, "Sin seleccion", "Vuelve atras con B");
            return;
        };

        let mut launch = false;
        let mut remove = false;
        let mut favorite = false;

        ui.horizontal_top(|ui| {
            let (rect, _) = ui.allocate_exact_size(CARD * 1.3, Sense::hover());
            cover_tile(ui, rect, &game.title, &game.collection, false);

            ui.add_space(28.0);
            ui.vertical(|ui| {
                ui.label(RichText::new(&game.title).size(40.0).strong());
                ui.add_space(6.0);
                let kind = match &game.launch {
                    gm_core::Launch::Executable { .. } => "Ejecutable".to_string(),
                    gm_core::Launch::Rom { platform, .. } => {
                        format!("ROM · {}", gm_catalog::display_name(platform))
                    }
                    gm_core::Launch::Uri { .. } => "Lanzador externo".to_string(),
                };
                ui.label(RichText::new(kind).size(18.0).color(PALETTE.accent));
                ui.add_space(16.0);

                info_row(ui, "Tiempo jugado", &gm_core::util::format_playtime(game.play_seconds));
                info_row(
                    ui,
                    "Ultima partida",
                    &match game.last_played {
                        Some(_) => "registrada".to_string(),
                        None => "nunca".to_string(),
                    },
                );
                info_row(ui, "Favorito", if game.favorite { "si" } else { "no" });
                info_row(ui, "Destino", &game.launch.target_display());

                if !game.launch.is_available() {
                    ui.add_space(10.0);
                    ui.label(RichText::new("El archivo ya no existe en esa ruta.").size(17.0).color(PALETTE.bad));
                }

                ui.add_space(26.0);
                ui.horizontal(|ui| {
                    if big_button(ui, "▶  Jugar  (A)", PALETTE.accent) {
                        launch = true;
                    }
                    ui.add_space(12.0);
                    if big_button(ui, "★  Favorito  (Y)", PALETTE.surface_alt) {
                        favorite = true;
                    }
                    ui.add_space(12.0);
                    if big_button(ui, "✕  Quitar  (X)", PALETTE.surface_alt) {
                        remove = true;
                    }
                });
            });
        });

        if launch {
            let ctx = ui.ctx().clone();
            self.launch_focused(&ctx);
        }
        if favorite {
            self.toggle_favorite();
        }
        if remove {
            self.remove_focused();
        }
    }
}

fn info_row(ui: &mut egui::Ui, label: &str, value: &str) {
    ui.horizontal(|ui| {
        ui.label(RichText::new(label).size(16.0).color(PALETTE.text_dim));
        ui.add_space(10.0);
        ui.label(RichText::new(super::ellipsize(value, 70)).size(16.0));
    });
    ui.add_space(4.0);
}

/// Boton grande y legible a distancia. El texto se pinta oscuro sobre el
/// acento y claro sobre los tonos de fondo.
fn big_button(ui: &mut egui::Ui, text: &str, fill: egui::Color32) -> bool {
    let (rect, response) = ui.allocate_exact_size(Vec2::new(210.0, 54.0), Sense::click());
    ui.painter().rect_filled(rect, Rounding::same(10.0), fill);
    let text_color = if fill == PALETTE.accent { PALETTE.bg } else { PALETTE.text };
    ui.painter().text(rect.center(), egui::Align2::CENTER_CENTER, text, egui::FontId::proportional(19.0), text_color);
    response.clicked()
}
