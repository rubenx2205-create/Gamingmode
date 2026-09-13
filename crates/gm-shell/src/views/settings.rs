//! Ajustes navegables con mando: una lista de filas donde izquierda/derecha
//! cambia el valor. Nada de dialogos ni de teclados en pantalla.

use egui::{Align, RichText};

use gm_core::config::{GamePriority, PowerPlan, Theme};

use super::{focus_changed, list_row};
use crate::app::{App, BrowserPurpose};
use crate::theme;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Row {
    Fullscreen,
    ThemeMode,
    ActiveFps,
    IdleFps,
    BackgroundFps,
    MinimizeWhilePlaying,
    UiScale,
    Deadzone,
    RepeatDelay,
    GuideButton,
    Rumble,
    PowerPlan,
    StopServices,
    AutoThrottle,
    AutoThrottleMax,
    AutoThrottleMinMb,
    ProtectHandheld,
    EcoQos,
    TrimWorkingSets,
    GamePriority,
    KillExplorer,
    EngageOnStart,
    RestoreOnExit,
    CatalogDir,
    DownloadDir,
    CatalogBudget,
    CoverAutoFetch,
}

const ROWS: &[(&str, Row)] = &[
    ("Interfaz", Row::Fullscreen),
    ("Interfaz", Row::ThemeMode),
    ("Interfaz", Row::ActiveFps),
    ("Interfaz", Row::IdleFps),
    ("Interfaz", Row::BackgroundFps),
    ("Interfaz", Row::MinimizeWhilePlaying),
    ("Interfaz", Row::UiScale),
    ("Mando", Row::Deadzone),
    ("Mando", Row::RepeatDelay),
    ("Mando", Row::GuideButton),
    ("Mando", Row::Rumble),
    ("Rendimiento", Row::PowerPlan),
    ("Rendimiento", Row::StopServices),
    ("Rendimiento", Row::AutoThrottle),
    ("Rendimiento", Row::AutoThrottleMax),
    ("Rendimiento", Row::AutoThrottleMinMb),
    ("Rendimiento", Row::ProtectHandheld),
    ("Rendimiento", Row::EcoQos),
    ("Rendimiento", Row::TrimWorkingSets),
    ("Rendimiento", Row::GamePriority),
    ("Rendimiento", Row::KillExplorer),
    ("Rendimiento", Row::EngageOnStart),
    ("Rendimiento", Row::RestoreOnExit),
    ("Catalogo", Row::CatalogDir),
    ("Catalogo", Row::DownloadDir),
    ("Catalogo", Row::CatalogBudget),
    ("Caratulas", Row::CoverAutoFetch),
];

pub const SETTINGS_ROWS: usize = ROWS.len();

const ACTIVE_FPS: &[u32] = &[30, 45, 60, 90, 120, 144];
const IDLE_FPS: &[u32] = &[2, 5, 10, 20, 30];
const BACKGROUND_FPS: &[u32] = &[1, 2, 4, 8];
const UI_SCALE: &[f32] = &[0.8, 0.9, 1.0, 1.1, 1.25, 1.5];
const DEADZONE: &[f32] = &[0.15, 0.25, 0.35, 0.45];
const REPEAT_DELAY: &[u64] = &[250, 380, 500, 700];
const BUDGET_MB: &[u64] = &[64, 128, 256, 512, 1024];
const THROTTLE_MAX: &[usize] = &[4, 8, 12, 20, 30];
const THROTTLE_MIN_MB: &[u64] = &[50, 80, 120, 200, 400];
const PLANS: &[PowerPlan] = &[PowerPlan::Leave, PowerPlan::Balanced, PowerPlan::HighPerformance, PowerPlan::Ultimate];
const PRIORITIES: &[GamePriority] = &[GamePriority::Normal, GamePriority::AboveNormal, GamePriority::High];
const THEMES: &[Theme] = &[Theme::Dark, Theme::Light];

/// Siguiente (o anterior) valor de una lista cerrada.
fn cycle<T: PartialEq + Copy>(values: &[T], current: T, forward: bool) -> T {
    let index = values.iter().position(|value| *value == current).unwrap_or(0);
    let next = if forward { (index + 1) % values.len() } else { (index + values.len() - 1) % values.len() };
    values[next]
}

fn on_off(value: bool) -> &'static str {
    if value {
        "activado"
    } else {
        "desactivado"
    }
}

