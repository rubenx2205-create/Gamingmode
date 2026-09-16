//! Carga de caratulas reales como textura de egui.
//!
//! Decodificar y redimensionar una imagen de varios cientos de KB tarda del
//! orden de milisegundos: poco para una sola, pero bastante para notarse en
//! el hilo de dibujado si la biblioteca tiene cientos de juegos y todos
//! entran en pantalla a la vez. Aqui se hace en hilos aparte, acotados en
//! numero, y solo el resultado ya decodificado -unos pocos cientos de KB de
//! RGBA a resolucion de tarjeta, no los megabytes originales- cruza de vuelta
//! al hilo principal para subirse a la GPU con `Context::load_texture`.

use std::collections::{HashMap, HashSet, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, Sender};

use egui::{ColorImage, TextureHandle, TextureOptions};

/// Lado mas largo al que se redimensiona cualquier caratula antes de subirla
/// a la GPU. Una tarjeta se dibuja a 190x268 puntos; 512 px de lado cubre de
/// sobra incluso con la escala de interfaz al maximo (1.5x) mas una pantalla
/// de alta densidad, sin arrastrar los ~600x900 (2 MB sin comprimir) que
/// entrega SteamGridDB.
const MAX_SIDE: u32 = 512;

/// Techo de texturas decodificadas a la vez. Por encima de eso se desaloja la
/// que lleve mas tiempo sin pedirse: una biblioteca de miles de juegos no
/// debe poder comerse VRAM sin limite.
const MAX_CACHED: usize = 200;

/// Cuantas decodificaciones corren a la vez: abrir un hilo por caratula de
/// golpe al entrar en una biblioteca grande no decodifica nada mas rapido,
/// solo satura el equipo un instante.
const MAX_CONCURRENT_DECODES: usize = 4;

struct CachedEntry {
    texture: Option<TextureHandle>,
    /// Marca de uso para el LRU; se actualiza cada vez que se pide.
    last_used: u64,
}

pub struct CoverTextures {
    cache: HashMap<String, CachedEntry>,
    clock: u64,
    /// Encolado o decodificandose ahora mismo: evita encolar dos veces la
    /// misma caratula mientras la primera peticion sigue en marcha.
    in_flight: HashSet<String>,
    decoding: usize,
    /// Generacion vigente de cada id, para descartar en `pump` un resultado
    /// de una decodificacion que `invalidate` ya dejo obsoleta -si no, un
    /// decode viejo que termina tarde podria pisar la textura nueva con la
    /// imagen de antes, o (peor) borrar el `in_flight` de la peticion nueva
    /// que ocupa ese mismo id ahora mismo.
    generation: HashMap<String, u64>,
    queue: VecDeque<(String, u64, PathBuf)>,
    tx: Sender<DecodeResult>,
    rx: Receiver<DecodeResult>,
}

struct DecodeResult {
    game_id: String,
    generation: u64,
    image: Option<ColorImage>,
}

impl Default for CoverTextures {
    fn default() -> Self {
        let (tx, rx) = mpsc::channel();
        Self {
            cache: HashMap::new(),
            clock: 0,
            in_flight: HashSet::new(),
            decoding: 0,
            generation: HashMap::new(),
            queue: VecDeque::new(),
            tx,
            rx,
        }
    }
}

/// Manda el resultado al soltarse, con imagen si `finish` llego a llamarse o
/// sin ella en caso contrario -incluido un panico dentro de `decode`, que de
/// otro modo dejaria el hueco de concurrencia ocupado para siempre y la
/// descarga de caratulas parada a medias sin que nada lo notara.
struct DecodeGuard {
    tx: Sender<DecodeResult>,
    game_id: String,
    generation: u64,
    sent: bool,
}

impl DecodeGuard {
    fn finish(mut self, image: Option<ColorImage>) {
        self.sent = true;
        let _ = self.tx.send(DecodeResult {
            game_id: std::mem::take(&mut self.game_id),
            generation: self.generation,
            image,
        });
    }
}

impl Drop for DecodeGuard {
    fn drop(&mut self) {
        if !self.sent {
            let _ = self.tx.send(DecodeResult {
                game_id: std::mem::take(&mut self.game_id),
                generation: self.generation,
                image: None,
            });
        }
    }
}

