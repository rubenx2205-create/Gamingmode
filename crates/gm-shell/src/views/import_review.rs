//! Revision de una importacion de carpeta: nada se anade a la biblioteca sin
//! pasar por aqui. Cada `.exe` encontrado se marca o desmarca a mano antes de
//! confirmar, exe por exe, en vez de subir la carpeta entera a ciegas.

use egui::{Align, RichText};

use super::{empty_state, focus_changed, list_row};
use crate::app::App;
use crate::theme;

impl App {
    pub(super) fn import_review_ui(&mut self, ui: &mut egui::Ui) {
        if self.import_candidates.is_empty() {
            empty_state(ui, "Nada que revisar", "Vuelve atras con B");
            return;
        }

        let marked = self.import_candidates.iter().filter(|c| c.checked).count();
        ui.label(
            RichText::new(format!("{marked} de {} marcados para anadir", self.import_candidates.len()))
                .size(18.0)
                .color(theme::pal().text_dim),
        );
        ui.add_space(10.0);

        let focus = self.import_focus;
        let scroll_to_focus = focus_changed(ui.ctx(), "foco_importar", focus);
        let candidates = &self.import_candidates;
        let mut toggled: Option<usize> = None;

        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            for (position, candidate) in candidates.iter().enumerate() {
                let focused = position == focus;
                let response = list_row(ui, focused, 56.0, |ui| {
                    ui.horizontal(|ui| {
                        let check = if candidate.checked { "☑" } else { "☐" };
                        ui.label(RichText::new(check).size(19.0).color(if candidate.checked {
                            theme::pal().good
                        } else {
                            theme::pal().text_dim
                        }));
                        ui.add_space(8.0);
                        ui.vertical(|ui| {
                            ui.label(RichText::new(&candidate.title).size(17.0));
                            let mut subtitle = candidate.path.display().to_string();
                            if candidate.installer {
                                subtitle = format!("posible instalador · {subtitle}");
                            }
                            ui.label(RichText::new(super::ellipsize(&subtitle, 90)).size(13.0).color(
                                if candidate.installer { theme::pal().warn } else { theme::pal().text_dim },
                            ));
                        });
                    });
                });
                if response.clicked() {
                    toggled = Some(position);
                }
                if focused && scroll_to_focus {
                    ui.scroll_to_rect(response.rect.expand(16.0), Some(Align::Center));
                }
                ui.add_space(4.0);
            }
        });

        if let Some(position) = toggled {
            self.import_focus = position;
            self.toggle_import_focused();
        }
    }
}