impl App {
    fn row_value(&self, row: Row) -> String {
        match row {
            Row::Fullscreen => on_off(self.config.general.fullscreen).to_string(),
            Row::ThemeMode => match self.config.general.theme {
                Theme::Dark => "oscuro".to_string(),
                Theme::Light => "claro".to_string(),
            },
            Row::ActiveFps => format!("{} fps", self.config.general.active_fps),
            Row::IdleFps => format!("{} fps", self.config.general.idle_fps),
            Row::BackgroundFps => format!("{} Hz", self.config.general.background_fps),
            Row::MinimizeWhilePlaying => on_off(self.config.general.minimize_while_playing).to_string(),
            Row::UiScale => format!("{:.2}x", self.config.general.ui_scale),
            Row::Deadzone => format!("{:.0} %", self.config.input.stick_deadzone * 100.0),
            Row::RepeatDelay => format!("{} ms", self.config.input.repeat_delay_ms),
            Row::GuideButton => on_off(self.config.input.capture_guide_button).to_string(),
            Row::Rumble => on_off(self.config.input.rumble_feedback).to_string(),
            Row::PowerPlan => match self.config.power.power_plan {
                PowerPlan::Leave => "no tocar".to_string(),
                PowerPlan::Balanced => "equilibrado".to_string(),
                PowerPlan::HighPerformance => "alto rendimiento".to_string(),
                PowerPlan::Ultimate => "maximo rendimiento".to_string(),
            },
            Row::StopServices => {
                if !self.config.power.stop_services {
                    "desactivado".to_string()
                } else if self.config.power.services.is_empty() {
                    "seleccion recomendada".to_string()
                } else {
                    format!("{} elegidos a mano", self.config.power.services.len())
                }
            }
            Row::AutoThrottle => on_off(self.config.power.auto_throttle).to_string(),
            Row::AutoThrottleMax => format!("{} procesos", self.config.power.auto_throttle_max),
            Row::AutoThrottleMinMb => format!("desde {} MB", self.config.power.auto_throttle_min_mb),
            Row::ProtectHandheld => on_off(self.config.power.protect_handheld_helpers).to_string(),
            Row::EcoQos => on_off(self.config.power.eco_qos_background).to_string(),
            Row::TrimWorkingSets => on_off(self.config.power.trim_working_sets).to_string(),
            Row::GamePriority => match self.config.power.game_priority {
                GamePriority::Normal => "normal".to_string(),
                GamePriority::AboveNormal => "superior a normal".to_string(),
                GamePriority::High => "alta".to_string(),
            },
            Row::KillExplorer => on_off(self.config.power.kill_explorer).to_string(),
            Row::EngageOnStart => on_off(self.config.power.engage_on_start).to_string(),
            Row::RestoreOnExit => on_off(self.config.power.restore_on_exit).to_string(),
            Row::CatalogDir => self
                .config
                .catalog
                .index_dir
                .as_ref()
                .map(|p| p.display().to_string())
                .unwrap_or_else(|| "sin configurar".to_string()),
            Row::DownloadDir => self.config.download_dir().display().to_string(),
            Row::CatalogBudget => format!("{} MB", self.config.catalog.max_index_memory_mb),
            Row::CoverAutoFetch => match &self.config.covers.steamgrid_api_key {
                None => "sin clave configurada".to_string(),
                Some(key) if key.trim().is_empty() => "sin clave configurada".to_string(),
                Some(_) => on_off(self.config.covers.auto_fetch).to_string(),
            },
        }
    }

