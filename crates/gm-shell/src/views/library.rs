//! Rejilla de la biblioteca.

use egui::{Align, Margin, RichText, Sense};

use super::{cover_tile, empty_state, focus_changed};
use crate::app::{App, View};
use crate::theme::{self, CARD};

impl App {
    pub(super) fn library_view_ui(&mut self, ui: &mut egui::Ui) {
        self.library_toolbar(ui);
        ui.add_space(10.0);

        if self.library_view.is_empty() {
            let hint = if self.query.is_empty() {
                "Pulsa X (o F2) para anadir un .exe, o RB para abrir el catalogo de ROMs"
            } else {
                "Ninguna coincidencia. Pulsa B para limpiar la busqueda"
            };
            empty_state(ui, "No hay nada aqui todavia", hint);
            return;
        }

        let spacing = ui.spacing().item_spacing.x;
        let columns = crate::nav::columns_for(ui.available_width(), CARD.x, spacing);
        self.columns = columns;

        // Referencias a campos sueltos: asi el cierre no toma prestado `self`
        // entero y se puede seguir dibujando sin pelearse con el prestamo.
        // `covers` es un campo distinto de `library`, asi que pedir prestado
        // uno inmutable y el otro mutable a la vez no molesta al compilador.
        let focus = self.library_focus;
        let library = &self.library;
        let indices = &self.library_view;
        let covers = &mut self.covers;
        let mut clicked: Option<usize> = None;
        let mut to_fetch: Vec<(String, String)> = Vec::new();
        let scroll_to_focus = focus_changed(ui.ctx(), "foco_biblioteca", focus);

        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            for (row_index, row) in indices.chunks(columns).enumerate() {
                ui.horizontal(|ui| {
                    for (column_index, game_index) in row.iter().enumerate() {
                        let position = row_index * columns + column_index;
                        let Some(game) = library.games.get(*game_index) else { continue };
                        let (rect, response) = ui.allocate_exact_size(CARD, Sense::click());
                        let focused = position == focus;

                        let mut subtitle = game.collection.clone();
                        if game.favorite {
                            subtitle = format!("★ {subtitle}");
                        }
                        if !game.launch.is_available() {
                            subtitle = format!("⚠ no encontrado · {subtitle}");
                        }

                        let texture = covers.get(ui.ctx(), &game.id, game.cover.as_deref());
                        if texture.is_none() && game.cover.is_none() {
                            to_fetch.push((game.id.clone(), game.title.clone()));
                        }
                        cover_tile(ui, rect, &game.title, subtitle.trim(), focused, texture.as_ref());

                        if response.clicked() {
                            clicked = Some(position);
                        }
                        if focused && scroll_to_focus {
                            ui.scroll_to_rect(rect.expand(24.0), Some(Align::Center));
                        }
                    }
                });
                ui.add_space(6.0);
            }
            ui.add_space(20.0);
        });

        // Solo se piden las caratulas de lo que de verdad esta en pantalla:
        // el propio bucle de dibujado ya acota la lista a una pagina.
        for (game_id, title) in to_fetch {
            self.maybe_fetch_cover(&game_id, &title);
        }

        if let Some(position) = clicked {
            self.library_focus = position;
            self.push_view(View::GameDetail);
        }
    }

    fn library_toolbar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            let response = ui.add(
                egui::TextEdit::singleline(&mut self.query)
                    .hint_text("Buscar en la biblioteca...")
                    .desired_width(360.0)
                    .margin(Margin::symmetric(10.0, 8.0)),
            );
            if response.changed() {
                self.mark_library_dirty();
            }
            if self.search_active && !response.has_focus() {
                response.request_focus();
            }
            if !self.search_active && response.has_focus() {
                // El mando ha tomado el control: soltar el cursor de texto.
                response.surrender_focus();
            }

            ui.add_space(16.0);
            let sort_label = match self.sort {
                gm_core::Sort::Title => "Orden: titulo",
                gm_core::Sort::LastPlayed => "Orden: recientes",
                gm_core::Sort::PlayTime => "Orden: mas jugados",
                gm_core::Sort::Added => "Orden: anadidos",
            };
            if ui.button(sort_label).clicked() {
                self.cycle_sort();
            }

            let favorites = if self.favorites_only { "Solo favoritos ✓" } else { "Solo favoritos" };
            if ui.button(favorites).clicked() {
                self.favorites_only = !self.favorites_only;
                self.mark_library_dirty();
            }

            ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
                let stats = self.library.stats();
                ui.label(
                    RichText::new(format!(
                        "{} juegos · {} favoritos · {} h jugadas",
                        stats.get("total").copied().unwrap_or(0),
                        stats.get("favoritos").copied().unwrap_or(0),
                        stats.get("horas").copied().unwrap_or(0)
                    ))
                    .size(15.0)
                    .color(theme::pal().text_dim),
                );
            });
        });
    }
}
