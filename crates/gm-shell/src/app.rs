//! Estado y bucle principal del shell.
//!
//! El bucle esta disenado alrededor de una idea: **el shell no debe consumir
//! nada cuando no esta haciendo nada**. egui solo repinta cuando se le pide, y
//! aqui se le pide a un ritmo que depende de lo que este pasando (navegando,
//! en reposo o con un juego en marcha).

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, Sender};
use std::sync::{mpsc, Arc, Mutex};
use std::time::{Duration, Instant};

use crate::covers::CoverTextures;
use gm_catalog::download::Downloader;
use gm_catalog::{Catalog, PlatformIndex};
use gm_core::config::Config;
use gm_core::launcher::{self, Session};
use gm_core::library::{Game, Launch, Library, Sort};
use gm_input::{InputHub, NavAction, PollMode};
use gm_power::{MemoryStatus, Optimizer, Report, SystemScan};

use crate::browser::{Browser, Filter};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum View {
    Library,
    GameDetail,
    Catalog,
    CatalogEntries,
    Downloads,
    /// Que esta consumiendo el equipo y que se va a hacer al respecto.
    Resources,
    Settings,
    /// Alta de la clave de SteamGridDB, con explicacion, pegado y prueba.
    CoverSetup,
    /// Panel rapido del boton Guia; funciona tambien con un juego en marcha.
    Quick,
    Browser,
}

/// Como va la prueba de la clave de SteamGridDB que se esta editando.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KeyTestState {
    Idle,
    Testing,
    Valid,
    Invalid(String),
}

/// Resultado de una busqueda de caratula, vuelto del hilo de fondo.
/// `notify` marca si viene de una peticion explicita del usuario (boton
/// "Buscar caratula" o "Descargar las que faltan"): esas avisan con un aviso
/// en pantalla; las automaticas (al desplazar la biblioteca) se quedan
/// calladas y solo se nota cuando la textura aparece sola.
enum CoverFetchResult {
    Found { game_id: String, title: String, path: PathBuf, notify: bool },
    NotFound { title: String, error: String, notify: bool },
}

/// Para que se abrio el explorador de ficheros.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BrowserPurpose {
    AddExecutable,
    AddRom,
    PickCatalogDir,
    PickDownloadDir,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToastKind {
    Info,
    Good,
    Bad,
}

pub struct Toast {
    pub text: String,
    pub kind: ToastKind,
    pub until: Instant,
}

/// Carga de una plataforma del catalogo en un hilo aparte.
struct LoadJob {
    id: String,
    rx: Receiver<gm_core::Result<PlatformIndex>>,
    started: Instant,
}

pub struct App {
    pub config: Config,
    pub library: Library,
    pub catalog: Catalog,
    pub downloader: Downloader,
    pub input: InputHub,

    /// El optimizador se maneja desde un hilo: parar servicios puede tardar
    /// segundos y la interfaz no se puede quedar congelada mientras tanto.
    optimizer: Arc<Mutex<Optimizer>>,
    power_busy: Arc<AtomicBool>,
    power_tx: Sender<Report>,
    power_rx: Receiver<Report>,
    pub power_engaged: bool,
    pub last_report: Option<Report>,

    pub session: Option<Session>,
    pub view: View,
    stack: Vec<View>,

    // Biblioteca
    pub library_focus: usize,
    pub library_view: Vec<usize>,
    library_dirty: bool,
    pub query: String,
    pub sort: Sort,
    pub favorites_only: bool,
    pub search_active: bool,

    // Catalogo
    pub platform_focus: usize,
    pub open_platform: Option<String>,
    pub entry_focus: usize,
    pub entry_hits: Vec<usize>,
    pub catalog_query: String,
    loading: Option<LoadJob>,

    // Explorador de ficheros
    pub browser: Option<Browser>,
    pub browser_purpose: BrowserPurpose,

    // Menus
    pub quick_focus: usize,
    pub settings_focus: usize,
    pub downloads_focus: usize,

    // Recursos del sistema
    pub resources: Option<SystemScan>,
    pub resource_focus: usize,
    resource_busy: Arc<AtomicBool>,
    resource_tx: Sender<SystemScan>,
    resource_rx: Receiver<SystemScan>,
    last_scan: Option<Instant>,

    pub toasts: Vec<Toast>,
    pub memory: MemoryStatus,
    /// Cambio de pantalla completa pendiente de enviarle a la ventana.
    pending_fullscreen: Option<bool>,
    /// Se puso true al cambiar de tema: hay que releer el estilo de egui.
    pending_theme_apply: bool,
    first_frame: bool,
    last_memory_check: Instant,
    pub last_activity: Instant,
    /// Con que se jugo por ultima vez: decide que boton ensenar en la ayuda
    /// de pantalla. Cambia solo, como en cualquier consola.
    pub input_source: crate::nav::InputSource,
    download_meta: HashMap<u64, (String, String)>,
    handled_downloads: HashSet<u64>,
    pub columns: usize,

    // Caratulas (SteamGridDB)
    pub covers: CoverTextures,
    /// Juegos para los que ya se intento buscar caratula esta sesion, con
    /// exito o sin el: evita volver a pedirla solo porque la tarjeta ha
    /// vuelto a entrar en pantalla al desplazar la rejilla.
    cover_attempted: HashSet<String>,
    cover_tx: Sender<CoverFetchResult>,
    cover_rx: Receiver<CoverFetchResult>,

    // Alta de la clave de SteamGridDB (pantalla dedicada, ver views/cover_setup.rs)
    /// Lo que hay escrito en el campo ahora mismo; no es la clave guardada
    /// hasta que se pulsa Guardar (o se sale de la pantalla).
    pub cover_key_draft: String,
    pub cover_key_editing: bool,
    pub cover_key_test: KeyTestState,
    cover_test_busy: Arc<AtomicBool>,
    cover_test_tx: Sender<Result<(), String>>,
    cover_test_rx: Receiver<Result<(), String>>,
}

