//! Estado y bucle principal del shell.
//!
//! El bucle esta disenado alrededor de una idea: **el shell no debe consumir
//! nada cuando no esta haciendo nada**. egui solo repinta cuando se le pide, y
//! aqui se le pide a un ritmo que depende de lo que este pasando (navegando,
//! en reposo o con un juego en marcha).

use std::collections::{HashSet, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, Sender};
use std::sync::{mpsc, Arc, Mutex};
use std::time::{Duration, Instant};

use crate::covers::CoverTextures;
use gm_core::config::Config;
use gm_core::launcher::{self, Session};
use gm_core::library::{Game, Launch, Library, Sort};
use gm_input::{InputHub, NavAction, PollMode};
use gm_power::{MemoryStatus, Optimizer, Report, SystemScan};

use crate::browser::{Browser, Filter};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum View {
    Library,
    Favorites,
    GameDetail,
    /// Que esta consumiendo el equipo y que se va a hacer al respecto.
    Resources,
    Settings,
    /// Alta de la clave de SteamGridDB, con explicacion, pegado y prueba.
    CoverSetup,
    /// Panel rapido del boton Guia; funciona tambien con un juego en marcha.
    Quick,
    Browser,
    /// Revision, exe por exe, de lo que se va a anadir tras elegir una
    /// carpeta a importar: nada entra en la biblioteca sin que el usuario lo
    /// marque aqui primero.
    ImportReview,
}

/// Un `.exe` encontrado al importar una carpeta, pendiente de confirmar.
#[derive(Debug, Clone)]
pub struct ImportCandidate {
    pub path: PathBuf,
    pub title: String,
    /// Se anade a la biblioteca si se confirma con esto marcado.
    pub checked: bool,
    /// Pinta de instalador/redistribuible (nombre tipo `setup`, `unins000`...);
    /// entra desmarcado de serie, pero se puede marcar a mano si hace falta.
    pub installer: bool,
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

/// Una busqueda de caratula pendiente de lanzar.
struct CoverRequest {
    game_id: String,
    title: String,
    notify: bool,
}

/// Garantiza que `spawn_cover_fetch` manda algo por el canal pase lo que
/// pase, incluido un panico dentro de `fetch_cover`: sin esto, `cover_active`
/// no bajaria nunca para esa peticion y, agotado el cupo de tres a la vez,
/// las descargas de caratula se quedarian paradas para siempre sin que nada
/// lo avisara.
struct CoverFetchGuard {
    tx: Sender<CoverFetchResult>,
    title: String,
    notify: bool,
    sent: bool,
}

impl CoverFetchGuard {
    fn finish(mut self, result: CoverFetchResult) {
        self.sent = true;
        let _ = self.tx.send(result);
    }
}

impl Drop for CoverFetchGuard {
    fn drop(&mut self) {
        if !self.sent {
            let _ = self.tx.send(CoverFetchResult::NotFound {
                title: std::mem::take(&mut self.title),
                error: "fallo interno al buscar la caratula".to_string(),
                notify: self.notify,
            });
        }
    }
}

/// Cuantas busquedas de caratula corren a la vez como mucho. "Descargar las
/// que faltan" puede encolar toda la biblioteca de golpe; sin este limite
/// serian otros tantos hilos e igual de tantas peticiones simultaneas a
/// SteamGridDB.
const MAX_CONCURRENT_COVER_FETCHES: usize = 3;

/// Para que se abrio el explorador de ficheros.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BrowserPurpose {
    AddExecutable,
    /// Analiza una carpeta entera (y sus subcarpetas) y anade cada `.exe`
    /// que encuentra a la biblioteca de una vez.
    ImportFolder,
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

pub struct App {
    pub config: Config,
    pub library: Library,
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
    pub search_active: bool,
    /// Ficha de un juego con el titulo en edicion: util para dejar un nombre
    /// mas parecido al oficial antes de buscarle caratula en SteamGridDB,
    /// sin salir del modo juego a tocar ningun fichero a mano.
    pub renaming: bool,
    pub rename_draft: String,

    // Explorador de ficheros
    pub browser: Option<Browser>,
    pub browser_purpose: BrowserPurpose,

