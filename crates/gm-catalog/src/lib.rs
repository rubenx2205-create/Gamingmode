//! Catalogo de ROMs: lee los `.jsonl` del repositorio Roms, los indexa bajo un
//! presupuesto de memoria y permite buscar y descargar.

pub mod download;
pub mod entry;
pub mod names;

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use gm_core::error::{Error, Result};
use gm_core::util;

pub use entry::{load_platform, CatalogEntry, EntryUrl, PlatformIndex};
pub use names::{display_name, family, Family};

/// Una plataforma detectada en disco, todavia sin cargar.
#[derive(Debug, Clone)]
pub struct PlatformInfo {
    pub id: String,
    pub display: String,
    pub family: Family,
    pub path: PathBuf,
    pub size_bytes: u64,
}

/// Indice de plataformas con cache acotada.
///
/// `mame.jsonl` solo pesa 16 MB, pero cargar las 150 plataformas a la vez se
/// come cientos de MB para nada: se mantienen solo las ultimas usadas hasta
/// agotar el presupuesto.
pub struct Catalog {
    dir: Option<PathBuf>,
    platforms: Vec<PlatformInfo>,
    loaded: HashMap<String, PlatformIndex>,
    /// Orden de uso; el ultimo es el mas reciente.
    usage: Vec<String>,
    budget_bytes: usize,
}

impl Catalog {
    pub fn new(dir: Option<PathBuf>, budget_mb: u64) -> Self {
        let mut catalog = Self {
            dir,
            platforms: Vec::new(),
            loaded: HashMap::new(),
            usage: Vec::new(),
            budget_bytes: (budget_mb.max(16) * 1024 * 1024) as usize,
        };
        if let Err(e) = catalog.rescan() {
            log::warn!("no se pudo leer el catalogo: {e}");
        }
        catalog
    }

    pub fn dir(&self) -> Option<&Path> {
        self.dir.as_deref()
    }

    pub fn set_dir(&mut self, dir: Option<PathBuf>) -> Result<()> {
        self.dir = dir;
        self.loaded.clear();
        self.usage.clear();
        self.rescan()
    }

    /// Vuelve a mirar que `.jsonl` hay en la carpeta configurada.
    pub fn rescan(&mut self) -> Result<()> {
        self.platforms.clear();
        let Some(dir) = self.dir.clone() else {
            return Ok(());
        };
        if !dir.is_dir() {
            return Err(Error::NotFound(dir.display().to_string()));
        }

        for item in std::fs::read_dir(&dir)? {
            let item = item?;
            let path = item.path();
            if path.extension().and_then(|e| e.to_str()) != Some("jsonl") {
                continue;
            }
            let Some(id) = path.file_stem().and_then(|s| s.to_str()) else { continue };
            self.platforms.push(PlatformInfo {
                id: id.to_string(),
                display: names::display_name(id),
                family: names::family(id),
                path: path.clone(),
                size_bytes: item.metadata().map(|m| m.len()).unwrap_or(0),
            });
        }
        self.platforms.sort_by(|a, b| a.display.to_lowercase().cmp(&b.display.to_lowercase()));
        log::info!("catalogo: {} plataformas en {}", self.platforms.len(), dir.display());
        Ok(())
    }

    pub fn platforms(&self) -> &[PlatformInfo] {
        &self.platforms
    }

    pub fn platform(&self, id: &str) -> Option<&PlatformInfo> {
        self.platforms.iter().find(|p| p.id == id)
    }

    /// Plataformas de una familia, en el orden de la rejilla.
    pub fn by_family(&self, family: Family) -> Vec<&PlatformInfo> {
        self.platforms.iter().filter(|p| p.family == family).collect()
    }

    pub fn is_loaded(&self, id: &str) -> bool {
        self.loaded.contains_key(id)
    }

    pub fn get(&self, id: &str) -> Option<&PlatformIndex> {
        self.loaded.get(id)
    }

    /// Marca una plataforma como recien usada (para el desalojo por LRU).
    pub fn touch(&mut self, id: &str) {
        if let Some(pos) = self.usage.iter().position(|u| u == id) {
            let id = self.usage.remove(pos);
            self.usage.push(id);
        }
    }

    /// Guarda un indice ya cargado (normalmente desde un hilo de trabajo).
    pub fn insert(&mut self, index: PlatformIndex) {
        let id = index.id.clone();
        self.usage.retain(|u| *u != id);
        self.loaded.insert(id.clone(), index);
        self.usage.push(id);
        self.evict_over_budget();
    }

    pub fn unload(&mut self, id: &str) {
        self.loaded.remove(id);
        self.usage.retain(|u| u != id);
    }

    pub fn loaded_bytes(&self) -> usize {
        self.loaded.values().map(|index| index.approx_bytes).sum()
    }

    pub fn budget_bytes(&self) -> usize {
        self.budget_bytes
    }

    pub fn memory_summary(&self) -> String {
        format!(
            "{} / {} en {} plataforma(s)",
            util::format_bytes(self.loaded_bytes() as u64),
            util::format_bytes(self.budget_bytes as u64),
            self.loaded.len()
        )
    }

    /// Desaloja las plataformas mas antiguas hasta volver al presupuesto. La
    /// recien insertada nunca se desaloja.
    fn evict_over_budget(&mut self) {
        while self.loaded_bytes() > self.budget_bytes && self.usage.len() > 1 {
            let oldest = self.usage.remove(0);
            log::debug!("catalogo: descargando '{oldest}' de memoria por presupuesto");
            self.loaded.remove(&oldest);
        }
    }

