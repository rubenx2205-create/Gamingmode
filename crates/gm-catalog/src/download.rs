//! Descargas del catalogo, con reanudacion y cola acotada.
//!
//! Cada descarga vive en su propio hilo y publica su progreso en un
//! `Arc<Mutex<Progress>>`: la interfaz lo lee una vez por frame y no se bloquea
//! nunca. En Windows el transporte usa schannel (la pila TLS del sistema), asi
//! que no hay ninguna libreria de criptografia dentro del binario.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DownloadState {
    Queued,
    Connecting,
    Running,
    Done,
    Cancelled,
    Failed(String),
}

impl DownloadState {
    pub fn is_terminal(&self) -> bool {
        matches!(self, DownloadState::Done | DownloadState::Cancelled | DownloadState::Failed(_))
    }

    pub fn is_active(&self) -> bool {
        matches!(self, DownloadState::Connecting | DownloadState::Running)
    }
}

#[derive(Debug, Clone)]
pub struct Progress {
    pub state: DownloadState,
    pub done: u64,
    pub total: Option<u64>,
    pub speed_bps: u64,
}

impl Default for Progress {
    fn default() -> Self {
        Self { state: DownloadState::Queued, done: 0, total: None, speed_bps: 0 }
    }
}

impl Progress {
    /// Fraccion 0.0-1.0 si se conoce el tamano total.
    pub fn fraction(&self) -> Option<f32> {
        match self.total {
            Some(total) if total > 0 => Some((self.done as f32 / total as f32).clamp(0.0, 1.0)),
            _ => None,
        }
    }

    pub fn describe(&self) -> String {
        use gm_core::util::format_bytes;
        match &self.state {
            DownloadState::Queued => "en cola".to_string(),
            DownloadState::Connecting => "conectando...".to_string(),
            DownloadState::Running => match self.total {
                Some(total) => format!(
                    "{} / {} ({}/s)",
                    format_bytes(self.done),
                    format_bytes(total),
                    format_bytes(self.speed_bps)
                ),
                None => format!("{} ({}/s)", format_bytes(self.done), format_bytes(self.speed_bps)),
            },
            DownloadState::Done => format!("completada ({})", format_bytes(self.done)),
            DownloadState::Cancelled => "cancelada".to_string(),
            DownloadState::Failed(e) => format!("error: {e}"),
        }
    }
}

pub struct Download {
    pub id: u64,
    pub label: String,
    pub url: String,
    pub dest: PathBuf,
    progress: Arc<Mutex<Progress>>,
    cancel: Arc<AtomicBool>,
}

impl Download {
    pub fn snapshot(&self) -> Progress {
        self.progress.lock().map(|p| p.clone()).unwrap_or_default()
    }

    pub fn cancel(&self) {
        self.cancel.store(true, Ordering::Relaxed);
    }
}

/// Cola de descargas con un maximo de transferencias simultaneas: saturar la
/// linea con diez descargas a la vez no descarga antes y si hace que el juego
/// que se este jugando note el ruido de red.
pub struct Downloader {
    items: Vec<Download>,
    next_id: u64,
    max_concurrent: usize,
    #[cfg(windows)]
    agent: Option<ureq::Agent>,
}

impl Downloader {
    pub fn new(max_concurrent: usize) -> Self {
        Self {
            items: Vec::new(),
            next_id: 1,
            max_concurrent: max_concurrent.clamp(1, 8),
            #[cfg(windows)]
            agent: build_agent(),
        }
    }

    pub fn items(&self) -> &[Download] {
        &self.items
    }

    pub fn active_count(&self) -> usize {
        self.items.iter().filter(|d| d.snapshot().state.is_active()).count()
    }

    pub fn is_available(&self) -> bool {
        #[cfg(windows)]
        {
            self.agent.is_some()
        }
        #[cfg(not(windows))]
        {
            false
        }
    }

