//! Configuracion persistente (`config.toml`).
//!
//! Todas las estructuras llevan `#[serde(default)]` a nivel de contenedor: un
//! fichero parcial (o de una version anterior) se completa con los valores por
//! defecto en lugar de fallar.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::error::Result;
use crate::paths;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub general: General,
    pub input: Input,
    pub power: Power,
    pub catalog: Catalog,
    pub covers: Covers,
    pub emulators: Vec<Emulator>,
}

/// Esquema de color de la interfaz. Deliberadamente monocromo (blanco, negro
/// y grises): sin marca, sin acento de color de ningun lanzador.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Theme {
    Light,
    #[default]
    Dark,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct General {
    /// Arrancar en pantalla completa sin bordes (estilo Big Picture).
    pub fullscreen: bool,
    /// Blanco y negro, claro u oscuro. Se cambia al momento desde los
    /// ajustes, sin reiniciar el shell.
    pub theme: Theme,
    /// FPS mientras el usuario navega.
    pub active_fps: u32,
    /// FPS cuando no ha habido entrada durante `idle_after_secs`.
    pub idle_fps: u32,
    pub idle_after_secs: u64,
    /// FPS mientras hay un juego en ejecucion: el shell debe desaparecer del
    /// reparto de CPU/GPU, no competir con el juego.
    pub background_fps: u32,
    /// Ocultar la ventana del shell mientras el juego esta en primer plano.
    pub minimize_while_playing: bool,
    pub show_perf_overlay: bool,
    /// Escala global de la interfaz (1.0 = disenada para 1080p a 2 m de la TV).
    pub ui_scale: f32,
}