    /// Busca dentro de una plataforma ya cargada.
    pub fn search(&self, platform_id: &str, query: &str, limit: usize) -> Vec<usize> {
        match self.loaded.get(platform_id) {
            Some(index) => search(&index.entries, query, limit),
            None => Vec::new(),
        }
    }
}

/// Puntua una entrada frente a la consulta. Mas alto es mejor; 0 descarta.
fn score(entry: &CatalogEntry, needle: &str, tokens: &[&str]) -> u32 {
    if entry.search_key == needle {
        return 1000;
    }
    if entry.search_key.starts_with(needle) {
        return 800;
    }
    if entry.search_key.contains(needle) {
        return 600;
    }
    if tokens.len() > 1 && tokens.iter().all(|t| entry.search_key.contains(t)) {
        return 400;
    }
    0
}

/// Busqueda ordenada por relevancia; a igual puntuacion gana el nombre mas
/// corto, que casi siempre es la version "limpia" del juego.
pub fn search(entries: &[CatalogEntry], query: &str, limit: usize) -> Vec<usize> {
    let needle = util::normalize(query);
    if needle.is_empty() {
        return (0..entries.len().min(limit)).collect();
    }
    let tokens: Vec<&str> = needle.split(' ').filter(|t| !t.is_empty()).collect();

    let mut hits: Vec<(u32, usize, usize)> = entries
        .iter()
        .enumerate()
        .filter_map(|(index, entry)| {
            let score = score(entry, &needle, &tokens);
            (score > 0).then_some((score, entry.name.len(), index))
        })
        .collect();

    hits.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)).then(a.2.cmp(&b.2)));
    hits.truncate(limit);
    hits.into_iter().map(|(_, _, index)| index).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_dir(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("gm-catalog-{}-{tag}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        std::fs::write(
            dir.join("snes.jsonl"),
            concat!(
                r#"{"type":"file","p":"Super Mario World.sfc","n":"Super Mario World","ext":".sfc","parent":"","urls":[{"u":"https://e.org/1"}]}"#,
                "\n",
                r#"{"type":"file","p":"Super Mario All-Stars.sfc","n":"Super Mario All-Stars","ext":".sfc","parent":"","urls":[{"u":"https://e.org/2"}]}"#,
                "\n",
                r#"{"type":"file","p":"Chrono Trigger.sfc","n":"Chrono Trigger","ext":".sfc","parent":"","urls":[{"u":"https://e.org/3"}]}"#,
                "\n",
            ),
        )
        .unwrap();
        std::fs::write(
            dir.join("psx.jsonl"),
            concat!(
                r#"{"type":"file","p":"Metal Gear Solid.bin","n":"Metal Gear Solid","ext":".bin","parent":"","urls":[{"u":"https://e.org/4"}]}"#,
                "\n",
            ),
        )
        .unwrap();
        std::fs::write(dir.join("leeme.txt"), "no soy un catalogo").unwrap();
        dir
    }

    #[test]
    fn detecta_solo_los_jsonl_y_los_ordena_por_nombre() {
        let dir = sample_dir("scan");
        let catalog = Catalog::new(Some(dir.clone()), 64);
        let ids: Vec<&str> = catalog.platforms().iter().map(|p| p.id.as_str()).collect();
        assert_eq!(ids, vec!["psx", "snes"], "PlayStation antes que Super Nintendo");
        assert_eq!(catalog.platform("snes").unwrap().display, "Super Nintendo");
        assert_eq!(catalog.by_family(Family::Sony).len(), 1);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn busca_por_relevancia() {
        let dir = sample_dir("search");
        let mut catalog = Catalog::new(Some(dir.clone()), 64);
        let index = load_platform(&catalog.platform("snes").unwrap().path.clone(), "snes").unwrap();
        catalog.insert(index);

        let hits = catalog.search("snes", "super mario world", 10);
        let entries = &catalog.get("snes").unwrap().entries;
        assert_eq!(entries[hits[0]].name, "Super Mario World", "la coincidencia exacta va primera");

        // Varias palabras sueltas tambien encuentran.
        let hits = catalog.search("snes", "mario", 10);
        assert_eq!(hits.len(), 2);

        // Sin consulta se devuelve el principio de la lista, no todo.
        assert_eq!(catalog.search("snes", "", 2).len(), 2);
        // Una plataforma no cargada no rompe: devuelve vacio.
        assert!(catalog.search("psx", "metal", 10).is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn el_presupuesto_desaloja_la_plataforma_mas_antigua() {
        let dir = sample_dir("lru");
        // Presupuesto minimo (16 MB) reducido a mano para forzar el desalojo.
        let mut catalog = Catalog::new(Some(dir.clone()), 64);
        catalog.budget_bytes = 1;

        catalog.insert(load_platform(&catalog.platform("snes").unwrap().path.clone(), "snes").unwrap());
        catalog.insert(load_platform(&catalog.platform("psx").unwrap().path.clone(), "psx").unwrap());

        assert!(!catalog.is_loaded("snes"), "la mas antigua deberia haberse descargado");
        assert!(catalog.is_loaded("psx"), "la recien cargada nunca se desaloja");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn una_carpeta_inexistente_da_error_pero_no_revienta() {
        let catalog = Catalog::new(Some(PathBuf::from("/no/existe/roms")), 64);
        assert!(catalog.platforms().is_empty());
        assert_eq!(catalog.loaded_bytes(), 0);
    }
}