    fn row_label(row: Row) -> &'static str {
        match row {
            Row::Fullscreen => "Pantalla completa",
            Row::ThemeMode => "Tema",
            Row::ActiveFps => "Ritmo navegando",
            Row::IdleFps => "Ritmo en reposo",
            Row::BackgroundFps => "Ritmo con un juego en marcha",
            Row::MinimizeWhilePlaying => "Minimizar al lanzar un juego",
            Row::UiScale => "Escala de la interfaz",
            Row::Deadzone => "Zona muerta del stick",
            Row::RepeatDelay => "Retardo de repeticion",
            Row::GuideButton => "Capturar el boton Guia",
            Row::Rumble => "Vibracion",
            Row::PowerPlan => "Plan de energia",
            Row::StopServices => "Detener servicios de Windows",
            Row::AutoThrottle => "Buscar solo lo que mas pesa",
            Row::AutoThrottleMax => "Cuantos procesos degradar",
            Row::AutoThrottleMinMb => "A partir de cuanta RAM",
            Row::ProtectHandheld => "Proteger utilidades de portatil",
            Row::EcoQos => "EcoQoS en procesos de fondo",
            Row::TrimWorkingSets => "Liberar memoria de procesos de fondo",
            Row::GamePriority => "Prioridad del juego",
            Row::KillExplorer => "Cerrar explorer.exe (avanzado)",
            Row::EngageOnStart => "Activar modo juego al arrancar",
            Row::RestoreOnExit => "Restaurar el sistema al salir",
            Row::CatalogDir => "Carpeta del catalogo",
            Row::DownloadDir => "Carpeta de descargas",
            Row::CatalogBudget => "Memoria maxima del catalogo",
            Row::CoverAutoFetch => "Descargar caratulas automaticamente",
        }
    }

    fn row_hint(row: Row) -> &'static str {
        match row {
            Row::BackgroundFps => "A menos Hz, menos molesta el shell al juego (solo vigila el boton del mando)",
            Row::StopServices => {
                "Solo se ofrecen servicios que se pueden parar sin romper nada. Elige cuales en Recursos"
            }
            Row::AutoThrottle => "Mide el equipo y degrada los procesos de fondo mas pesados, sin listas de programas",
            Row::ProtectHandheld => {
                "Mando, ventiladores, TDP y superposiciones de Steam Deck y similares: nunca se tocan"
            }
            Row::EcoQos => "Manda los procesos de fondo a los nucleos eficientes; reversible",
            Row::KillExplorer => "Libera RAM y quita la barra de tareas. Se relanza al salir",
            Row::PowerPlan => "Se restaura el plan original al salir del modo juego",
            Row::CatalogDir => "Carpeta con los .jsonl del repositorio Roms",
            Row::Fullscreen => "Se aplica al momento; tambien con F11",
            Row::ThemeMode => "Blanco y negro nada mas: sin acento de color de ningun lanzador",
            Row::CoverAutoFetch => {
                "Via SteamGridDB. Pon tu clave gratuita en covers.steamgrid_api_key dentro de config.toml"
            }
            _ => "",
        }
    }

    /// Cambia el valor de la fila enfocada.
    pub fn adjust_setting(&mut self, index: usize, forward: bool) {
        let Some((_, row)) = ROWS.get(index).copied() else { return };
        match row {
            Row::Fullscreen => {
                self.toggle_fullscreen();
                return;
            }
            Row::ThemeMode => {
                self.set_theme(cycle(THEMES, self.config.general.theme, forward));
                return;
            }
            Row::ActiveFps => {
                self.config.general.active_fps = cycle(ACTIVE_FPS, self.config.general.active_fps, forward)
            }
            Row::IdleFps => self.config.general.idle_fps = cycle(IDLE_FPS, self.config.general.idle_fps, forward),
            Row::BackgroundFps => {
                self.config.general.background_fps = cycle(BACKGROUND_FPS, self.config.general.background_fps, forward)
            }
            Row::MinimizeWhilePlaying => {
                self.config.general.minimize_while_playing = !self.config.general.minimize_while_playing
            }
            Row::UiScale => self.config.general.ui_scale = cycle(UI_SCALE, self.config.general.ui_scale, forward),
            Row::Deadzone => {
                self.config.input.stick_deadzone = cycle(DEADZONE, self.config.input.stick_deadzone, forward)
            }
            Row::RepeatDelay => {
                self.config.input.repeat_delay_ms = cycle(REPEAT_DELAY, self.config.input.repeat_delay_ms, forward)
            }
            Row::GuideButton => {
                self.config.input.capture_guide_button = !self.config.input.capture_guide_button;
            }
            Row::Rumble => self.config.input.rumble_feedback = !self.config.input.rumble_feedback,
            Row::PowerPlan => self.config.power.power_plan = cycle(PLANS, self.config.power.power_plan, forward),
            Row::StopServices => self.config.power.stop_services = !self.config.power.stop_services,
            Row::AutoThrottle => self.config.power.auto_throttle = !self.config.power.auto_throttle,
            Row::AutoThrottleMax => {
                self.config.power.auto_throttle_max = cycle(THROTTLE_MAX, self.config.power.auto_throttle_max, forward)
            }
            Row::AutoThrottleMinMb => {
                self.config.power.auto_throttle_min_mb =
                    cycle(THROTTLE_MIN_MB, self.config.power.auto_throttle_min_mb, forward)
            }
            Row::ProtectHandheld => {
                self.config.power.protect_handheld_helpers = !self.config.power.protect_handheld_helpers
            }
            Row::EcoQos => self.config.power.eco_qos_background = !self.config.power.eco_qos_background,
            Row::TrimWorkingSets => self.config.power.trim_working_sets = !self.config.power.trim_working_sets,
            Row::GamePriority => {
                self.config.power.game_priority = cycle(PRIORITIES, self.config.power.game_priority, forward)
            }
            Row::KillExplorer => self.config.power.kill_explorer = !self.config.power.kill_explorer,
            Row::EngageOnStart => self.config.power.engage_on_start = !self.config.power.engage_on_start,
            Row::RestoreOnExit => self.config.power.restore_on_exit = !self.config.power.restore_on_exit,
            Row::CatalogBudget => {
                self.config.catalog.max_index_memory_mb =
                    cycle(BUDGET_MB, self.config.catalog.max_index_memory_mb, forward)
            }
            Row::CatalogDir => {
                self.open_browser(BrowserPurpose::PickCatalogDir);
                return;
            }
            Row::DownloadDir => {
                self.open_browser(BrowserPurpose::PickDownloadDir);
                return;
            }
            Row::CoverAutoFetch => self.config.covers.auto_fetch = !self.config.covers.auto_fetch,
        }
        self.save_config();
        self.apply_live_settings();
    }

    /// Ajustes que se notan al instante sin reiniciar el shell.
    fn apply_live_settings(&mut self) {
        self.input.set_settings(self.config.input.clone());
    }

    pub(super) fn settings_ui(&mut self, ui: &mut egui::Ui) {
        let focus = self.settings_focus;
        let scroll_to_focus = focus_changed(ui.ctx(), "foco_ajustes", focus);
        let mut activated: Option<usize> = None;
        let mut last_section = "";

        let values: Vec<(usize, &'static str, Row, String)> = ROWS
            .iter()
            .enumerate()
            .map(|(index, (section, row))| (index, *section, *row, self.row_value(*row)))
            .collect();

        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            for (index, section, row, value) in values {
                if section != last_section {
                    ui.add_space(if last_section.is_empty() { 0.0 } else { 14.0 });
                    ui.label(RichText::new(section).size(17.0).strong().color(theme::pal().accent));
                    ui.add_space(6.0);
                    last_section = section;
                }
                let focused = index == focus;
                let hint = App::row_hint(row);
                let height = if focused && !hint.is_empty() { 72.0 } else { 52.0 };
                let response = list_row(ui, focused, height, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(App::row_label(row)).size(18.0));
                        ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
                            ui.label(RichText::new(super::ellipsize(&value, 48)).size(18.0).color(if focused {
                                theme::pal().text
                            } else {
                                theme::pal().text_dim
                            }));
                        });
                    });
                    if focused && !hint.is_empty() {
                        ui.label(RichText::new(hint).size(14.0).color(theme::pal().text_dim));
                    }
                });
                if response.clicked() {
                    activated = Some(index);
                }
                if focused && scroll_to_focus {
                    ui.scroll_to_rect(response.rect.expand(16.0), Some(Align::Center));
                }
                ui.add_space(6.0);
            }
            ui.add_space(20.0);
        });

        if let Some(index) = activated {
            self.settings_focus = index;
            self.adjust_setting(index, true);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn el_ciclo_de_valores_da_la_vuelta_en_los_dos_sentidos() {
        assert_eq!(cycle(ACTIVE_FPS, 30, true), 45);
        assert_eq!(cycle(ACTIVE_FPS, 30, false), 144, "hacia atras desde el primero va al ultimo");
        assert_eq!(cycle(ACTIVE_FPS, 144, true), 30);
        // Un valor que no esta en la lista (config editada a mano) no rompe.
        assert_eq!(cycle(ACTIVE_FPS, 77, true), 45);
    }

    #[test]
    fn cada_fila_tiene_etiqueta_y_seccion() {
        assert_eq!(SETTINGS_ROWS, ROWS.len());
        for (section, row) in ROWS {
            assert!(!section.is_empty());
            assert!(!App::row_label(*row).is_empty(), "falta etiqueta para {row:?}");
        }
    }
}
