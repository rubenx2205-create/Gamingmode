//! Catalogo de ROMs: plataformas y entradas.

use egui::{Align, Margin, RichText, Sense};

use super::{cover_tile, empty_state, focus_changed, list_row};
use crate::app::{App, BrowserPurpose};
use crate::theme::{CARD, PALETTE};

impl App {
    pub(super) fn catalog_ui(&mut self, ui: &mut egui::Ui) {
        if let Some((id, elapsed)) = self.is_loading() {
            ui.label(
                RichText::new(format!("Cargando {}... ({:.1} s)", gm_catalog::display_name(id), elapsed.as_secs_f32()))
                    .size(20.0)
                    .color(PALETTE.accent),
            );
            ui.add_space(8.0);
            ui.spinner();
            return;
        }

        if self.catalog.dir().is_none() {
            empty_state(
                ui,
                "El catalogo no esta configurado",
                "Pulsa X y elige la carpeta con los .jsonl del repositorio Roms",
            );
            return;
        }
        if self.catalog.platforms().is_empty() {
            empty_state(ui, "No hay ningun .jsonl en esa carpeta", "Pulsa X para elegir otra carpeta");
            return;
        }

        ui.horizontal(|ui| {
            ui.label(RichText::new(self.catalog.memory_summary()).size(15.0).color(PALETTE.text_dim));
            ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
                if ui.button("Cambiar carpeta").clicked() {
                    self.open_browser(BrowserPurpose::PickCatalogDir);
                }
            });
        });
        ui.add_space(10.0);

        let spacing = ui.spacing().item_spacing.x;
        let columns = crate::nav::columns_for(ui.available_width(), CARD.x, spacing);
        self.columns = columns;
        let focus = self.platform_focus;
        let scroll_to_focus = focus_changed(ui.ctx(), "foco_catalogo", focus);
        let platforms = self.catalog.platforms();
        let mut clicked: Option<usize> = None;

        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            for (row_index, row) in platforms.chunks(columns).enumerate() {
                ui.horizontal(|ui| {
                    for (column_index, info) in row.iter().enumerate() {
                        let position = row_index * columns + column_index;
                        let (rect, response) = ui.allocate_exact_size(CARD, Sense::click());
                        let subtitle =
                            format!("{} · {}", info.family.label(), gm_core::util::format_bytes(info.size_bytes));
                        cover_tile(ui, rect, &info.display, &subtitle, position == focus);
                        if response.clicked() {
                            clicked = Some(position);
                        }
                        if position == focus && scroll_to_focus {
                            ui.scroll_to_rect(rect.expand(24.0), Some(Align::Center));
                        }
                    }
                });
                ui.add_space(6.0);
            }
            ui.add_space(20.0);
        });

        if let Some(position) = clicked {
            self.platform_focus = position;
            if let Some(info) = self.catalog.platforms().get(position) {
                let id = info.id.clone();
                self.open_platform(id);
            }
        }
    }

    pub(super) fn entries_ui(&mut self, ui: &mut egui::Ui) {
        let Some(platform) = self.open_platform.clone() else {
            empty_state(ui, "Ninguna plataforma abierta", "Vuelve atras con B");
            return;
        };

        ui.horizontal(|ui| {
            let response = ui.add(
                egui::TextEdit::singleline(&mut self.catalog_query)
                    .hint_text("Buscar en esta plataforma...")
                    .desired_width(420.0)
                    .margin(Margin::symmetric(10.0, 8.0)),
            );
            if response.changed() {
                self.refresh_entry_hits();
            }
            if self.search_active && !response.has_focus() {
                response.request_focus();
            }
            if !self.search_active && response.has_focus() {
                response.surrender_focus();
            }
            ui.add_space(14.0);
            ui.label(RichText::new(format!("{} resultados", self.entry_hits.len())).size(15.0).color(PALETTE.text_dim));
        });
        ui.add_space(10.0);

        if self.entry_hits.is_empty() {
            empty_state(ui, "Sin resultados", "Prueba con menos palabras");
            return;
        }

        let focus = self.entry_focus;
        let scroll_to_focus = focus_changed(ui.ctx(), "foco_entradas", focus);
        let Some(index) = self.catalog.get(&platform) else { return };
        let hits = &self.entry_hits;
        let mut clicked: Option<usize> = None;

        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            for (position, entry_index) in hits.iter().enumerate() {
                let Some(entry) = index.entries.get(*entry_index) else { continue };
                let focused = position == focus;
                let response = list_row(ui, focused, 58.0, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(super::ellipsize(&entry.name, 72)).size(18.0));
                        ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
                            ui.label(RichText::new(&entry.ext).size(15.0).color(PALETTE.text_dim));
                        });
                    });
                });
                if response.clicked() {
                    clicked = Some(position);
                }
                if focused && scroll_to_focus {
                    ui.scroll_to_rect(response.rect.expand(20.0), Some(Align::Center));
                }
                ui.add_space(6.0);
            }
        });

        if let Some(position) = clicked {
            self.entry_focus = position;
            self.download_focused_entry();
        }
    }
}