impl CoverTextures {
    /// Textura de la caratula de `game_id`, si ya esta decodificada. Si no lo
    /// esta todavia, encola la decodificacion (si no lo estaba ya) y devuelve
    /// `None` por ahora: la tarjeta se dibuja con la caratula generada
    /// mientras tanto, sin bloquear el frame actual a esperar un disco lento
    /// o una imagen grande.
    pub fn get(&mut self, game_id: &str, path: Option<&Path>) -> Option<TextureHandle> {
        let path = path?;
        self.clock += 1;
        if let Some(entry) = self.cache.get_mut(game_id) {
            entry.last_used = self.clock;
            return entry.texture.clone();
        }
        if self.in_flight.insert(game_id.to_string()) {
            let generation = self.generation.get(game_id).copied().unwrap_or(0);
            self.queue.push_back((game_id.to_string(), generation, path.to_path_buf()));
        }
        None
    }

    /// Se llama cuando una descarga acaba de reemplazar la caratula: fuera de
    /// la cache y de la cola, para que la proxima peticion la decodifique de
    /// nuevo en vez de devolver lo que hubiera (que podria ser un `None` de
    /// un intento anterior, o la imagen vieja). Tambien sube la generacion
    /// del id, para que un decode que ya estuviera en marcha con la imagen
    /// vieja no pueda pisar lo que traiga la peticion nueva.
    pub fn invalidate(&mut self, game_id: &str) {
        self.cache.remove(game_id);
        self.in_flight.remove(game_id);
        self.queue.retain(|(id, _, _)| id != game_id);
        *self.generation.entry(game_id.to_string()).or_insert(0) += 1;
    }

    /// Trabajo de fondo: se llama una vez por frame. Lanza decodificaciones
    /// pendientes hasta el limite de concurrencia y recoge las que ya han
    /// terminado, subiendolas a la GPU aqui, en el hilo de dibujado, que es
    /// el unico sitio donde `Context::load_texture` es valido.
    pub fn pump(&mut self, ctx: &egui::Context) {
        while self.decoding < MAX_CONCURRENT_DECODES {
            let Some((game_id, generation, path)) = self.queue.pop_front() else { break };
            self.decoding += 1;
            let guard = DecodeGuard { tx: self.tx.clone(), game_id: game_id.clone(), generation, sent: false };
            std::thread::spawn(move || {
                let decoded = decode(&path);
                guard.finish(decoded);
            });
        }

        let mut arrived = false;
        while let Ok(result) = self.rx.try_recv() {
            self.decoding = self.decoding.saturating_sub(1);
            let current_generation = self.generation.get(&result.game_id).copied().unwrap_or(0);
            if result.generation != current_generation {
                // Obsoleto: la caratula de este id se invalido mientras se
                // decodificaba. No se toca ni `in_flight` ni el cache, o se
                // pisaria el estado de la peticion nueva que ocupa ese id.
                continue;
            }
            arrived = true;
            self.in_flight.remove(&result.game_id);
            if result.image.is_none() {
                log::warn!("no se pudo decodificar la caratula de '{}'", result.game_id);
            }
            let texture = result
                .image
                .map(|image| ctx.load_texture(format!("caratula-{}", result.game_id), image, TextureOptions::LINEAR));
            self.clock += 1;
            self.cache.insert(result.game_id, CachedEntry { texture, last_used: self.clock });
        }

        if arrived {
            self.evict_over_budget();
        }
    }

    fn evict_over_budget(&mut self) {
        while self.cache.len() > MAX_CACHED {
            let Some(oldest) = self.cache.iter().min_by_key(|(_, entry)| entry.last_used).map(|(id, _)| id.clone())
            else {
                break;
            };
            self.cache.remove(&oldest);
        }
    }
}

/// Decodifica y redimensiona, pensada para correr en un hilo aparte. `None`
/// si el fichero no se pudo leer o no era una imagen valida; se cachea igual
/// (como `None`) para no reintentar un fichero corrupto en cada frame.
fn decode(path: &Path) -> Option<ColorImage> {
    let bytes = std::fs::read(path).ok()?;
    let image = image::load_from_memory(&bytes).ok()?;
    let image = if image.width() > MAX_SIDE || image.height() > MAX_SIDE {
        image.resize(MAX_SIDE, MAX_SIDE, image::imageops::FilterType::Triangle)
    } else {
        image
    };
    let rgba = image.to_rgba8();
    let (width, height) = rgba.dimensions();
    Some(ColorImage::from_rgba_unmultiplied([width as usize, height as usize], &rgba))
}
