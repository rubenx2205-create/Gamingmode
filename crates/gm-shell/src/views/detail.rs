//! Ficha de un juego.

use egui::{RichText, Sense};
use gm_input::NavAction;

use super::{action_button, cover_tile, empty_state};
use crate::app::App;
use crate::nav::button_label;
use crate::theme::{self, CARD};

impl App {
    pub(super) fn detail_ui(&mut self, ui: &mut egui::Ui) {
        let Some(game) = self.focused_game().cloned() else {
            empty_state(ui, "Sin seleccion", "Vuelve atras con B");
            return;
        };

        let mut launch = false;
        let mut remove = false;
        let mut favorite = false;
        let mut fetch_cover = false;
        let mut rename = false;
        let mut confirm_rename = false;
        let mut cancel_rename = false;

        let source = self.input_source;
        let has_key = self.config.covers.steamgrid_api_key.as_deref().is_some_and(|k| !k.trim().is_empty());
        let texture = self.covers.get(&game.id, game.cover.as_deref());
        let renaming = self.renaming;

        ui.horizontal_top(|ui| {
            let (rect, _) = ui.allocate_exact_size(CARD * 1.3, Sense::hover());
            cover_tile(ui, rect, &game.title, &game.collection, false, texture.as_ref());

            ui.add_space(28.0);
            ui.vertical(|ui| {
                if renaming {
                    ui.horizontal(|ui| {
                        let response = ui.add(
                            egui::TextEdit::singleline(&mut self.rename_draft)
                                .desired_width(420.0)
                                .font(egui::FontId::proportional(28.0)),
                        );
                        if !response.has_focus() {
                            response.request_focus();
                        }
                    });
                } else {
                    ui.label(RichText::new(&game.title).size(40.0).strong());
                }
                ui.add_space(6.0);
                let kind = match &game.launch {
                    gm_core::Launch::Executable { .. } => "Ejecutable".to_string(),
                    gm_core::Launch::Shortcut { .. } => "Atajo del sistema".to_string(),
                };
                ui.label(RichText::new(kind).size(18.0).color(theme::pal().accent));
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
                    ui.label(RichText::new("El archivo ya no existe en esa ruta.").size(17.0).color(theme::pal().bad));
                }

                ui.add_space(26.0);
                if renaming {
                    ui.horizontal(|ui| {
                        if action_button(
                            ui,
                            &format!("✔  Guardar titulo  ({})", button_label(source, NavAction::Accept)),
                            theme::pal().accent,
                        ) {
                            confirm_rename = true;
                        }
                        ui.add_space(12.0);
                        if action_button(
                            ui,
                            &format!("✕  Cancelar  ({})", button_label(source, NavAction::Back)),
                            theme::pal().surface_alt,
                        ) {
                            cancel_rename = true;
                        }
                    });
                } else {
                    ui.horizontal(|ui| {
                        if action_button(
                            ui,
                            &format!("▶  Jugar  ({})", button_label(source, NavAction::Accept)),
                            theme::pal().accent,
                        ) {
                            launch = true;
                        }
                        ui.add_space(12.0);
                        if action_button(
                            ui,
                            &format!("★  Favorito  ({})", button_label(source, NavAction::Favorite)),
                            theme::pal().surface_alt,
                        ) {
                            favorite = true;
                        }
                        ui.add_space(12.0);
                        if action_button(
                            ui,
                            &format!("✕  Quitar  ({})", button_label(source, NavAction::Context)),
                            theme::pal().surface_alt,
                        ) {
                            remove = true;
                        }
                        ui.add_space(12.0);
                        if action_button(
                            ui,
                            &format!("✎  Renombrar  ({})", button_label(source, NavAction::TabPrev)),
                            theme::pal().surface_alt,
                        ) {
                            rename = true;
                        }
                        if has_key {
                            ui.add_space(12.0);
                            let label = if game.cover.is_some() { "Volver a buscar" } else { "Buscar caratula" };
                            if action_button(
                                ui,
                                &format!("🖼  {label}  ({})", button_label(source, NavAction::Search)),
                                theme::pal().surface_alt,
                            ) {
                                fetch_cover = true;
                            }
                        }
                    });
                    if !has_key {
                        ui.add_space(10.0);
                        ui.label(
                            RichText::new(
                                "Configura una clave gratuita en Ajustes → Caratulas automaticas para buscar caratulas.",
                            )
                            .size(14.0)
                            .color(theme::pal().text_dim),
                        );
                    }
                }
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
        if fetch_cover {
            let (id, title) = (game.id.clone(), game.title.clone());
            self.fetch_cover_now(&id, &title);
        }
        if rename {
            self.begin_rename();
        }
        if confirm_rename {
            self.confirm_rename();
        }
        if cancel_rename {
            self.cancel_rename();
        }
    }
}

fn info_row(ui: &mut egui::Ui, label: &str, value: &str) {
    ui.horizontal(|ui| {
        ui.label(RichText::new(label).size(16.0).color(theme::pal().text_dim));
        ui.add_space(10.0);
        ui.label(RichText::new(super::ellipsize(value, 70)).size(16.0));
    });
    ui.add_space(4.0);
}
