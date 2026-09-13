//! Lectura de los `.jsonl` del repositorio Roms.
//!
//! Cada linea es un objeto independiente:
//! ```text
//! {"type":"file","p":"sfiii3.zip","n":"sfiii3","ext":".zip","parent":"",
//!  "urls":[{"u":"https://...","label":"..."}]}
//! ```
//! Se lee en streaming: `mame.jsonl` pasa de 16 MB y no tiene ningun sentido
//! cargar el fichero entero en memoria antes de empezar a parsear.

use std::io::BufRead;
use std::path::Path;

use gm_core::error::Result;
use gm_core::util;
use serde::Deserialize;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntryUrl {
    pub url: String,
    pub label: Option<String>,
}

#[derive(Debug, Clone)]
pub struct CatalogEntry {
    /// Nombre legible (campo `n`).
    pub name: String,
    /// Ruta relativa dentro del conjunto (campo `p`).
    pub path: String,
    pub ext: String,
    pub parent: String,
    pub urls: Vec<EntryUrl>,
    /// Nombre normalizado para buscar.
    pub search_key: String,
}

impl CatalogEntry {
    pub fn primary_url(&self) -> Option<&str> {
        self.urls.first().map(|u| u.url.as_str())
    }

    /// Nombre de fichero sugerido al descargar.
    pub fn file_name(&self) -> String {
        let candidate = self.path.rsplit('/').next().unwrap_or(&self.path);
        if candidate.is_empty() {
            self.name.clone()
        } else {
            candidate.to_string()
        }
    }

    fn approx_bytes(&self) -> usize {
        self.name.len()
            + self.path.len()
            + self.ext.len()
            + self.parent.len()
            + self.search_key.len()
            + self.urls.iter().map(|u| u.url.len() + u.label.as_ref().map_or(0, |l| l.len())).sum::<usize>()
            + 96 // cabeceras de String/Vec
    }
}

#[derive(Deserialize)]
struct RawUrl {
    #[serde(default)]
    u: String,
    #[serde(default)]
    label: Option<String>,
}

#[derive(Deserialize)]
struct RawEntry {
    #[serde(default, rename = "type")]
    kind: String,
    #[serde(default)]
    p: String,
    #[serde(default)]
    n: String,
    #[serde(default)]
    ext: String,
    #[serde(default)]
    parent: String,
    #[serde(default)]
    urls: Vec<RawUrl>,
}

/// Una plataforma del catalogo ya cargada en memoria.
#[derive(Debug, Clone)]
pub struct PlatformIndex {
    pub id: String,
    pub display: String,
    pub entries: Vec<CatalogEntry>,
    /// Memoria aproximada que ocupa, para el presupuesto del catalogo.
    pub approx_bytes: usize,
    /// Lineas descartadas (JSON invalido o entradas sin descarga).
    pub skipped: usize,
}

/// Carga un `.jsonl` completo. Es una operacion lenta (decimas de segundo en
/// los ficheros grandes): la interfaz debe llamarla desde un hilo aparte.
pub fn load_platform(path: &Path, id: &str) -> Result<PlatformIndex> {
    let file = std::fs::File::open(path)?;
    // Buffer generoso: son ficheros de decenas de MB leidos de una tirada.
    let reader = std::io::BufReader::with_capacity(256 * 1024, file);

    let mut entries = Vec::new();
    let mut approx_bytes = 0usize;
    let mut skipped = 0usize;

    for line in reader.lines() {
        let line = line?;
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let raw: RawEntry = match serde_json::from_str(trimmed) {
            Ok(raw) => raw,
            Err(e) => {
                log::debug!("linea invalida en {}: {e}", path.display());
                skipped += 1;
                continue;
            }
        };
        // Las entradas de tipo "dir" solo describen carpetas del conjunto.
        if raw.kind == "dir" || raw.urls.is_empty() {
            skipped += 1;
            continue;
        }

        let name = if raw.n.is_empty() { raw.p.clone() } else { raw.n };
        let entry = CatalogEntry {
            search_key: util::normalize(&name),
            name,
            path: raw.p,
            ext: raw.ext,
            parent: raw.parent,
            urls: raw
                .urls
                .into_iter()
                .filter(|u| !u.u.is_empty())
                .map(|u| EntryUrl { url: u.u, label: u.label })
                .collect(),
        };
        approx_bytes += entry.approx_bytes();
        entries.push(entry);
    }

    entries.shrink_to_fit();
    Ok(PlatformIndex { id: id.to_string(), display: crate::names::display_name(id), entries, approx_bytes, skipped })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_sample(dir: &Path) -> std::path::PathBuf {
        std::fs::create_dir_all(dir).unwrap();
        let path = dir.join("snes.jsonl");
        std::fs::write(
            &path,
            concat!(
                r#"{"type": "file", "p": "Super Mario World.sfc", "n": "Super Mario World", "ext": ".sfc", "parent": "", "urls": [{"u": "https://example.org/smw.sfc"}]}"#,
                "\n",
                r#"{"type": "dir", "p": "carpeta", "n": "carpeta", "parent": ""}"#,
                "\n",
                "\n",
                r#"{"type": "file", "p": "roms/Chrono Trigger.sfc", "n": "Chrono Trigger", "ext": ".sfc", "parent": "roms", "urls": [{"u": "https://example.org/ct.sfc", "label": "Chrono Trigger"}]}"#,
                "\n",
                "{esto no es json}\n",
            ),
        )
        .unwrap();
        path
    }

    #[test]
    fn carga_saltando_directorios_y_lineas_rotas() {
        let dir = std::env::temp_dir().join(format!("gm-cat-entry-{}", std::process::id()));
        let path = write_sample(&dir);
        let index = load_platform(&path, "snes").unwrap();

        assert_eq!(index.entries.len(), 2);
        assert_eq!(index.skipped, 2, "un dir y una linea rota");
        assert_eq!(index.display, "Super Nintendo");
        assert!(index.approx_bytes > 0);

        let smw = &index.entries[0];
        assert_eq!(smw.primary_url(), Some("https://example.org/smw.sfc"));
        assert_eq!(smw.search_key, "super mario world");
        assert_eq!(smw.file_name(), "Super Mario World.sfc");
        // La ruta con carpetas se queda solo con el nombre de fichero.
        assert_eq!(index.entries[1].file_name(), "Chrono Trigger.sfc");

        let _ = std::fs::remove_dir_all(&dir);
    }
}