impl App {
    pub fn new(ctx: &egui::Context) -> Self {
        let config = Config::load().unwrap_or_else(|e| {
            log::error!("config invalida, se usan los valores por defecto: {e}");
            Config::default()
        });
        crate::theme::set_theme(config.general.theme);
        crate::theme::install(ctx, config.general.ui_scale);

        let library = Library::load().unwrap_or_else(|e| {
            log::error!("no se pudo leer la biblioteca: {e}");
            Library::default()
        });
        let catalog = Catalog::new(config.catalog.index_dir.clone(), config.catalog.max_index_memory_mb);
        let downloader = Downloader::new(2);
        let input = InputHub::new(config.input.clone());
        let optimizer = Optimizer::new(config.power.clone());
        let (power_tx, power_rx) = mpsc::channel();
        let (resource_tx, resource_rx) = mpsc::channel();
        let (cover_tx, cover_rx) = mpsc::channel();
        let (cover_test_tx, cover_test_rx) = mpsc::channel();

        let mut app = Self {
            config,
            library,
            catalog,
            downloader,
            input,
            optimizer: Arc::new(Mutex::new(optimizer)),
            power_busy: Arc::new(AtomicBool::new(false)),
            power_tx,
            power_rx,
            power_engaged: false,
            last_report: None,
            session: None,
            view: View::Library,
            stack: Vec::new(),
            library_focus: 0,
            library_view: Vec::new(),
            library_dirty: true,
            query: String::new(),
            sort: Sort::LastPlayed,
            favorites_only: false,
            search_active: false,
            platform_focus: 0,
            open_platform: None,
            entry_focus: 0,
            entry_hits: Vec::new(),
            catalog_query: String::new(),
            loading: None,
            browser: None,
            browser_purpose: BrowserPurpose::AddExecutable,
            quick_focus: 0,
            settings_focus: 0,
            downloads_focus: 0,
            resources: None,
            resource_focus: 0,
            resource_busy: Arc::new(AtomicBool::new(false)),
            resource_tx,
            resource_rx,
            last_scan: None,
            toasts: Vec::new(),
            memory: MemoryStatus::default(),
            pending_fullscreen: None,
            pending_theme_apply: false,
            first_frame: true,
            last_memory_check: Instant::now() - Duration::from_secs(60),
            last_activity: Instant::now(),
            input_source: crate::nav::InputSource::KeyboardMouse,
            download_meta: HashMap::new(),
            handled_downloads: HashSet::new(),
            columns: 5,
            covers: CoverTextures::default(),
            cover_attempted: HashSet::new(),
            cover_tx,
            cover_rx,
            cover_key_draft: String::new(),
            cover_key_editing: false,
            cover_key_test: KeyTestState::Idle,
            cover_test_busy: Arc::new(AtomicBool::new(false)),
            cover_test_tx,
            cover_test_rx,
        };

        // Si la sesion anterior se fue sin restaurar, se deshace ahora.
        let recovered = app.optimizer.lock().ok().and_then(|mut optimizer| optimizer.recover_stale());
        if let Some(report) = recovered {
            app.toast(ToastKind::Info, format!("Sesion anterior revertida: {}", report.summary()));
        }
        if app.config.power.engage_on_start {
            app.toggle_power();
        }
        app
    }

    // ------------------------------------------------------------------
    // Utilidades de estado
    // ------------------------------------------------------------------

    pub fn toast(&mut self, kind: ToastKind, text: impl Into<String>) {
        let text = text.into();
        log::info!("{text}");
        self.toasts.push(Toast { text, kind, until: Instant::now() + Duration::from_secs(5) });
        // No tiene sentido apilar mas de un punado.
        if self.toasts.len() > 4 {
            self.toasts.remove(0);
        }
    }

    pub fn push_view(&mut self, view: View) {
        self.stack.push(self.view);
        self.view = view;
    }

    pub fn pop_view(&mut self) {
        self.view = self.stack.pop().unwrap_or(View::Library);
    }

    /// Cambia de vista principal vaciando la pila de navegacion.
    pub fn set_root_view(&mut self, view: View) {
        self.stack.clear();
        self.view = view;
    }

    /// Siguiente criterio de ordenacion de la biblioteca.
    pub fn cycle_sort(&mut self) {
        self.sort = match self.sort {
            Sort::LastPlayed => Sort::Title,
            Sort::Title => Sort::PlayTime,
            Sort::PlayTime => Sort::Added,
            Sort::Added => Sort::LastPlayed,
        };
        self.mark_library_dirty();
    }

    /// Pone o quita la pantalla completa y lo deja guardado.
    ///
    /// El cambio se aplica en el acto: antes habia que reiniciar el shell para
    /// que la casilla de los ajustes sirviese de algo.
    pub fn set_fullscreen(&mut self, fullscreen: bool) {
        self.config.general.fullscreen = fullscreen;
        self.pending_fullscreen = Some(fullscreen);
        self.save_config();
    }

    pub fn toggle_fullscreen(&mut self) {
        self.set_fullscreen(!self.config.general.fullscreen);
    }

    /// Cambia entre el tema claro y el oscuro, y lo deja guardado. Se nota en
    /// el acto, sin reiniciar el shell.
    pub fn set_theme(&mut self, theme: gm_core::config::Theme) {
        self.config.general.theme = theme;
        crate::theme::set_theme(theme);
        self.pending_theme_apply = true;
        self.save_config();
    }

    /// Reafirma el estado de ventana guardado en la configuracion.
    fn request_window_mode(&mut self) {
        self.pending_fullscreen = Some(self.config.general.fullscreen);
    }

