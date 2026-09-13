//! Vista de recursos: que esta pesando en este equipo y que se va a hacer con
//! ello. Es la carta de presentacion del modo juego, asi que ensena las cosas
//! antes de tocarlas, no despues.

use egui::{Align, Frame, Margin, RichText, Rounding};

use super::{chip, empty_state, focus_changed, list_row};
use crate::app::App;
use crate::theme::PALETTE;

impl App {
    pub(super) fn resources_ui(&mut self, ui: &mut egui::Ui) {
        let Some(scan) = self.resources.as_ref() else {
            empty_state(
                ui,
                if self.scanning() { "Midiendo el equipo..." } else { "Sin datos todavia" },
                "Pulsa X para medir que esta consumiendo recursos",
            );
            return;
        };

        // Resumen de arriba.
        ui.horizontal(|ui| {
            if scan.memory.total > 0 {
                chip(
                    ui,
                    &format!(
                        "RAM {} / {}",
                        gm_core::util::format_bytes(scan.memory.used()),
                        gm_core::util::format_bytes(scan.memory.total)
                    ),
                    PALETTE.text_dim,
                );
            }
            chip(
                ui,
                &format!(
                    "{} procesos a degradar ({})",
                    scan.selection.targets.len(),
                    gm_core::util::format_bytes(scan.selection.total_working_set())
                ),
                PALETTE.accent,
            );
            let stoppable = scan.services.iter().filter(|report| report.enabled && report.running).count();
            chip(ui, &format!("{stoppable} servicios a detener"), PALETTE.accent);
            if !scan.elevated {
                chip(ui, "sin permisos de administrador", PALETTE.warn);
            }
            if self.scanning() {
                chip(ui, "midiendo...", PALETTE.warn);
            }
        });

        // Utilidades del aparato que se respetan pase lo que pase.
        if !scan.helpers.is_empty() {
            ui.add_space(4.0);
            ui.horizontal_wrapped(|ui| {
                ui.label(RichText::new("Se respetan:").size(15.0).color(PALETTE.text_dim));
                for helper in &scan.helpers {
                    chip(ui, &format!("{} · {}", helper.suite, helper.role.label()), PALETTE.good);
                }
            });
        }

        ui.add_space(12.0);

        let available = ui.available_width();
        let left_width = (available * 0.52).max(360.0);

        ui.horizontal_top(|ui| {
            ui.vertical(|ui| {
                ui.set_width(left_width);
                ui.label(RichText::new("Lo que mas pesa ahora").size(20.0).strong());
                ui.add_space(8.0);
                heaviest_processes(ui, self.resources.as_ref().unwrap());
            });

            ui.add_space(24.0);

            ui.vertical(|ui| {
                ui.label(RichText::new("Servicios de Windows").size(20.0).strong());
                ui.label(
                    RichText::new("Solo se ofrecen los que se pueden parar sin romper nada. Se rearrancan al salir.")
                        .size(14.0)
                        .color(PALETTE.text_dim),
                );
                ui.add_space(8.0);
                self.service_list(ui);
            });
        });
    }

    fn service_list(&mut self, ui: &mut egui::Ui) {
        let focus = self.resource_focus;
        let scroll_to_focus = focus_changed(ui.ctx(), "foco_recursos", focus);
        let Some(scan) = self.resources.as_ref() else { return };
        let mut toggled: Option<&'static str> = None;

        egui::ScrollArea::vertical().id_salt("servicios").auto_shrink([false, false]).show(ui, |ui| {
            for (position, report) in scan.services.iter().enumerate() {
                let focused = position == focus;
                let height = if focused { 76.0 } else { 56.0 };
                let response = list_row(ui, focused, height, |ui| {
                    ui.horizontal(|ui| {
                        let mark = if report.enabled { "☑" } else { "☐" };
                        let color = if report.enabled { PALETTE.accent } else { PALETTE.text_dim };
                        ui.label(RichText::new(mark).size(18.0).color(color));
                        ui.label(RichText::new(super::ellipsize(report.entry.display, 34)).size(17.0));

                        ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
                            if !report.running {
                                ui.label(RichText::new("parado").size(14.0).color(PALETTE.text_dim));
                            } else if let Some(ram) = report.ram {
                                let text = if report.shared_process {
                                    format!("≈ {} (compartido)", gm_core::util::format_bytes(ram))
                                } else {
                                    gm_core::util::format_bytes(ram)
                                };
                                ui.label(RichText::new(text).size(14.0).color(PALETTE.text_dim));
                            } else {
                                ui.label(RichText::new("en ejecucion").size(14.0).color(PALETTE.good));
                            }
                        });
                    });
                    if focused {
                        ui.label(RichText::new(report.entry.effect).size(14.0).color(PALETTE.text_dim));
                    }
                });
                if response.clicked() {
                    toggled = Some(report.entry.name);
                }
                if focused && scroll_to_focus {
                    ui.scroll_to_rect(response.rect.expand(16.0), Some(Align::Center));
                }
                ui.add_space(6.0);
            }
        });

        if let Some(name) = toggled {
            self.toggle_service(name);
        }
    }
}

fn heaviest_processes(ui: &mut egui::Ui, scan: &gm_power::SystemScan) {
    Frame::none().fill(PALETTE.surface).rounding(Rounding::same(10.0)).inner_margin(Margin::same(14.0)).show(
        ui,
        |ui| {
            egui::ScrollArea::vertical().id_salt("procesos").auto_shrink([false, false]).show(ui, |ui| {
                for process in scan.processes.iter().take(24) {
                    // Cada proceso lleva su destino escrito: degradado,
                    // respetado (y por que) o simplemente ignorado.
                    let target = scan.selection.targets.iter().find(|target| target.pid == process.pid);
                    let untouched =
                        scan.selection.untouched.iter().find(|entry| entry.name.eq_ignore_ascii_case(&process.name));

                    ui.horizontal(|ui| {
                        ui.label(RichText::new(super::ellipsize(&process.name, 26)).size(16.0));
                        ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
                            ui.label(
                                RichText::new(gm_core::util::format_bytes(process.working_set))
                                    .size(15.0)
                                    .color(PALETTE.text),
                            );
                            ui.add_space(10.0);
                            let (text, color) = match (target, untouched) {
                                (Some(target), _) if target.forced => ("siempre a segundo plano", PALETTE.accent),
                                (Some(_), _) => ("a segundo plano", PALETTE.accent),
                                (None, Some(entry)) => (entry.reason.as_str(), PALETTE.good),
                                (None, None) => ("sin tocar", PALETTE.text_dim),
                            };
                            ui.label(RichText::new(super::ellipsize(text, 30)).size(14.0).color(color));
                        });
                    });
                    ui.add_space(6.0);
                }
                if scan.processes.is_empty() {
                    ui.label(RichText::new("No se pudo leer la lista de procesos.").color(PALETTE.text_dim));
                }
            });
        },
    );
}
