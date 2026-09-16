//! Explorador de ficheros con mando.

use egui::{Align, RichText};

use super::{empty_state, focus_changed, list_row};
use crate::app::{App, BrowserPurpose};
use crate::browser::Filter;
use crate::theme;

impl App {
    pub(super) fn explorer_ui(&mut self, ui: &mut egui::Ui) {
        let purpose = self.browser_purpose;
        let Some(browser) = self.browser.as_mut() else {
            empty_state(ui, "Explorador cerrado", "Vuelve atras con B");
            return;
        };

        let title = match purpose {
            BrowserPurpose::AddExecutable => "Elige un ejecutable",
            BrowserPurpose::ImportFolder => "Elige la carpeta con tus juegos",
        };
        ui.label(RichText::new(title).size(22.0).strong());
        ui.add_space(4.0);
        ui.label(RichText::new(browser.title()).size(16.0).color(theme::pal().text_dim));
        ui.add_space(10.0);

        if let Some(error) = &browser.error {
            ui.label(RichText::new(error).size(17.0).color(theme::pal().bad));
            return;
        }
        if browser.items.is_empty() {
            empty_state(ui, "Carpeta vacia", "Pulsa B para subir un nivel");
            return;
        }

        let focus = browser.selected;
        let scroll_to_focus = focus_changed(ui.ctx(), "foco_explorador", focus);
        let filter = browser.filter;
        let items = &browser.items;
        let mut clicked: Option<usize> = None;

        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            for (position, item) in items.iter().enumerate() {
                let focused = position == focus;
                let response = list_row(ui, focused, 52.0, |ui| {
                    ui.horizontal(|ui| {
                        let icon = if item.is_dir { "📁" } else { "▶" };
                        ui.label(RichText::new(icon).size(17.0));
                        ui.label(RichText::new(super::ellipsize(&item.name, 70)).size(17.0));
                        if !item.is_dir && item.size > 0 {
                            ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
                                ui.label(
                                    RichText::new(gm_core::util::format_bytes(item.size))
                                        .size(14.0)
                                        .color(theme::pal().text_dim),
                                );
                            });
                        }
                    });
                });
                if response.clicked() {
                    clicked = Some(position);
                }
                if focused && scroll_to_focus {
                    ui.scroll_to_rect(response.rect.expand(16.0), Some(Align::Center));
                }
                ui.add_space(4.0);
            }
        });

        if filter == Filter::Directories {
            ui.add_space(8.0);
            ui.label(RichText::new("Pulsa Y para usar la carpeta actual").size(16.0).color(theme::pal().accent));
        }

        if let Some(position) = clicked {
            if let Some(browser) = self.browser.as_mut() {
                browser.selected = position;
                if let Some(path) = browser.open_selected() {
                    self.browser_result(path);
                }
            }
        }
    }
}