    fn apply_window_mode(&mut self, ctx: &egui::Context) {
        let Some(fullscreen) = self.pending_fullscreen.take() else { return };
        // Sin bordes en pantalla completa; con ellos en ventana, para poder
        // moverla y cerrarla como cualquier otra.
        ctx.send_viewport_cmd(egui::ViewportCommand::Decorations(!fullscreen));
        ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(fullscreen));
    }

    pub fn power_busy(&self) -> bool {
        self.power_busy.load(Ordering::Relaxed)
    }

    /// Ritmo al que conviene trabajar ahora mismo.
    pub fn poll_mode(&self) -> PollMode {
        if self.session.is_some() && self.view != View::Quick {
            PollMode::InGame
        } else if self.last_activity.elapsed() > Duration::from_secs(self.config.general.idle_after_secs) {
            PollMode::Idle
        } else {
            PollMode::Active
        }
    }

    pub fn mark_library_dirty(&mut self) {
        self.library_dirty = true;
    }

    /// Recalcula la lista filtrada solo cuando algo ha cambiado: ordenar unos
    /// miles de juegos en cada frame seria tirar CPU a la basura.
    fn refresh_library_view(&mut self) {
        if !self.library_dirty {
            return;
        }
        self.library_view = self.library.view(&self.query, None, self.sort, self.favorites_only);
        self.library_focus = self.library_focus.min(self.library_view.len().saturating_sub(1));
        self.library_dirty = false;
    }

    pub fn focused_game(&self) -> Option<&Game> {
        let index = *self.library_view.get(self.library_focus)?;
        self.library.games.get(index)
    }

    // ------------------------------------------------------------------
    // Acciones
    // ------------------------------------------------------------------

    pub fn launch_focused(&mut self, ctx: &egui::Context) {
        let Some(game) = self.focused_game().cloned() else { return };
        if self.session.is_some() {
            self.toast(ToastKind::Info, "Ya hay un juego en marcha");
            return;
        }
        match launcher::launch(&game, &self.config) {
            Ok(session) => {
                if let Ok(mut optimizer) = self.optimizer.lock() {
                    optimizer.set_game_pid(session.pid);
                }
                self.input.rumble(0.4, Duration::from_millis(180), Instant::now());
                self.toast(ToastKind::Good, format!("Lanzando {}", game.title));
                self.session = Some(session);
                self.view = View::Library;
                self.stack.clear();
                if self.config.general.minimize_while_playing {
                    // Apartarse: el juego debe quedarse con la pantalla y con
                    // la GPU.
                    ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(true));
                }
            }
            Err(e) => self.toast(ToastKind::Bad, format!("No se pudo lanzar {}: {e}", game.title)),
        }
    }

    pub fn toggle_favorite(&mut self) {
        let Some(index) = self.library_view.get(self.library_focus).copied() else { return };
        if let Some(game) = self.library.games.get_mut(index) {
            game.favorite = !game.favorite;
            let message = if game.favorite { "Anadido a favoritos" } else { "Quitado de favoritos" };
            self.save_library();
            self.mark_library_dirty();
            self.toast(ToastKind::Info, message);
        }
    }

    pub fn remove_focused(&mut self) {
        let Some(index) = self.library_view.get(self.library_focus).copied() else { return };
        let Some(game) = self.library.games.get(index).cloned() else { return };
        self.library.remove(&game.id);
        self.save_library();
        self.mark_library_dirty();
        self.toast(ToastKind::Info, format!("{} quitado de la biblioteca", game.title));
        if self.view == View::GameDetail {
            self.pop_view();
        }
    }

    pub fn save_library(&mut self) {
        if let Err(e) = self.library.save() {
            self.toast(ToastKind::Bad, format!("No se pudo guardar la biblioteca: {e}"));
        }
    }

    pub fn save_config(&mut self) {
        let power = self.config.power.clone();
        if let Ok(mut optimizer) = self.optimizer.lock() {
            optimizer.set_config(power);
        }
        self.input.set_settings(self.config.input.clone());
        if let Err(e) = self.config.save() {
            self.toast(ToastKind::Bad, format!("No se pudo guardar la configuracion: {e}"));
        }
    }

    /// Activa o revierte el modo juego en segundo plano.
    pub fn toggle_power(&mut self) {
        if self.power_busy() {
            self.toast(ToastKind::Info, "El modo juego ya esta trabajando...");
            return;
        }
        let optimizer = Arc::clone(&self.optimizer);
        let busy = Arc::clone(&self.power_busy);
        let tx = self.power_tx.clone();
        busy.store(true, Ordering::Relaxed);
        std::thread::spawn(move || {
            let report = match optimizer.lock() {
                Ok(mut optimizer) => {
                    if optimizer.is_engaged() {
                        optimizer.restore()
                    } else {
                        optimizer.engage()
                    }
                }
                Err(_) => Report::default(),
            };
            busy.store(false, Ordering::Relaxed);
            let _ = tx.send(report);
        });
    }

    /// Mide el equipo en segundo plano: enumerar procesos y abrir un handle
    /// por cada uno tarda lo suyo y no puede bloquear el dibujado.
    pub fn scan_resources(&mut self) {
        if self.resource_busy.load(Ordering::Relaxed) {
            return;
        }
        let config = self.config.power.clone();
        let enabled = self.enabled_services();
        let game_pid = self.session.as_ref().and_then(|session| session.pid);
        let busy = Arc::clone(&self.resource_busy);
        let tx = self.resource_tx.clone();
        busy.store(true, Ordering::Relaxed);
        self.last_scan = Some(Instant::now());
        std::thread::spawn(move || {
            let scan = gm_power::scan(&config, &enabled, game_pid);
            busy.store(false, Ordering::Relaxed);
            let _ = tx.send(scan);
        });
    }

    pub fn scanning(&self) -> bool {
        self.resource_busy.load(Ordering::Relaxed)
    }

    /// Servicios marcados para detener. La lista vacia significa "la seleccion
    /// recomendada", asi que hay que resolverla antes de ensenarla o tocarla.
    pub fn enabled_services(&self) -> Vec<String> {
        if !self.config.power.stop_services {
            return Vec::new();
        }
        if self.config.power.services.is_empty() {
            gm_power::services::defaults()
        } else {
            gm_power::services::sanitize(&self.config.power.services).0
        }
    }

    /// Marca o desmarca un servicio del catalogo.
    pub fn toggle_service(&mut self, name: &str) {
        let mut enabled = self.enabled_services();
        match enabled.iter().position(|other| other.eq_ignore_ascii_case(name)) {
            Some(index) => {
                enabled.remove(index);
            }
            None => {
                if !gm_power::services::is_allowed(name) {
                    self.toast(ToastKind::Bad, format!("{name} no esta en el catalogo seguro"));
                    return;
                }
                enabled.push(name.to_string());
            }
        }
        // Al tocar algo se deja de heredar la seleccion recomendada; si se
        // vacia del todo se apaga la parada de servicios, que es lo que el
        // usuario esta pidiendo.
        self.config.power.stop_services = !enabled.is_empty();
        self.config.power.services = enabled;
        self.save_config();
        self.refresh_scan_marks();
    }

    /// Vuelve a la seleccion recomendada del catalogo.
    pub fn reset_services(&mut self) {
        self.config.power.stop_services = true;
        self.config.power.services.clear();
        self.save_config();
        self.refresh_scan_marks();
        self.toast(ToastKind::Info, "Servicios: seleccion recomendada");
    }

    /// Reetiqueta la ultima medicion sin volver a medir: cambiar una casilla no
    /// justifica recorrer otra vez todos los procesos del equipo.
    fn refresh_scan_marks(&mut self) {
        let enabled = self.enabled_services();
        if let Some(scan) = self.resources.as_mut() {
            for report in &mut scan.services {
                report.enabled = enabled.iter().any(|name| name.eq_ignore_ascii_case(report.entry.name));
            }
        }
    }

    pub fn open_browser(&mut self, purpose: BrowserPurpose) {
        let filter = match purpose {
            BrowserPurpose::AddExecutable => Filter::Executables,
            BrowserPurpose::AddRom => Filter::Roms,
            BrowserPurpose::PickCatalogDir | BrowserPurpose::PickDownloadDir => Filter::Directories,
        };
        let start = match purpose {
            BrowserPurpose::PickCatalogDir => self.config.catalog.index_dir.clone(),
            BrowserPurpose::PickDownloadDir => self.config.catalog.download_dir.clone(),
            _ => None,
        };
        self.browser_purpose = purpose;
        self.browser = Some(Browser::new(filter, start));
        self.push_view(View::Browser);
    }

    /// El explorador ha devuelto una ruta (fichero o carpeta).
    pub fn browser_result(&mut self, path: PathBuf) {
        match self.browser_purpose {
            BrowserPurpose::AddExecutable => {
                let title = Game::title_from_path(&path);
                let mut game = Game::new(
                    title.clone(),
                    Launch::Executable { path: path.clone(), args: Vec::new(), working_dir: None },
                );
                game.collection = "PC".to_string();
                if self.library.add(game) {
                    self.save_library();
                    self.mark_library_dirty();
                    self.toast(ToastKind::Good, format!("{title} anadido a la biblioteca"));
                } else {
                    self.toast(ToastKind::Info, format!("{title} ya estaba en la biblioteca"));
                }
                self.pop_view();
            }
            BrowserPurpose::AddRom => {
                let title = Game::title_from_path(&path);
                let platform = self.open_platform.clone().unwrap_or_default();
                let mut game =
                    Game::new(title.clone(), Launch::Rom { path, platform: platform.clone(), emulator_id: None });
                game.collection = gm_catalog::display_name(&platform);
                if self.library.add(game) {
                    self.save_library();
                    self.mark_library_dirty();
                    self.toast(ToastKind::Good, format!("{title} anadido a la biblioteca"));
                }
                self.pop_view();
            }
            BrowserPurpose::PickCatalogDir => {
                self.config.catalog.index_dir = Some(path.clone());
                self.save_config();
                if let Err(e) = self.catalog.set_dir(Some(path)) {
                    self.toast(ToastKind::Bad, format!("Catalogo: {e}"));
                } else {
                    self.toast(
                        ToastKind::Good,
                        format!("Catalogo: {} plataformas detectadas", self.catalog.platforms().len()),
                    );
                }
                self.pop_view();
            }
            BrowserPurpose::PickDownloadDir => {
                self.config.catalog.download_dir = Some(path);
                self.save_config();
                self.toast(ToastKind::Good, "Carpeta de descargas actualizada");
                self.pop_view();
            }
        }
    }

    /// Abre una plataforma del catalogo, cargandola en un hilo si hace falta.
    pub fn open_platform(&mut self, id: String) {
        self.entry_focus = 0;
        self.catalog_query.clear();
        if self.catalog.is_loaded(&id) {
            self.catalog.touch(&id);
            self.open_platform = Some(id);
            self.refresh_entry_hits();
            self.push_view(View::CatalogEntries);
            return;
        }
        let Some(info) = self.catalog.platform(&id).cloned() else {
            self.toast(ToastKind::Bad, format!("La plataforma '{id}' ya no esta en el catalogo"));
            return;
        };
        let (tx, rx) = mpsc::channel();
        let path = info.path.clone();
        let job_id = id.clone();
        std::thread::spawn(move || {
            let _ = tx.send(gm_catalog::load_platform(&path, &job_id));
        });
        self.loading = Some(LoadJob { id, rx, started: Instant::now() });
    }

    pub fn is_loading(&self) -> Option<(&str, Duration)> {
        self.loading.as_ref().map(|job| (job.id.as_str(), job.started.elapsed()))
    }

    pub fn refresh_entry_hits(&mut self) {
        let Some(platform) = self.open_platform.clone() else {
            self.entry_hits.clear();
            return;
        };
        self.entry_hits = self.catalog.search(&platform, &self.catalog_query, self.config.catalog.max_search_results);
        self.entry_focus = self.entry_focus.min(self.entry_hits.len().saturating_sub(1));
    }

    /// Encola la descarga de la entrada enfocada.
    pub fn download_focused_entry(&mut self) {
        let Some(platform) = self.open_platform.clone() else { return };
        let Some(index) = self.entry_hits.get(self.entry_focus).copied() else { return };
        let Some(entry) = self.catalog.get(&platform).and_then(|p| p.entries.get(index)).cloned() else { return };
        let Some(url) = entry.primary_url().map(|u| u.to_string()) else {
            self.toast(ToastKind::Bad, "Esa entrada no tiene enlace de descarga");
            return;
        };
        if !self.downloader.is_available() {
            self.toast(ToastKind::Bad, "Las descargas solo funcionan en Windows");
            return;
        }
        let dest = self.config.download_dir().join(&platform).join(entry.file_name());
        let id = self.downloader.enqueue(entry.name.clone(), url, dest);
        self.download_meta.insert(id, (platform, entry.name.clone()));
        self.toast(ToastKind::Good, format!("Descargando {}", entry.name));
    }

    /// La clave configurada, si hay alguna que no sea solo espacios.
    fn cover_key(&self) -> Option<String> {
        self.config.covers.steamgrid_api_key.clone().filter(|key| !key.trim().is_empty())
    }

    /// Lanza la busqueda de una caratula en un hilo aparte. No abre ninguna
    /// ventana ni dialogo: el resultado vuelve por el canal y `pump()` lo
    /// recoge, avisando en pantalla solo si `notify` esta activo.
    fn spawn_cover_fetch(&self, api_key: String, game_id: String, title: String, notify: bool) {
        let dest_dir = gm_core::paths::covers_dir();
        let tx = self.cover_tx.clone();
        std::thread::spawn(move || {
            let result = match gm_catalog::steamgrid::fetch_cover(&api_key, &title, &game_id, &dest_dir) {
                Ok(path) => CoverFetchResult::Found { game_id, title, path, notify },
                Err(e) => CoverFetchResult::NotFound { title, error: e.to_string(), notify },
            };
            let _ = tx.send(result);
        });
    }

    /// Busca la caratula de un juego que acaba de aparecer en pantalla, sin
    /// que el usuario haya pedido nada: silenciosa, y solo una vez por
    /// partida mientras el shell este abierto (si falla, no se reintenta solo
    /// por volver a desplazar la rejilla hasta esa tarjeta).
    pub fn maybe_fetch_cover(&mut self, game_id: &str, title: &str) {
        if !self.config.covers.auto_fetch || self.cover_attempted.contains(game_id) {
            return;
        }
        let Some(api_key) = self.cover_key() else { return };
        self.cover_attempted.insert(game_id.to_string());
        self.spawn_cover_fetch(api_key, game_id.to_string(), title.to_string(), false);
    }

    /// Busca (o vuelve a buscar) la caratula de un juego concreto porque el
    /// usuario lo ha pedido -boton "Buscar caratula" de la ficha-, avisando
    /// del resultado aunque sea un fallo: es la via para forzar un reintento
    /// cuando la busqueda automatica no encontro nada (titulo raro sacado del
    /// nombre del .exe, por ejemplo).
    pub fn fetch_cover_now(&mut self, game_id: &str, title: &str) {
        let Some(api_key) = self.cover_key() else {
            self.toast(ToastKind::Bad, "Antes configura una clave en Ajustes → Caratulas automaticas");
            return;
        };
        self.toast(ToastKind::Info, format!("Buscando caratula de \"{title}\"..."));
        self.cover_attempted.insert(game_id.to_string());
        self.spawn_cover_fetch(api_key, game_id.to_string(), title.to_string(), true);
    }

    /// Lanza una busqueda para cada juego de la biblioteca que todavia no
    /// tiene caratula, de una vez. Es el boton "Descargar las que faltan" de
    /// la pantalla de SteamGridDB: el motivo de tener la clave puesta es
    /// justo este, no quedarse esperando a que cada juego pase por la
    /// rejilla.
    pub fn fetch_all_missing_covers(&mut self) {
        let Some(api_key) = self.cover_key() else {
            self.toast(ToastKind::Bad, "Antes configura una clave en Ajustes → Caratulas automaticas");
            return;
        };
        let missing: Vec<(String, String)> = self
            .library
            .games
            .iter()
            .filter(|game| game.cover.is_none())
            .map(|game| (game.id.clone(), game.title.clone()))
            .collect();
        if missing.is_empty() {
            self.toast(ToastKind::Info, "Todos los juegos de la biblioteca ya tienen caratula");
            return;
        }
        self.toast(ToastKind::Info, format!("Buscando caratula para {} juegos...", missing.len()));
        for (game_id, title) in missing {
            self.cover_attempted.insert(game_id.clone());
            self.spawn_cover_fetch(api_key.clone(), game_id, title, false);
        }
    }

    /// Abre la pantalla de alta de la clave de SteamGridDB, con lo que haya
    /// guardado ya como punto de partida.
    pub fn open_cover_setup(&mut self) {
        self.cover_key_draft = self.config.covers.steamgrid_api_key.clone().unwrap_or_default();
        self.cover_key_editing = false;
        self.cover_key_test = KeyTestState::Idle;
        self.push_view(View::CoverSetup);
    }

    /// Pega el contenido del portapapeles en el campo. Pensado para poder
    /// hacerlo con el mando (copiar la clave en el navegador del telefono o
    /// del propio PC y traerla aqui sin escribir un solo caracter).
    pub fn cover_key_paste(&mut self) {
        match arboard::Clipboard::new().and_then(|mut clipboard| clipboard.get_text()) {
            Ok(text) => {
                self.cover_key_draft = text.trim().to_string();
                self.cover_key_test = KeyTestState::Idle;
                self.toast(ToastKind::Info, "Pegado del portapapeles");
            }
            Err(e) => self.toast(ToastKind::Bad, format!("No se pudo leer el portapapeles: {e}")),
        }
    }

    pub fn cover_key_clear(&mut self) {
        self.cover_key_draft.clear();
        self.cover_key_test = KeyTestState::Idle;
    }

    pub fn cover_key_busy(&self) -> bool {
        self.cover_test_busy.load(Ordering::Relaxed)
    }

    /// Prueba la clave escrita contra SteamGridDB de verdad, en un hilo aparte,
    /// sin guardar nada todavia. Es la diferencia entre "guardar y enterarte
    /// del error la primera vez que falle en silencio" y saberlo al momento.
    pub fn test_cover_key(&mut self) {
        let key = self.cover_key_draft.trim().to_string();
        if key.is_empty() {
            self.cover_key_test = KeyTestState::Invalid("no hay nada escrito todavia".to_string());
            return;
        }
        if self.cover_key_busy() {
            return;
        }
        self.cover_key_test = KeyTestState::Testing;
        let busy = Arc::clone(&self.cover_test_busy);
        let tx = self.cover_test_tx.clone();
        busy.store(true, Ordering::Relaxed);
        std::thread::spawn(move || {
            let result = gm_catalog::steamgrid::validate_key(&key).map_err(|e| e.to_string());
            busy.store(false, Ordering::Relaxed);
            let _ = tx.send(result);
        });
    }

    /// Guarda el borrador como clave definitiva (o lo borra si esta vacio) y
    /// vuelve a los ajustes.
    pub fn save_cover_key(&mut self) {
        let key = self.cover_key_draft.trim().to_string();
        let had_key = self.config.covers.steamgrid_api_key.is_some();
        self.config.covers.steamgrid_api_key = if key.is_empty() { None } else { Some(key) };
        self.save_config();
        match (had_key, &self.config.covers.steamgrid_api_key) {
            (_, Some(_)) => self.toast(ToastKind::Good, "Clave de SteamGridDB guardada"),
            (true, None) => self.toast(ToastKind::Info, "Clave de SteamGridDB borrada"),
            (false, None) => {}
        }
        self.cover_key_editing = false;
        self.pop_view();
    }

    // ------------------------------------------------------------------
    // Trabajo de fondo
    // ------------------------------------------------------------------

    fn pump(&mut self, ctx: &egui::Context) {
        let now = Instant::now();

        // Caratulas de SteamGridDB que acaban de llegar (o de fallar).
        let mut cover_arrived = false;
        while let Ok(result) = self.cover_rx.try_recv() {
            match result {
                CoverFetchResult::Found { game_id, title, path, notify } => {
                    if let Some(game) = self.library.get_mut(&game_id) {
                        game.cover = Some(path);
                        self.covers.invalidate(&game_id);
                        self.mark_library_dirty();
                        cover_arrived = true;
                    }
                    if notify {
                        self.toast(ToastKind::Good, format!("Caratula de \"{title}\" descargada"));
                    }
                }
                CoverFetchResult::NotFound { title, error, notify } => {
                    log::debug!("sin caratula para '{title}': {error}");
                    if notify {
                        self.toast(ToastKind::Bad, format!("Sin caratula para \"{title}\": {error}"));
                    }
                }
            }
        }
        if cover_arrived {
            // Se guarda de inmediato: una caratula descargada no deberia
            // perderse si el shell se cierra antes del siguiente cambio en la
            // biblioteca.
            self.save_library();
        }

        // Resultado de probar la clave de SteamGridDB.
        if let Ok(result) = self.cover_test_rx.try_recv() {
            self.cover_key_test = match result {
                Ok(()) => KeyTestState::Valid,
                Err(e) => KeyTestState::Invalid(e),
            };
        }

        // Juego terminado?
        let finished = match self.session.as_mut() {
            Some(session) => !session.is_running(),
            None => false,
        };
        if finished {
            if let Some(session) = self.session.take() {
                if let Ok(mut optimizer) = self.optimizer.lock() {
                    optimizer.set_game_pid(None);
                }
                let seconds = session.elapsed_secs();
                self.library.record_session(&session.game_id, seconds);
                self.save_library();
                self.mark_library_dirty();
                ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(false));
                ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
                // Salir de minimizado puede dejar la ventana en modo normal.
                self.request_window_mode();
                self.toast(ToastKind::Info, format!("{}: {}", session.title, gm_core::util::format_playtime(seconds)));
                self.last_activity = now;
            }
        }

        // Plataforma del catalogo cargada?
        if let Some(job) = self.loading.as_ref() {
            match job.rx.try_recv() {
                Ok(Ok(index)) => {
                    let id = index.id.clone();
                    let count = index.entries.len();
                    self.catalog.insert(index);
                    self.open_platform = Some(id.clone());
                    self.loading = None;
                    self.refresh_entry_hits();
                    self.push_view(View::CatalogEntries);
                    self.toast(ToastKind::Info, format!("{count} entradas cargadas"));
                }
                Ok(Err(e)) => {
                    self.toast(ToastKind::Bad, format!("No se pudo cargar la plataforma: {e}"));
                    self.loading = None;
                }
                Err(mpsc::TryRecvError::Empty) => {}
                Err(mpsc::TryRecvError::Disconnected) => self.loading = None,
            }
        }

        // Informe del optimizador.
        while let Ok(report) = self.power_rx.try_recv() {
            let kind = if report.count(gm_power::Outcome::Failed) > 0 { ToastKind::Info } else { ToastKind::Good };
            let summary = report.summary();
            self.power_engaged = self.optimizer.try_lock().map(|o| o.is_engaged()).unwrap_or(self.power_engaged);
            let prefix = if self.power_engaged { "Modo juego activo" } else { "Sistema restaurado" };
            self.toast(kind, format!("{prefix}: {summary}"));
            self.last_report = Some(report);
        }

        // Medicion de recursos.
        while let Ok(scan) = self.resource_rx.try_recv() {
            self.resource_focus = self.resource_focus.min(scan.services.len().saturating_sub(1));
            self.resources = Some(scan);
        }
        if self.view == View::Resources && !self.scanning() {
            let stale = self.last_scan.is_none_or(|last| last.elapsed() > Duration::from_secs(10));
            if stale {
                self.scan_resources();
            }
        }

        // Descargas.
        self.downloader.pump();
        self.collect_finished_downloads();

        // Memoria para la barra superior, dos veces por segundo como mucho.
        if now.duration_since(self.last_memory_check) > Duration::from_millis(1500) {
            self.last_memory_check = now;
            if let Ok(status) = gm_power::memory_status() {
                self.memory = status;
            }
        }

        self.toasts.retain(|toast| toast.until > now);
    }

    /// Anade a la biblioteca las ROMs cuya descarga acaba de terminar.
    fn collect_finished_downloads(&mut self) {
        let finished: Vec<(u64, PathBuf)> = self
            .downloader
            .items()
            .iter()
            .filter(|item| item.snapshot().state == gm_catalog::download::DownloadState::Done)
            .filter(|item| !self.handled_downloads.contains(&item.id))
            .map(|item| (item.id, item.dest.clone()))
            .collect();

        for (id, path) in finished {
            self.handled_downloads.insert(id);
            let Some((platform, name)) = self.download_meta.get(&id).cloned() else { continue };
            let mut game = Game::new(name.clone(), Launch::Rom { path, platform: platform.clone(), emulator_id: None });
            game.collection = gm_catalog::display_name(&platform);
            if self.library.add(game) {
                self.save_library();
                self.mark_library_dirty();
            }
            self.toast(ToastKind::Good, format!("{name} descargado y anadido a la biblioteca"));
        }
    }

    // ------------------------------------------------------------------
    // Entrada
    // ------------------------------------------------------------------

    fn collect_actions(&mut self, ctx: &egui::Context) -> Vec<NavAction> {
        let now = Instant::now();
        let frame = self.input.poll(now);
        let mut actions = frame.actions;

        // La ayuda de botones sigue a quien jugo por ultimo, no a quien esta
        // conectado: con un mando y un teclado a la vez, se ensena el que se
        // acaba de usar.
        if frame.activity {
            let kind = self.input.pad_kind().unwrap_or(gm_input::PadKind::Xbox);
            self.input_source = crate::nav::InputSource::from_pad_kind(kind);
        } else if crate::nav::keyboard_or_mouse_activity(ctx) {
            self.input_source = crate::nav::InputSource::KeyboardMouse;
        }

        // Mientras se escribe en un buscador o en la clave de SteamGridDB el
        // teclado es texto, no navegacion; el mando sigue funcionando con
        // normalidad (WASD/F/Tab del teclado si se colarian en la clave que
        // se esta tecleando).
        let keyboard = crate::nav::keyboard_actions(ctx);
        if self.search_active || self.cover_key_editing {
            actions.extend(
                keyboard.into_iter().filter(|a| matches!(a, NavAction::Back | NavAction::Accept | NavAction::Guide)),
            );
        } else {
            actions.extend(keyboard);
        }

        // F11 no es una accion de navegacion: es un interruptor de ventana.
        if ctx.input(|input| input.key_pressed(egui::Key::F11)) {
            self.toggle_fullscreen();
            self.last_activity = now;
        }

        if !actions.is_empty() || frame.activity {
            self.last_activity = now;
        }
        actions
    }

    fn handle_actions(&mut self, actions: &[NavAction], ctx: &egui::Context) {
        for action in actions {
            // El boton Guia y el menu funcionan desde cualquier sitio, incluso
            // con un juego en marcha.
            match action {
                NavAction::Guide | NavAction::Menu if self.view != View::Quick => {
                    self.quick_focus = 0;
                    self.push_view(View::Quick);
                    if self.session.is_some() {
                        ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(false));
                        ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
                    }
                    continue;
                }
                _ => {}
            }
            self.handle_action(*action, ctx);
        }
    }

    fn handle_action(&mut self, action: NavAction, ctx: &egui::Context) {
        match self.view {
            View::Library => self.handle_library_action(action, ctx),
            View::GameDetail => self.handle_detail_action(action, ctx),
            View::Catalog => self.handle_catalog_action(action),
            View::CatalogEntries => self.handle_entries_action(action),
            View::Downloads => self.handle_downloads_action(action),
            View::Resources => self.handle_resources_action(action),
            View::Settings => self.handle_settings_action(action),
            View::CoverSetup => self.handle_cover_setup_action(action),
            View::Quick => self.handle_quick_action(action, ctx),
            View::Browser => self.handle_browser_action(action),
        }
    }

    fn handle_library_action(&mut self, action: NavAction, ctx: &egui::Context) {
        let len = self.library_view.len();
        match action {
            NavAction::Up
            | NavAction::Down
            | NavAction::Left
            | NavAction::Right
            | NavAction::PageUp
            | NavAction::PageDown => {
                self.library_focus = crate::nav::move_focus(self.library_focus, len, self.columns, action);
            }
            NavAction::Accept => {
                if self.search_active {
                    self.search_active = false;
                } else if len > 0 {
                    self.push_view(View::GameDetail);
                }
            }
            NavAction::Back => {
                if self.search_active {
                    self.search_active = false;
                    self.query.clear();
                    self.mark_library_dirty();
                } else if !self.query.is_empty() {
                    self.query.clear();
                    self.mark_library_dirty();
                }
            }
            NavAction::Favorite => self.toggle_favorite(),
            NavAction::Context => self.open_browser(BrowserPurpose::AddExecutable),
            NavAction::Search => {
                self.search_active = !self.search_active;
            }
            NavAction::TabNext => self.push_view(View::Catalog),
            NavAction::TabPrev => self.cycle_sort(),
            _ => {}
        }
        let _ = ctx;
    }

    fn handle_detail_action(&mut self, action: NavAction, ctx: &egui::Context) {
        match action {
            NavAction::Accept => self.launch_focused(ctx),
            NavAction::Back => self.pop_view(),
            NavAction::Favorite => self.toggle_favorite(),
            NavAction::Context => self.remove_focused(),
            NavAction::Search => {
                if let Some(game) = self.focused_game() {
                    let (id, title) = (game.id.clone(), game.title.clone());
                    self.fetch_cover_now(&id, &title);
                }
            }
            _ => {}
        }
    }

    fn handle_catalog_action(&mut self, action: NavAction) {
        let len = self.catalog.platforms().len();
        match action {
            NavAction::Up
            | NavAction::Down
            | NavAction::Left
            | NavAction::Right
            | NavAction::PageUp
            | NavAction::PageDown => {
                self.platform_focus = crate::nav::move_focus(self.platform_focus, len, self.columns, action);
            }
            NavAction::Accept => {
                if let Some(info) = self.catalog.platforms().get(self.platform_focus) {
                    let id = info.id.clone();
                    self.open_platform(id);
                }
            }
            NavAction::Back => self.pop_view(),
            NavAction::Context => self.open_browser(BrowserPurpose::PickCatalogDir),
            NavAction::TabNext => self.push_view(View::Downloads),
            _ => {}
        }
    }

    fn handle_entries_action(&mut self, action: NavAction) {
        let len = self.entry_hits.len();
        match action {
            NavAction::Up | NavAction::Down | NavAction::PageUp | NavAction::PageDown => {
                self.entry_focus = crate::nav::move_list(self.entry_focus, len, action);
            }
            NavAction::Accept => {
                if self.search_active {
                    self.search_active = false;
                } else {
                    self.download_focused_entry();
                }
            }
            NavAction::Back => {
                if self.search_active {
                    self.search_active = false;
                } else if !self.catalog_query.is_empty() {
                    self.catalog_query.clear();
                    self.refresh_entry_hits();
                } else {
                    self.pop_view();
                }
            }
            NavAction::Search => self.search_active = !self.search_active,
            NavAction::Context => self.open_browser(BrowserPurpose::AddRom),
            NavAction::TabNext => self.push_view(View::Downloads),
            _ => {}
        }
    }

    fn handle_downloads_action(&mut self, action: NavAction) {
        let len = self.downloader.items().len();
        match action {
            NavAction::Up | NavAction::Down => {
                self.downloads_focus = crate::nav::move_list(self.downloads_focus, len, action);
            }
            NavAction::Context => {
                if let Some(item) = self.downloader.items().get(self.downloads_focus) {
                    let id = item.id;
                    self.downloader.cancel(id);
                }
            }
            NavAction::Favorite => self.downloader.clear_finished(),
            NavAction::Back => self.pop_view(),
            _ => {}
        }
    }

    fn handle_resources_action(&mut self, action: NavAction) {
        let len = self.resources.as_ref().map(|scan| scan.services.len()).unwrap_or(0);
        match action {
            NavAction::Up | NavAction::Down | NavAction::PageUp | NavAction::PageDown => {
                self.resource_focus = crate::nav::move_list(self.resource_focus, len, action);
            }
            NavAction::Accept => {
                let name = self
                    .resources
                    .as_ref()
                    .and_then(|scan| scan.services.get(self.resource_focus))
                    .map(|report| report.entry.name);
                if let Some(name) = name {
                    self.toggle_service(name);
                }
            }
            NavAction::Context => self.scan_resources(),
            NavAction::Favorite => self.reset_services(),
            NavAction::Back => self.pop_view(),
            _ => {}
        }
    }

    fn handle_browser_action(&mut self, action: NavAction) {
        let Some(browser) = self.browser.as_mut() else {
            self.pop_view();
            return;
        };
        match action {
            NavAction::Up | NavAction::Down | NavAction::PageUp | NavAction::PageDown => browser.navigate(action),
            NavAction::Accept => {
                if let Some(path) = browser.open_selected() {
                    self.browser_result(path);
                }
            }
            NavAction::Favorite => {
                // En modo carpeta: elegir la carpeta actual.
                if browser.filter == Filter::Directories {
                    if let Some(dir) = browser.current_dir() {
                        self.browser_result(dir);
                    }
                }
            }
            NavAction::Back => {
                if !browser.go_up() {
                    self.browser = None;
                    self.pop_view();
                }
            }
            _ => {}
        }
    }

    fn handle_settings_action(&mut self, action: NavAction) {
        match action {
            NavAction::Up | NavAction::Down => {
                self.settings_focus = crate::nav::move_list(self.settings_focus, crate::views::SETTINGS_ROWS, action);
            }
            NavAction::Left | NavAction::Right | NavAction::Accept => {
                let forward = !matches!(action, NavAction::Left);
                self.adjust_setting(self.settings_focus, forward);
            }
            NavAction::Back => {
                self.save_config();
                self.pop_view();
            }
            _ => {}
        }
    }

    fn handle_cover_setup_action(&mut self, action: NavAction) {
        match action {
            NavAction::Accept => self.cover_key_editing = !self.cover_key_editing,
            NavAction::Context => self.cover_key_paste(),
            NavAction::Favorite => self.test_cover_key(),
            NavAction::TabPrev => self.cover_key_clear(),
            NavAction::TabNext => self.fetch_all_missing_covers(),
            NavAction::Back => {
                if self.cover_key_editing {
                    self.cover_key_editing = false;
                } else {
                    self.save_cover_key();
                }
            }
            _ => {}
        }
    }

    fn handle_quick_action(&mut self, action: NavAction, ctx: &egui::Context) {
        let options = self.quick_options();
        match action {
            NavAction::Up | NavAction::Down => {
                self.quick_focus = crate::nav::move_list(self.quick_focus, options.len(), action);
            }
            NavAction::Accept => {
                let choice = options.get(self.quick_focus).copied();
                self.run_quick_option(choice, ctx);
            }
            NavAction::Back | NavAction::Guide | NavAction::Menu => {
                self.pop_view();
                if self.session.is_some() && self.config.general.minimize_while_playing {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(true));
                }
            }
            _ => {}
        }
    }

    pub fn exit(&mut self, ctx: &egui::Context) {
        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
    }
}