    // Revision de una importacion de carpeta pendiente de confirmar
    pub import_candidates: Vec<ImportCandidate>,
    pub import_focus: usize,

    // Menus
    pub quick_focus: usize,
    pub settings_focus: usize,

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
    pub columns: usize,

    // Caratulas (SteamGridDB)
    pub covers: CoverTextures,
    /// Juegos para los que ya se intento buscar caratula esta sesion, con
    /// exito o sin el: evita volver a pedirla solo porque la tarjeta ha
    /// vuelto a entrar en pantalla al desplazar la rejilla.
    cover_attempted: HashSet<String>,
    /// Peticiones pendientes de lanzar: "Descargar las que faltan" en una
    /// biblioteca grande puede encolar cientos de una vez, y no deben salir
    /// todas como hilos simultaneos (satura la maquina un instante y es facil
    /// que SteamGridDB responda 429). Se lanzan de a
    /// `MAX_CONCURRENT_COVER_FETCHES`, igual que hace el descargador de ROMs.
    cover_queue: VecDeque<CoverRequest>,
    cover_active: usize,
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
        let input = InputHub::new(config.input.clone());
        let optimizer = Optimizer::new(config.power.clone());
        let (power_tx, power_rx) = mpsc::channel();
        let (resource_tx, resource_rx) = mpsc::channel();
        let (cover_tx, cover_rx) = mpsc::channel();
        let (cover_test_tx, cover_test_rx) = mpsc::channel();

        let mut app = Self {
            config,
            library,
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
            search_active: false,
            renaming: false,
            rename_draft: String::new(),
            browser: None,
            browser_purpose: BrowserPurpose::AddExecutable,
            import_candidates: Vec::new(),
            import_focus: 0,
            quick_focus: 0,
            settings_focus: 0,
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
            // Resta saturada: en Windows `Instant` cuenta desde el arranque
            // del sistema, y un PC recien encendido (el caso de uso tipico de
            // un shell de salon) puede llevar menos de 60 s vivo. Con `-`
            // normal eso hace panic; aqui, en el peor caso, la primera
            // lectura de memoria simplemente se retrasa 1.5 s en vez de
            // hacerse en el primer frame.
            last_memory_check: Instant::now().checked_sub(Duration::from_secs(60)).unwrap_or_else(Instant::now),
            last_activity: Instant::now(),
            input_source: crate::nav::InputSource::KeyboardMouse,
            columns: 5,
            covers: CoverTextures::default(),
            cover_attempted: HashSet::new(),
            cover_queue: VecDeque::new(),
            cover_active: 0,
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
        self.cancel_rename();
    }

    pub fn pop_view(&mut self) {
        self.view = self.stack.pop().unwrap_or(View::Library);
        self.cancel_rename();
    }