    /// Encola una descarga. Devuelve su id.
    pub fn enqueue(&mut self, label: impl Into<String>, url: impl Into<String>, dest: PathBuf) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        self.items.push(Download {
            id,
            label: label.into(),
            url: url.into(),
            dest,
            progress: Arc::new(Mutex::new(Progress::default())),
            cancel: Arc::new(AtomicBool::new(false)),
        });
        self.pump();
        id
    }

    pub fn cancel(&mut self, id: u64) {
        if let Some(item) = self.items.iter().find(|d| d.id == id) {
            item.cancel();
        }
    }

    /// Quita de la lista las descargas ya terminadas.
    pub fn clear_finished(&mut self) {
        self.items.retain(|d| !d.snapshot().state.is_terminal());
    }

    /// Arranca las descargas en cola que quepan. Se llama una vez por frame.
    pub fn pump(&mut self) {
        let mut running = self.active_count();
        for item in &self.items {
            if running >= self.max_concurrent {
                break;
            }
            if item.snapshot().state != DownloadState::Queued {
                continue;
            }
            set_state(&item.progress, DownloadState::Connecting);
            running += 1;
            self.start(item);
        }
    }

    #[cfg(windows)]
    fn start(&self, item: &Download) {
        let Some(agent) = self.agent.clone() else {
            set_state(&item.progress, DownloadState::Failed("no hay transporte TLS disponible".into()));
            return;
        };
        let url = item.url.clone();
        let dest = item.dest.clone();
        let progress = Arc::clone(&item.progress);
        let cancel = Arc::clone(&item.cancel);
        std::thread::spawn(move || {
            if let Err(e) = run_download(&agent, &url, &dest, &progress, &cancel) {
                set_state(&progress, DownloadState::Failed(e));
            }
        });
    }

    #[cfg(not(windows))]
    fn start(&self, item: &Download) {
        set_state(&item.progress, DownloadState::Failed("las descargas solo estan disponibles en Windows".into()));
    }
}

fn set_state(progress: &Arc<Mutex<Progress>>, state: DownloadState) {
    if let Ok(mut guard) = progress.lock() {
        guard.state = state;
    }
}

/// Ruta del fichero temporal: se descarga a `.part` y solo al terminar se
/// renombra, para que una descarga a medias nunca parezca una ROM valida.
pub fn part_path(dest: &Path) -> PathBuf {
    let mut name = dest.as_os_str().to_os_string();
    name.push(".part");
    PathBuf::from(name)
}

/// Cliente HTTP compartido: la conexion TLS via schannel (la pila del propio
/// Windows) es la unica que necesita el binario, tanto para descargar ROMs
/// como para hablar con SteamGridDB.
#[cfg(windows)]
pub(crate) fn build_agent() -> Option<ureq::Agent> {
    match native_tls::TlsConnector::new() {
        Ok(connector) => Some(
            ureq::AgentBuilder::new()
                .tls_connector(Arc::new(connector))
                .timeout_connect(std::time::Duration::from_secs(20))
                .user_agent(concat!("GamingMode/", env!("CARGO_PKG_VERSION")))
                .build(),
        ),
        Err(e) => {
            log::error!("no se pudo inicializar TLS del sistema: {e}");
            None
        }
    }
}

