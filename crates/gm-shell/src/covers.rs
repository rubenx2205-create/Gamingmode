//! Carga de caratulas reales como textura de egui.
//!
//! Las imagenes ya estan en disco (las guarda `gm_catalog::steamgrid` en un
//! hilo aparte); este modulo solo decodifica y sube a la GPU, con un cache
//! para no repetir el trabajo en cada frame. Ni una linea de aqui toca la red.

use std::collections::HashMap;
use std::path::Path;

use egui::{ColorImage, TextureHandle, TextureOptions};

#[derive(Default)]
pub struct CoverTextures {
    // `None` cacheado significa "se intento y no se pudo decodificar": evita
    // reintentar la lectura de un fichero corrupto en cada frame.
    cache: HashMap<String, Option<TextureHandle>>,
}

impl CoverTextures {
    /// Textura de la caratula de `game_id`, decodificandola la primera vez.
    /// `None` si no hay ruta todavia o si el fichero no se pudo leer.
    pub fn get(&mut self, ctx: &egui::Context, game_id: &str, path: Option<&Path>) -> Option<TextureHandle> {
        let path = path?;
        if let Some(cached) = self.cache.get(game_id) {
            return cached.clone();
        }
        let texture = load(ctx, game_id, path);
        if texture.is_none() {
            log::warn!("no se pudo decodificar la caratula de '{game_id}' ({})", path.display());
        }
        self.cache.insert(game_id.to_string(), texture.clone());
        texture
    }

    /// Se llama cuando una descarga acaba de reemplazar la caratula: la
    /// proxima vez que se pida se decodifica de nuevo en vez de devolver lo
    /// que hubiera en cache (que podria ser `None` de un intento anterior).
    pub fn invalidate(&mut self, game_id: &str) {
        self.cache.remove(game_id);
    }
}

fn load(ctx: &egui::Context, game_id: &str, path: &Path) -> Option<TextureHandle> {
    let bytes = std::fs::read(path).ok()?;
    let image = image::load_from_memory(&bytes).ok()?.to_rgba8();
    let (width, height) = image.dimensions();
    let color_image = ColorImage::from_rgba_unmultiplied([width as usize, height as usize], &image);
    Some(ctx.load_texture(format!("caratula-{game_id}"), color_image, TextureOptions::LINEAR))
}