    /// Cambia de vista principal vaciando la pila de navegacion.
    pub fn set_root_view(&mut self, view: View) {
        self.stack.clear();
        self.view = view;
        self.cancel_rename();
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
        self.library_view = self.library.view(&self.query, None, self.sort, self.view == View::Favorites);
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
                // Un atajo lo abre el sistema por su cuenta: no queda proceso
                // hijo al que seguirle la pista, asi que el modo juego no
                // puede darse cuenta solo de cuando se cierra. Un aviso aqui
                // no serviria de nada: sale justo antes de minimizar, asi que
                // nadie llega a leerlo. En vez de eso, `quick.rs` deja la
                // nota donde si se ve: en el propio panel que sirve para
                // terminar la sesion a mano.
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

    /// Abre el titulo del juego enfocado para edicion. Pensado sobre todo
    /// para arreglar el nombre que salio de un .exe ("Mi Juego v1.2.3-win64")
    /// antes de pedirle caratula a SteamGridDB, que busca por titulo: un
    /// nombre mas parecido al oficial es la diferencia entre encontrarla o no.
    pub fn begin_rename(&mut self) {
        let Some(game) = self.focused_game() else { return };
        self.rename_draft = game.title.clone();
        self.renaming = true;
    }

    /// Descarta la edicion sin tocar el titulo guardado.
    pub fn cancel_rename(&mut self) {
        self.renaming = false;
        self.rename_draft.clear();
    }

    pub fn confirm_rename(&mut self) {
        let Some(game) = self.focused_game() else {
            self.cancel_rename();
            return;
        };
        let id = game.id.clone();
        let title = self.rename_draft.trim().to_string();
        if title.is_empty() {
            self.toast(ToastKind::Bad, "El titulo no puede quedar vacio");
            return;
        }
        if self.library.rename(&id, title.clone()) {
            self.save_library();
            // Un titulo nuevo no vale como intento fallido del viejo: si el
            // renombrado era justo para que SteamGridDB lo reconociera, la
            // busqueda automatica tiene que poder volver a intentarlo.
            self.cover_attempted.remove(&id);
            self.toast(ToastKind::Info, format!("Renombrado a \"{title}\""));
            // El titulo nuevo puede cambiar el orden (si se ordena por
            // titulo) o sacar al juego de una busqueda activa: se
            // reconstruye la vista aqui mismo, en vez de esperar al proximo
            // refresco perezoso, para poder reanclar el foco al id -no a la
            // posicion vieja, que ahora podria apuntar a otro juego distinto.
            self.library_view = self.library.view(&self.query, None, self.sort, self.view == View::Favorites);
            self.library_dirty = false;
            let still_visible =
                self.library_view.iter().position(|&index| self.library.games.get(index).is_some_and(|g| g.id == id));
            match still_visible {
                Some(position) => self.library_focus = position,
                None => self.pop_view(),
            }
        }
        self.cancel_rename();
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
            BrowserPurpose::ImportFolder => Filter::Directories,
        };
        self.browser_purpose = purpose;
        self.browser = Some(Browser::new(filter, None));
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
            BrowserPurpose::ImportFolder => {
                let candidates = self.scan_exe_candidates(&path);
                if candidates.is_empty() {
                    self.toast(ToastKind::Info, "No se encontro ningun .exe nuevo en esa carpeta");
                    self.pop_view();
                    return;
                }
                self.import_candidates = candidates;
                self.import_focus = 0;
                self.set_root_view(View::ImportReview);
            }
        }
    }

    /// Nombres de ejecutables que casi nunca son el juego en si (instaladores,
    /// redistribuibles, desinstaladores...): se destacan en la lista de
    /// revision, pero siguen pudiendo marcarse a mano si de verdad hacen falta.
    fn looks_like_installer(stem: &str) -> bool {
        const NOISE: &[&str] = &[
            "unins", "setup", "install", "vcredist", "vc_redist", "dxsetup", "directx", "dotnetfx", "redist",
            "prereq", "crashpad_handler", "ue4prereqsetup", "ueprereqsetup",
        ];
        let lower = stem.to_lowercase();
        NOISE.iter().any(|needle| lower.contains(needle))
    }

    /// Recorre `dir` (y sus subcarpetas, hasta una profundidad razonable)
    /// buscando `.exe` que todavia no esten en la biblioteca. No anade nada
    /// todavia: solo arma la lista que se revisa, exe por exe, en
    /// `View::ImportReview` antes de que el usuario confirme que se sube.
    fn scan_exe_candidates(&self, dir: &Path) -> Vec<ImportCandidate> {
        const MAX_DEPTH: u32 = 6;
        let mut candidates = Vec::new();
        let mut pending = vec![(dir.to_path_buf(), 0u32)];

        while let Some((current, depth)) = pending.pop() {
            let Ok(entries) = std::fs::read_dir(&current) else { continue };
            for entry in entries.flatten() {
                let path = entry.path();
                let is_dir = entry.file_type().map(|t| t.is_dir()).unwrap_or(false);
                if is_dir {
                    if depth < MAX_DEPTH {
                        pending.push((path, depth + 1));
                    }
                    continue;
                }
                if path.extension().and_then(|e| e.to_str()).map(|e| e.eq_ignore_ascii_case("exe")) != Some(true) {
                    continue;
                }
                if self.library.games.iter().any(|g| matches!(&g.launch, Launch::Executable { path: p, .. } if p == &path))
                {
                    continue; // ya esta en la biblioteca: no hace falta volver a preguntar.
                }
                let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or_default();
                let installer = Self::looks_like_installer(stem);
                let title = Game::title_from_path(&path);
                // Los que parecen instalador entran sin marcar: hay que
                // quererlos a proposito, no colarse por defecto.
                candidates.push(ImportCandidate { path, title, checked: !installer, installer });
            }
        }
        candidates.sort_by(|a, b| a.title.to_lowercase().cmp(&b.title.to_lowercase()));
        candidates
    }

