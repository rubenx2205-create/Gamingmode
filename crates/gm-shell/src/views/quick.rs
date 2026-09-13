//! Panel rapido (boton Guia). Es el unico menu que debe funcionar igual de
//! bien con el shell en primer plano que con un juego a pantalla completa.

use egui::{Align, Frame, Margin, RichText, Rounding};

use super::list_row;
use crate::app::{App, ToastKind, View};
use crate::theme::PALETTE;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuickOption {
    ResumeGame,
    EndSession,
    TogglePower,
    Library,
    Catalog,
    Downloads,
    Settings,
    Exit,
}

impl QuickOption {
    fn label(self, power_engaged: bool) -> String {
        match self {
            QuickOption::ResumeGame => "Volver al juego".to_string(),
            QuickOption::EndSession => "Terminar la partida".to_string(),
            QuickOption::TogglePower => {
                if power_engaged {
                    "Desactivar modo juego (restaurar sistema)".to_string()
                } else {
                    "Activar modo juego (optimizar recursos)".to_string()
                }
            }
            QuickOption::Library => "Biblioteca".to_string(),
            QuickOption::Catalog => "Catalogo de ROMs".to_string(),
            QuickOption::Downloads => "Descargas".to_string(),
            QuickOption::Settings => "Ajustes".to_string(),
            QuickOption::Exit => "Salir del modo juego".to_string(),
        }
    }
}

impl App {
    pub fn quick_options(&self) -> Vec<QuickOption> {
        let mut options = Vec::with_capacity(8);
        if self.session.is_some() {
            options.push(QuickOption::ResumeGame);
            options.push(QuickOption::EndSession);
        }
        options.extend([
            QuickOption::TogglePower,
            QuickOption::Library,
            QuickOption::Catalog,
            QuickOption::Downloads,
            QuickOption::Settings,
            QuickOption::Exit,
        ]);
        options
    }

    pub fn run_quick_option(&mut self, choice: Option<QuickOption>, ctx: &egui::Context) {
        let Some(choice) = choice else { return };
        match choice {
            QuickOption::ResumeGame => {
                self.pop_view();
                if self.config.general.minimize_while_playing {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(true));
                }
            }
            QuickOption::EndSession => {
                if let Some(mut session) = self.session.take() {
                    session.terminate();
                    let seconds = session.elapsed_secs();
                    self.library.record_session(&session.game_id, seconds);
                    self.save_library();
                    self.mark_library_dirty();
                    self.toast(ToastKind::Info, format!("{} cerrado", session.title));
                }
                self.pop_view();
            }
            QuickOption::TogglePower => self.toggle_power(),
            QuickOption::Library => self.set_root_view(View::Library),
            QuickOption::Catalog => self.set_root_view(View::Catalog),
            QuickOption::Downloads => self.set_root_view(View::Downloads),
            QuickOption::Settings => self.set_root_view(View::Settings),
            QuickOption::Exit => self.exit(ctx),
        }
    }

    pub(super) fn quick_ui(&mut self, ui: &mut egui::Ui) {
        let options = self.quick_options();
        let focus = self.quick_focus.min(options.len().saturating_sub(1));
        let engaged = self.power_engaged;
        let busy = self.power_busy();
        let mut chosen: Option<QuickOption> = None;

        ui.horizontal_top(|ui| {
            ui.vertical(|ui| {
                ui.set_width(520.0);
                for (position, option) in options.iter().enumerate() {
                    let focused = position == focus;
                    let mut label = option.label(engaged);
                    if *option == QuickOption::TogglePower && busy {
                        label.push_str("  (trabajando...)");
                    }
                    let response = list_row(ui, focused, 56.0, |ui| {
                        ui.label(RichText::new(label).size(19.0));
                    });
                    if response.clicked() {
                        chosen = Some(*option);
                    }
                    ui.add_space(8.0);
                }
            });

            ui.add_space(30.0);
            ui.vertical(|ui| self.power_report_ui(ui));
        });

        if let Some(choice) = chosen {
            let ctx = ui.ctx().clone();
            self.quick_focus = focus;
            self.run_quick_option(Some(choice), &ctx);
        }
    }

    /// Detalle de lo que el modo juego ha tocado en el sistema.
    fn power_report_ui(&mut self, ui: &mut egui::Ui) {
        Frame::none().fill(PALETTE.surface).rounding(Rounding::same(10.0)).inner_margin(Margin::same(18.0)).show(
            ui,
            |ui| {
                ui.set_min_width(460.0);
                ui.label(RichText::new("Estado del sistema").size(20.0).strong());
                ui.add_space(10.0);

                if self.memory.total > 0 {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("RAM en uso").size(16.0).color(PALETTE.text_dim));
                        ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
                            ui.label(
                                RichText::new(format!(
                                    "{} / {}",
                                    gm_core::util::format_bytes(self.memory.used()),
                                    gm_core::util::format_bytes(self.memory.total)
                                ))
                                .size(16.0),
                            );
                        });
                    });
                    ui.add_space(6.0);
                }
                ui.horizontal(|ui| {
                    ui.label(RichText::new("Mandos").size(16.0).color(PALETTE.text_dim));
                    ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
                        ui.label(RichText::new(self.input.backend_name()).size(15.0));
                    });
                });

                ui.add_space(14.0);
                match self.last_report.as_ref() {
                    None => {
                        ui.label(
                            RichText::new("El modo juego no se ha activado en esta sesion.")
                                .size(16.0)
                                .color(PALETTE.text_dim),
                        );
                    }
                    Some(report) => {
                        ui.label(RichText::new(report.summary()).size(16.0).color(PALETTE.accent));
                        ui.add_space(8.0);
                        egui::ScrollArea::vertical().max_height(300.0).show(ui, |ui| {
                            for entry in &report.entries {
                                let (icon, color) = match entry.outcome {
                                    gm_power::Outcome::Applied => ("✔", PALETTE.good),
                                    gm_power::Outcome::Skipped => ("–", PALETTE.text_dim),
                                    gm_power::Outcome::Failed => ("✖", PALETTE.bad),
                                };
                                ui.label(
                                    RichText::new(format!("{icon} [{}] {}", entry.area, entry.message))
                                        .size(14.0)
                                        .color(color),
                                );
                            }
                        });
                        if !report.elevated {
                            ui.add_space(8.0);
                            ui.label(
                                RichText::new(
                                    "Sin permisos de administrador no se pueden parar servicios ni bajar \
                                     la prioridad de procesos de otros usuarios.",
                                )
                                .size(14.0)
                                .color(PALETTE.warn),
                            );
                        }
                    }
                }
            },
        );
    }
}
