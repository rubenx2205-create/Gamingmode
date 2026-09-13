//! Cola de descargas.

use egui::{Align, ProgressBar, RichText};

use super::{empty_state, list_row};
use crate::app::App;
use crate::theme::PALETTE;

impl App {
    pub(super) fn downloads_ui(&mut self, ui: &mut egui::Ui) {
        if self.downloader.items().is_empty() {
            empty_state(ui, "No hay descargas", "Elige una entrada del catalogo y pulsa A");
            return;
        }

        let focus = self.downloads_focus;
        let items = self.downloader.items();
        let mut cancel: Option<u64> = None;

        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            for (position, item) in items.iter().enumerate() {
                let progress = item.snapshot();
                let focused = position == focus;
                let response = list_row(ui, focused, 84.0, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(super::ellipsize(&item.label, 64)).size(18.0));
                        ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
                            let color = match progress.state {
                                gm_catalog::download::DownloadState::Done => PALETTE.good,
                                gm_catalog::download::DownloadState::Failed(_) => PALETTE.bad,
                                _ => PALETTE.text_dim,
                            };
                            ui.label(RichText::new(progress.describe()).size(15.0).color(color));
                        });
                    });
                    ui.add_space(6.0);
                    let bar = match progress.fraction() {
                        Some(fraction) => ProgressBar::new(fraction).desired_height(10.0),
                        None => ProgressBar::new(0.0).desired_height(10.0),
                    };
                    ui.add(bar);
                });
                if response.clicked() {
                    cancel = Some(item.id);
                }
                ui.add_space(8.0);
            }
        });

        if let Some(id) = cancel {
            self.downloader.cancel(id);
        }
    }
}