    /// Anade a la biblioteca los candidatos marcados en `View::ImportReview`.
    pub fn confirm_import(&mut self) {
        let checked: Vec<ImportCandidate> = self.import_candidates.drain(..).filter(|c| c.checked).collect();
        if checked.is_empty() {
            self.toast(ToastKind::Info, "No se marco ningun .exe");
            self.pop_view();
            return;
        }
        let mut added = 0;
        for candidate in checked {
            let mut game = Game::new(
                candidate.title,
                Launch::Executable { path: candidate.path, args: Vec::new(), working_dir: None },
            );
            game.collection = "PC".to_string();
            if self.library.add(game) {
                added += 1;
            }
        }
        self.save_library();
        self.mark_library_dirty();
        self.toast(ToastKind::Good, format!("{added} juego(s) anadidos a la biblioteca"));
        self.pop_view();
    }

    pub fn cancel_import(&mut self) {
        self.import_candidates.clear();
        self.pop_view();
    }

    pub fn toggle_import_focused(&mut self) {
        if let Some(candidate) = self.import_candidates.get_mut(self.import_focus) {
            candidate.checked = !candidate.checked;
        }
    }

    /// Marca o desmarca todos de golpe: si ya estaban todos marcados, los
    /// desmarca; si no, los marca todos. Para carpetas con decenas de
    /// candidatos, marcar uno a uno para quitar solo un par no es razonable.
    pub fn toggle_import_all(&mut self) {
        let all_checked = self.import_candidates.iter().all(|c| c.checked);
        for candidate in &mut self.import_candidates {
            candidate.checked = !all_checked;
        }
    }

    /// La clave configurada, si hay alguna que no sea solo espacios.
    fn cover_key(&self) -> Option<String> {
        self.config.covers.steamgrid_api_key.clone().filter(|key| !key.trim().is_empty())
    }

    /// Lanza la busqueda de una caratula en un hilo aparte, sin pasar por la
    /// cola: solo se llama desde `pump_cover_queue`, que ya ha comprobado el
    /// limite de concurrencia. No abre ninguna ventana ni dialogo: el
    /// resultado vuelve por el canal y `pump()` lo recoge, avisando en
    /// pantalla solo si `notify` esta activo.
    fn spawn_cover_fetch(&self, api_key: String, game_id: String, title: String, notify: bool) {
        let dest_dir = gm_core::paths::covers_dir();
        let tx = self.cover_tx.clone();
        std::thread::spawn(move || {
            let guard = CoverFetchGuard { tx: tx.clone(), title: title.clone(), notify, sent: false };
            let result = match gm_catalog::steamgrid::fetch_cover(&api_key, &title, &game_id, &dest_dir) {
                Ok(path) => CoverFetchResult::Found { game_id, title, path, notify },
                Err(e) => CoverFetchResult::NotFound { title, error: e.to_string(), notify },
            };
            guard.finish(result);
        });
    }

    /// Encola una busqueda de caratula. Puede tardar en arrancar si ya hay
    /// `MAX_CONCURRENT_COVER_FETCHES` en marcha: `pump_cover_queue` la lanza
    /// en cuanto haya hueco.
    fn enqueue_cover_fetch(&mut self, game_id: &str, title: &str, notify: bool) {
        self.cover_queue.push_back(CoverRequest { game_id: game_id.to_string(), title: title.to_string(), notify });
    }

    /// Se llama una vez por frame: lanza peticiones encoladas hasta el limite
    /// de concurrencia. Si la clave desaparecio a media cola (el usuario la
    /// borro mientras habia busquedas pendientes), se descarta el resto en
    /// silencio, ya no hay con que buscar.
    fn pump_cover_queue(&mut self) {
        while self.cover_active < MAX_CONCURRENT_COVER_FETCHES {
            let Some(request) = self.cover_queue.pop_front() else { break };
            let Some(api_key) = self.cover_key() else {
                self.cover_queue.clear();
                break;
            };
            self.cover_active += 1;
            self.spawn_cover_fetch(api_key, request.game_id, request.title, request.notify);
        }
    }