impl eframe::App for App {
    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        let bg = crate::theme::pal().bg;
        [bg.r() as f32 / 255.0, bg.g() as f32 / 255.0, bg.b() as f32 / 255.0, 1.0]
    }

    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let first_frame = self.first_frame;
        if first_frame {
            self.first_frame = false;
            // La peticion de pantalla completa hecha al crear la ventana la
            // ignoran algunos controladores y gestores de ventanas; se repite
            // una vez con la ventana ya viva, que es cuando siempre funciona.
            self.request_window_mode();
        }
        self.apply_window_mode(ctx);
        if first_frame {
            // La ventana nace oculta (ver main.rs) para no ensenar ni una
            // fraccion de segundo el tamano de ventana normal antes de saltar
            // a pantalla completa: se revela justo despues de aplicar el modo
            // de pantalla, asi que el primer frame visible ya es el
            // definitivo.
            ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
        }
        if self.pending_theme_apply {
            crate::theme::install(ctx, self.config.general.ui_scale);
            self.pending_theme_apply = false;
        }

        self.pump(ctx);
        self.refresh_library_view();

        let actions = self.collect_actions(ctx);
        self.handle_actions(&actions, ctx);
        // Una accion puede haber cambiado los filtros.
        self.refresh_library_view();

        self.draw(ctx);

        // Pulso del bucle: lo que marca la diferencia entre un shell que se
        // olvida en segundo plano y uno que se come un nucleo sin hacer nada.
        let fps = match self.poll_mode() {
            PollMode::InGame => self.config.general.background_fps,
            PollMode::Idle => self.config.general.idle_fps,
            PollMode::Active => self.config.general.active_fps,
        };
        ctx.request_repaint_after(Duration::from_secs_f32(1.0 / fps.max(1) as f32));
    }

    fn on_exit(&mut self) {
        self.input.stop_rumble();
        // Nunca se sale dejando el sistema tocado.
        if self.config.power.restore_on_exit {
            if let Ok(mut optimizer) = self.optimizer.lock() {
                if optimizer.is_engaged() {
                    let report = optimizer.restore();
                    log::info!("restaurado al salir: {}", report.summary());
                }
            }
        }
        let _ = self.library.save();
        let _ = self.config.save();
    }
}