#[cfg(windows)]
fn run_download(
    agent: &ureq::Agent,
    url: &str,
    dest: &Path,
    progress: &Arc<Mutex<Progress>>,
    cancel: &Arc<AtomicBool>,
) -> Result<(), String> {
    use std::io::{Read, Seek, Write};

    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let part = part_path(dest);
    let resume_from = std::fs::metadata(&part).map(|m| m.len()).unwrap_or(0);

    let mut request = agent.get(url);
    if resume_from > 0 {
        request = request.set("Range", &format!("bytes={resume_from}-"));
    }
    let response = request.call().map_err(|e| e.to_string())?;

    // Si el servidor ignora el Range devuelve 200 y hay que empezar de cero.
    let resuming = response.status() == 206 && resume_from > 0;
    let body_len = response.header("Content-Length").and_then(|v| v.parse::<u64>().ok());
    let total = body_len.map(|len| if resuming { len + resume_from } else { len });

    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(!resuming)
        .open(&part)
        .map_err(|e| e.to_string())?;
    let mut done = if resuming { file.seek(std::io::SeekFrom::End(0)).map_err(|e| e.to_string())? } else { 0 };

    if let Ok(mut guard) = progress.lock() {
        guard.state = DownloadState::Running;
        guard.done = done;
        guard.total = total;
    }

    let mut reader = response.into_reader();
    let mut buffer = vec![0u8; 64 * 1024];
    let mut window_start = std::time::Instant::now();
    let mut window_bytes = 0u64;

    loop {
        if cancel.load(Ordering::Relaxed) {
            let _ = file.flush();
            set_state(progress, DownloadState::Cancelled);
            // El `.part` se conserva: la descarga se puede reanudar despues.
            return Ok(());
        }
        let read = reader.read(&mut buffer).map_err(|e| e.to_string())?;
        if read == 0 {
            break;
        }
        file.write_all(&buffer[..read]).map_err(|e| e.to_string())?;
        done += read as u64;
        window_bytes += read as u64;

        let elapsed = window_start.elapsed();
        if elapsed >= std::time::Duration::from_millis(500) {
            let speed = (window_bytes as f64 / elapsed.as_secs_f64()) as u64;
            if let Ok(mut guard) = progress.lock() {
                guard.done = done;
                guard.speed_bps = speed;
            }
            window_start = std::time::Instant::now();
            window_bytes = 0;
        }
    }

    file.flush().map_err(|e| e.to_string())?;
    drop(file);
    if dest.exists() {
        std::fs::remove_file(dest).map_err(|e| e.to_string())?;
    }
    std::fs::rename(&part, dest).map_err(|e| e.to_string())?;

    if let Ok(mut guard) = progress.lock() {
        guard.state = DownloadState::Done;
        guard.done = done;
        guard.total = Some(done);
        guard.speed_bps = 0;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn el_temporal_no_pisa_la_extension_original() {
        let dest = Path::new("C:/roms/Super Mario World.sfc");
        assert_eq!(part_path(dest).file_name().unwrap(), "Super Mario World.sfc.part");
    }

    #[test]
    fn la_cola_respeta_el_maximo_de_simultaneas() {
        let downloader = Downloader::new(99);
        assert_eq!(downloader.max_concurrent, 8, "se recorta a un maximo sensato");
        let downloader = Downloader::new(0);
        assert_eq!(downloader.max_concurrent, 1);
    }

    #[test]
    fn el_progreso_se_describe_para_la_interfaz() {
        let mut progress = Progress {
            state: DownloadState::Running,
            done: 512 * 1024,
            total: Some(1024 * 1024),
            speed_bps: 256 * 1024,
        };
        assert_eq!(progress.fraction(), Some(0.5));
        assert!(progress.describe().contains("512.0 KB / 1.0 MB"));

        progress.total = None;
        assert_eq!(progress.fraction(), None);
        progress.state = DownloadState::Failed("sin red".into());
        assert_eq!(progress.describe(), "error: sin red");
    }

    #[test]
    fn fuera_de_windows_la_descarga_falla_con_un_mensaje_claro() {
        let mut downloader = Downloader::new(2);
        let id = downloader.enqueue("Prueba", "https://example.org/x.zip", PathBuf::from("/tmp/x.zip"));
        let item = downloader.items().iter().find(|d| d.id == id).unwrap();
        let state = item.snapshot().state;
        if cfg!(windows) {
            assert!(state.is_active() || state.is_terminal());
        } else {
            assert!(matches!(state, DownloadState::Failed(_)));
        }
        downloader.clear_finished();
        assert!(downloader.items().is_empty() || cfg!(windows));
    }
}