    /// Busca la caratula de un juego que acaba de aparecer en pantalla, sin
    /// que el usuario haya pedido nada: silenciosa, y solo una vez por
    /// partida mientras el shell este abierto (si falla, no se reintenta solo
    /// por volver a desplazar la rejilla hasta esa tarjeta).
    pub fn maybe_fetch_cover(&mut self, game_id: &str, title: &str) {
        if !self.config.covers.auto_fetch || self.cover_attempted.contains(game_id) {
            return;
        }
        if self.cover_key().is_none() {
            return;
        }
        self.cover_attempted.insert(game_id.to_string());
        self.enqueue_cover_fetch(game_id, title, false);
    }

    /// Busca (o vuelve a buscar) la caratula de un juego concreto porque el
    /// usuario lo ha pedido -boton "Buscar caratula" de la ficha-, avisando
    /// del resultado aunque sea un fallo: es la via para forzar un reintento
    /// cuando la busqueda automatica no encontro nada (titulo raro sacado del
    /// nombre del .exe, por ejemplo).
    pub fn fetch_cover_now(&mut self, game_id: &str, title: &str) {
        if self.cover_key().is_none() {
            self.toast(ToastKind::Bad, "Antes configura una clave en Ajustes → Caratulas automaticas");
            return;
        }
        self.toast(ToastKind::Info, format!("Buscando caratula de \"{title}\"..."));
        self.cover_attempted.insert(game_id.to_string());
        self.enqueue_cover_fetch(game_id, title, true);
    }

    /// Encola una busqueda para cada juego de la biblioteca que todavia no
    /// tiene caratula. Es el boton "Descargar las que faltan" de la pantalla
    /// de SteamGridDB: el motivo de tener la clave puesta es justo este, no
    /// quedarse esperando a que cada juego pase por la rejilla. Se lanzan de
    /// pocas en pocas (`MAX_CONCURRENT_COVER_FETCHES`), no todas de golpe.
    pub fn fetch_all_missing_covers(&mut self) {
        if self.cover_key().is_none() {
            self.toast(ToastKind::Bad, "Antes configura una clave en Ajustes → Caratulas automaticas");
            return;
        }
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
            self.enqueue_cover_fetch(&game_id, &title, false);
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

    /// Sale de la pantalla sin guardar nada: la forma de arrepentirse de un
    /// cambio en el borrador -sobre todo de un "Borrar" pulsado sin querer,
    /// que hasta aqui solo se podia deshacer volviendo a escribir la clave a
    /// mano- antes de que Atras lo convierta en definitivo. La clave que
    /// hubiera guardada, si la habia, se queda exactamente como estaba.
    pub fn cancel_cover_setup(&mut self) {
        self.cover_key_editing = false;
        self.cover_key_test = KeyTestState::Idle;
        self.pop_view();
    }

    // ------------------------------------------------------------------
    // Trabajo de fondo
    // ------------------------------------------------------------------

    fn pump(&mut self, ctx: &egui::Context) {
        let now = Instant::now();

        // Decodificados de caratula pendientes: la lectura y el redimensionado
        // corren en hilos aparte (ver covers.rs), aqui solo se suben a la GPU
        // los que ya han terminado.
        self.covers.pump(ctx);

        // Caratulas de SteamGridDB que acaban de llegar (o de fallar).
        let mut cover_arrived = false;
        while let Ok(result) = self.cover_rx.try_recv() {
            self.cover_active = self.cover_active.saturating_sub(1);
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
        // Hueco libre por lo que acaba de terminar (o por lo recien encolado
        // arriba en esta misma vuelta): lanzar lo siguiente de la cola.
        self.pump_cover_queue();
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

        // Memoria para la barra superior, dos veces por segundo como mucho.
        if now.duration_since(self.last_memory_check) > Duration::from_millis(1500) {
            self.last_memory_check = now;
            if let Ok(status) = gm_power::memory_status() {
                self.memory = status;
            }
        }

        self.toasts.retain(|toast| toast.until > now);
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
        let text_editing = self.search_active || self.cover_key_editing || self.renaming;
        actions.extend(crate::nav::keyboard_actions(ctx, text_editing));

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
            match action {
                // El boton Guia (el central del mando, o F12) es el unico
                // pensado para abrir el panel encima de un juego en marcha:
                // usa un boton que XInput no reporta por la via normal, asi
                // que ningun juego lo tiene ya asignado a su pausa.
                NavAction::Guide if self.view != View::Quick => {
                    self.quick_focus = 0;
                    self.push_view(View::Quick);
                    if self.session.is_some() {
                        ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(false));
                        ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
                    }
                    continue;
                }
                // Start/F1 tambien abre el panel, pero solo fuera de partida:
                // con un juego en marcha, Start es su pausa, no la nuestra.
                NavAction::Menu if self.view != View::Quick && self.session.is_none() => {
                    self.quick_focus = 0;
                    self.push_view(View::Quick);
                    continue;
                }
                _ => {}
            }
            self.handle_action(*action, ctx);
        }
    }