impl Default for General {
    fn default() -> Self {
        Self {
            fullscreen: true,
            theme: Theme::Dark,
            active_fps: 60,
            idle_fps: 10,
            idle_after_secs: 8,
            background_fps: 1,
            minimize_while_playing: true,
            show_perf_overlay: false,
            ui_scale: 1.0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Input {
    /// Zona muerta radial del stick izquierdo, 0.0 - 1.0.
    pub stick_deadzone: f32,
    /// A partir de que punto un gatillo cuenta como pulsado.
    pub trigger_threshold: f32,
    /// Retardo antes de que una direccion mantenida empiece a repetirse.
    pub repeat_delay_ms: u64,
    /// Intervalo entre repeticiones una vez arrancado el auto-repeat.
    pub repeat_interval_ms: u64,
    /// Frecuencia de sondeo de XInput mientras el shell tiene el foco.
    pub poll_hz: u32,
    /// Cada cuanto se comprueba si aparecio un mando nuevo. Sondear ranuras
    /// vacias de XInput es caro, asi que no se hace en cada frame.
    pub rescan_interval_ms: u64,
    /// Usar el ordinal 100 de xinput1_4.dll para leer el boton central del
    /// mando (el "Guia" de XInput).
    pub capture_guide_button: bool,
    /// Vibracion corta al lanzar un juego.
    pub rumble_feedback: bool,
    /// Permitir salir del modo juego con el teclado (Alt+F4 / Escape largo).
    pub keyboard_exit: bool,
}

impl Default for Input {
    fn default() -> Self {
        Self {
            stick_deadzone: 0.35,
            trigger_threshold: 0.5,
            repeat_delay_ms: 380,
            repeat_interval_ms: 90,
            poll_hz: 125,
            rescan_interval_ms: 2000,
            capture_guide_button: true,
            rumble_feedback: true,
            keyboard_exit: true,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PowerPlan {
    /// No tocar el plan de energia del usuario.
    Leave,
    Balanced,
    HighPerformance,
    /// "Ultimate Performance"; si no existe se cae a `HighPerformance`.
    Ultimate,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GamePriority {
    Normal,
    AboveNormal,
    High,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Power {
    /// Aplicar el modo juego automaticamente al arrancar el shell.
    pub engage_on_start: bool,
    pub power_plan: PowerPlan,

    /// Detener servicios de Windows mientras dure la sesion.
    pub stop_services: bool,
    /// Que servicios detener. Vacio = la seleccion recomendada del catalogo
    /// seguro. Solo se aceptan servicios de ese catalogo: parar cualquier otro
    /// es la via rapida a quedarse sin sonido o sin red.
    pub services: Vec<String>,

    /// Buscar automaticamente los procesos de fondo que mas recursos estan
    /// consumiendo en este equipo y degradarlos, en vez de ir con una lista
    /// fija de nombres de programas conocidos.
    pub auto_throttle: bool,
    /// Cuantos procesos se degradan como mucho.
    pub auto_throttle_max: usize,
    /// Un proceso entra en el ranking a partir de esta RAM.
    pub auto_throttle_min_mb: u64,
    /// Procesos que el usuario quiere degradar siempre, pesen lo que pesen.
    pub extra_throttle: Vec<String>,

    /// Procesos que no se tocan nunca, ademas de los protegidos de serie.
    pub protected_processes: Vec<String>,
    /// Proteger las utilidades de consolas portatiles (mando, ventiladores,
    /// TDP, superposiciones). Imprescindible en una Steam Deck con Windows.
    pub protect_handheld_helpers: bool,

    /// Aplicar EcoQoS a los procesos degradados: el planificador los manda a
    /// los nucleos eficientes y baja su frecuencia.
    pub eco_qos_background: bool,
    /// Vaciar el working set de los procesos degradados para devolver RAM.
    pub trim_working_sets: bool,
    /// Prioridad del proceso del juego.
    pub game_priority: GamePriority,
    /// Cerrar explorer.exe durante la sesion (libera ~150-300 MB y quita la
    /// barra de tareas del medio). Se relanza al salir.
    pub kill_explorer: bool,
    /// Revertir todo al cerrar el shell.
    pub restore_on_exit: bool,
}

impl Default for Power {
    fn default() -> Self {
        Self {
            engage_on_start: false,
            power_plan: PowerPlan::Ultimate,
            stop_services: true,
            services: Vec::new(),
            auto_throttle: true,
            auto_throttle_max: 12,
            auto_throttle_min_mb: 120,
            extra_throttle: Vec::new(),
            protected_processes: Vec::new(),
            protect_handheld_helpers: true,
            eco_qos_background: true,
            trim_working_sets: true,
            game_priority: GamePriority::High,
            kill_explorer: false,
            restore_on_exit: true,
        }
    }
}

/// Caratulas reales de los juegos, via SteamGridDB.
///
/// Es la unica pieza de la interfaz que habla con un servicio externo, y es
/// puramente una base de datos comunitaria de arte de caratulas: no requiere
/// cuenta para jugar, no lanza nada y no sabe de tiendas ni de compras. Sin
/// clave configurada, esta seccion no hace nada.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Covers {
    /// Clave gratuita de https://www.steamgriddb.com/profile/preferences/api
    pub steamgrid_api_key: Option<String>,
    /// Buscar la caratula sola en cuanto un juego sin caratula aparece en
    /// pantalla. Sin clave configurada no tiene efecto.
    pub auto_fetch: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Catalog {
    /// Carpeta con los `.jsonl` del repositorio Roms.
    pub index_dir: Option<PathBuf>,
    /// Donde se descargan las ROMs.
    pub download_dir: Option<PathBuf>,
    /// Techo de memoria para los indices de plataforma cargados. Al superarlo
    /// se descarga la plataforma usada hace mas tiempo.
    pub max_index_memory_mb: u64,
    /// Numero maximo de entradas que se mantienen en el resultado de busqueda.
    pub max_search_results: usize,
}

impl Default for Catalog {
    fn default() -> Self {
        Self { index_dir: None, download_dir: None, max_index_memory_mb: 256, max_search_results: 500 }
    }
}

/// Emulador externo al que se le pasa una ROM.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Emulator {
    pub id: String,
    pub name: String,
    pub exe: PathBuf,
    /// Argumentos; `{rom}` se sustituye por la ruta de la ROM.
    pub args: Vec<String>,
    /// Identificadores de plataforma que cubre (`snes`, `psx`, ...).
    pub platforms: Vec<String>,
}

impl Default for Emulator {
    fn default() -> Self {
        Self {
            id: String::new(),
            name: String::new(),
            exe: PathBuf::new(),
            args: vec!["{rom}".to_string()],
            platforms: Vec::new(),
        }
    }
}

impl Config {
    pub fn load() -> Result<Self> {
        Self::load_from(&paths::config_file())
    }

    pub fn load_from(path: &std::path::Path) -> Result<Self> {
        match std::fs::read_to_string(path) {
            Ok(text) => Ok(toml::from_str(&text)?),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(e) => Err(e.into()),
        }
    }

    pub fn save(&self) -> Result<()> {
        self.save_to(&paths::config_file())
    }

    pub fn save_to(&self, path: &std::path::Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        crate::util::write_atomic(path, toml::to_string_pretty(self)?.as_bytes())
    }

    /// Emulador asignado a una plataforma del catalogo, si hay alguno.
    pub fn emulator_for(&self, platform: &str) -> Option<&Emulator> {
        self.emulators.iter().find(|e| e.platforms.iter().any(|p| p.eq_ignore_ascii_case(platform)))
    }

    pub fn emulator_by_id(&self, id: &str) -> Option<&Emulator> {
        self.emulators.iter().find(|e| e.id == id)
    }

    pub fn download_dir(&self) -> PathBuf {
        self.catalog.download_dir.clone().unwrap_or_else(|| paths::data_dir().join("roms"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn un_config_parcial_se_completa_con_defaults() {
        let cfg: Config = toml::from_str("[general]\nactive_fps = 30\n").unwrap();
        assert_eq!(cfg.general.active_fps, 30);
        assert_eq!(cfg.general.idle_fps, General::default().idle_fps);
        assert!(cfg.power.restore_on_exit);
        assert!(cfg.power.protect_handheld_helpers, "la proteccion viene activada de serie");
    }

    #[test]
    fn las_claves_viejas_no_rompen_la_configuracion() {
        // Una config de una version anterior traia una lista de nombres de
        // programas; ahora la busqueda es automatica y esa clave sobra.
        let cfg: Config =
            toml::from_str("[power]\nthrottle_processes = [\"navegador\"]\nauto_throttle_max = 5\n").unwrap();
        assert_eq!(cfg.power.auto_throttle_max, 5);
        assert!(cfg.power.auto_throttle);
    }

    #[test]
    fn ida_y_vuelta_a_toml() {
        let cfg = Config::default();
        let text = toml::to_string_pretty(&cfg).unwrap();
        let back: Config = toml::from_str(&text).unwrap();
        assert_eq!(back.power.services, cfg.power.services);
        assert_eq!(back.power.auto_throttle_max, cfg.power.auto_throttle_max);
        assert_eq!(back.input.poll_hz, cfg.input.poll_hz);
    }
}