    fn handle_action(&mut self, action: NavAction, ctx: &egui::Context) {
        match self.view {
            View::Library | View::Favorites => self.handle_library_action(action, ctx),
            View::GameDetail => self.handle_detail_action(action, ctx),
            View::Resources => self.handle_resources_action(action),
            View::Settings => self.handle_settings_action(action),
            View::CoverSetup => self.handle_cover_setup_action(action),
            View::Quick => self.handle_quick_action(action, ctx),
            View::Browser => self.handle_browser_action(action),
            View::ImportReview => self.handle_import_review_action(action),
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
            NavAction::TabNext => {
                let other = if self.view == View::Favorites { View::Library } else { View::Favorites };
                self.mark_library_dirty();
                self.set_root_view(other);
            }
            NavAction::TabPrev => self.cycle_sort(),
            _ => {}
        }
        let _ = ctx;
    }

    fn handle_detail_action(&mut self, action: NavAction, ctx: &egui::Context) {
        if self.renaming {
            match action {
                NavAction::Accept => self.confirm_rename(),
                NavAction::Back => self.cancel_rename(),
                _ => {}
            }
            return;
        }
        match action {
            NavAction::Accept => self.launch_focused(ctx),
            NavAction::Back => self.pop_view(),
            NavAction::Favorite => self.toggle_favorite(),
            NavAction::Context => self.remove_focused(),
            NavAction::TabPrev => self.begin_rename(),
            NavAction::Search => {
                if let Some(game) = self.focused_game() {
                    let (id, title) = (game.id.clone(), game.title.clone());
                    self.fetch_cover_now(&id, &title);
                }
            }
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

    fn handle_import_review_action(&mut self, action: NavAction) {
        let len = self.import_candidates.len();
        match action {
            NavAction::Up | NavAction::Down | NavAction::PageUp | NavAction::PageDown => {
                self.import_focus = crate::nav::move_list(self.import_focus, len, action);
            }
            NavAction::Accept => self.toggle_import_focused(),
            NavAction::Favorite => self.toggle_import_all(),
            NavAction::Context => self.confirm_import(),
            NavAction::Back => self.cancel_import(),
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
            NavAction::Search => self.cancel_cover_setup(),
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
        let mode = self.poll_mode();
        let fps = match mode {
            PollMode::InGame => self.config.general.background_fps,
            PollMode::Idle => self.config.general.idle_fps,
            PollMode::Active => self.config.general.active_fps,
        };
        let mut interval = Duration::from_secs_f32(1.0 / fps.max(1) as f32);
        if mode == PollMode::InGame {
            // XInput reporta el estado en el instante de la lectura, no una
            // cola de eventos: con `background_fps` bajo (1 Hz de serie) el
            // boton Guia se puede pulsar y soltar entero entre dos sondeos y
            // no queda ni rastro del flanco, se pierde la pulsacion entera,
            // no solo se retrasa. Se acota el intervalo al que ya calculaba
            // `InputHub::poll_interval` para este modo (10 Hz) para que el
            // mando se siga sondeando a un ritmo fiable aunque el repintado
            // en si se pida mas lento; la ventana esta minimizada
            // en este modo, asi que el coste real es sondear un mando, no
            // dibujar un frame completo.
            interval = interval.min(self.input.poll_interval(PollMode::InGame));
        }
        ctx.request_repaint_after(interval);
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
