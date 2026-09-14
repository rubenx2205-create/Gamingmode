//! Biblioteca de juegos del usuario.
//!
//! El modo juego es un frontend neutral: no habla con ninguna tienda ni sabe de
//! cuentas. Solo conoce tres formas de arrancar algo: un ejecutable, una ROM
//! con su emulador, o un atajo del sistema.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::Result;
use crate::{paths, util};

/// Como se arranca una entrada de la biblioteca.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Launch {
    /// Un .exe cualquiera anadido por el usuario.
    Executable {
        path: PathBuf,
        #[serde(default)]
        args: Vec<String>,
        #[serde(default)]
        working_dir: Option<PathBuf>,
    },
    /// Una ROM que se abre con un emulador configurado.
    Rom {
        path: PathBuf,
        /// Id de plataforma del catalogo (`snes`, `psx`, ...).
        platform: String,
        /// Emulador concreto; si es None se resuelve por plataforma.
        #[serde(default)]
        emulator_id: Option<String>,
    },
    /// Un atajo del sistema: un `.lnk` o un URI de protocolo, sea cual sea.
    /// Lo resuelve Windows, aqui no se conoce ni se privilegia ningun programa
    /// concreto.
    Shortcut { target: String },
}

impl Launch {
    pub fn target_display(&self) -> String {
        match self {
            Launch::Executable { path, .. } => path.display().to_string(),
            Launch::Rom { path, .. } => path.display().to_string(),
            Launch::Shortcut { target } => target.clone(),
        }
    }

    /// Una entrada apunta a algo que existe todavia?
    pub fn is_available(&self) -> bool {
        match self {
            Launch::Executable { path, .. } | Launch::Rom { path, .. } => path.exists(),
            // Lo resuelve el sistema al abrirlo; desde aqui no se puede saber.
            Launch::Shortcut { .. } => true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Game {
    pub id: String,
    pub title: String,
    pub launch: Launch,
    /// Etiqueta libre para agrupar: "PC", "SNES", "Emuladores"...
    #[serde(default)]
    pub collection: String,
    #[serde(default)]
    pub cover: Option<PathBuf>,
    #[serde(default)]
    pub favorite: bool,
    #[serde(default)]
    pub added_at: u64,
    #[serde(default)]
    pub last_played: Option<u64>,
    #[serde(default)]
    pub play_seconds: u64,
    /// Clave de busqueda precalculada (no se serializa).
    #[serde(skip)]
    pub search_key: String,
}

impl Game {
    pub fn new(title: impl Into<String>, launch: Launch) -> Self {
        let title = title.into();
        let id = util::stable_id(&[&launch.target_display(), &title]);
        let search_key = util::normalize(&title);
        Self {
            id,
            title,
            launch,
            collection: String::new(),
            cover: None,
            favorite: false,
            added_at: now_secs(),
            last_played: None,
            play_seconds: 0,
            search_key,
        }
    }

    /// Deriva un titulo legible del nombre de fichero: quita la extension,
    /// cambia separadores por espacios y capitaliza.
    pub fn title_from_path(path: &Path) -> String {
        let stem =
            path.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_else(|| "Sin titulo".to_string());
        let cleaned = stem.replace(['_', '.'], " ").replace('-', " ");
        let mut out = String::with_capacity(cleaned.len());
        for (i, word) in cleaned.split_whitespace().enumerate() {
            if i > 0 {
                out.push(' ');
            }
            let mut chars = word.chars();
            if let Some(first) = chars.next() {
                out.extend(first.to_uppercase());
                out.push_str(chars.as_str());
            }
        }
        if out.is_empty() {
            stem
        } else {
            out
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sort {
    Title,
    LastPlayed,
    PlayTime,
    Added,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Library {
    #[serde(default)]
    pub games: Vec<Game>,
}

impl Library {
    pub fn load() -> Result<Self> {
        Self::load_from(&paths::library_file())
    }

    pub fn load_from(path: &Path) -> Result<Self> {
        let mut library: Library = match std::fs::read(path) {
            Ok(bytes) => serde_json::from_slice(&bytes)?,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Library::default(),
            Err(e) => return Err(e.into()),
        };
        for game in &mut library.games {
            game.search_key = util::normalize(&game.title);
        }
        Ok(library)
    }

    pub fn save(&self) -> Result<()> {
        self.save_to(&paths::library_file())
    }

    pub fn save_to(&self, path: &Path) -> Result<()> {
        util::write_atomic(path, &serde_json::to_vec_pretty(self)?)
    }

    /// Anade un juego; si ya existe uno con el mismo id devuelve false y no
    /// duplica la entrada.
    pub fn add(&mut self, game: Game) -> bool {
        if self.games.iter().any(|g| g.id == game.id) {
            return false;
        }
        self.games.push(game);
        true
    }

    pub fn remove(&mut self, id: &str) -> Option<Game> {
        let index = self.games.iter().position(|g| g.id == id)?;
        Some(self.games.remove(index))
    }

    pub fn get(&self, id: &str) -> Option<&Game> {
        self.games.iter().find(|g| g.id == id)
    }

    pub fn get_mut(&mut self, id: &str) -> Option<&mut Game> {
        self.games.iter_mut().find(|g| g.id == id)
    }

    /// Registra el final de una partida.
    pub fn record_session(&mut self, id: &str, seconds: u64) {
        if let Some(game) = self.get_mut(id) {
            game.play_seconds += seconds;
            game.last_played = Some(now_secs());
        }
    }

    /// Cambia el titulo de un juego ya anadido, recalculando la clave de
    /// busqueda a la vez: no deben quedar nunca desincronizados, o la
    /// biblioteca dejaria de encontrar por texto un juego recien renombrado.
    pub fn rename(&mut self, id: &str, title: impl Into<String>) -> bool {
        let Some(game) = self.get_mut(id) else { return false };
        game.title = title.into();
        game.search_key = util::normalize(&game.title);
        true
    }

    pub fn collections(&self) -> Vec<String> {
        let mut seen: Vec<String> = Vec::new();
        for game in &self.games {
            if !game.collection.is_empty() && !seen.contains(&game.collection) {
                seen.push(game.collection.clone());
            }
        }
        seen.sort();
        seen
    }

    /// Vista filtrada y ordenada. Devuelve indices para no clonar los juegos en
    /// cada frame de la interfaz.
    pub fn view(&self, query: &str, collection: Option<&str>, sort: Sort, favorites_only: bool) -> Vec<usize> {
        let needle = util::normalize(query);
        let mut indices: Vec<usize> = self
            .games
            .iter()
            .enumerate()
            .filter(|(_, g)| !favorites_only || g.favorite)
            .filter(|(_, g)| collection.is_none_or(|c| g.collection == c))
            .filter(|(_, g)| needle.is_empty() || needle.split(' ').all(|t| g.search_key.contains(t)))
            .map(|(i, _)| i)
            .collect();

        indices.sort_by(|a, b| {
            let (ga, gb) = (&self.games[*a], &self.games[*b]);
            match sort {
                Sort::Title => ga.search_key.cmp(&gb.search_key),
                Sort::LastPlayed => gb.last_played.cmp(&ga.last_played).then(ga.search_key.cmp(&gb.search_key)),
                Sort::PlayTime => gb.play_seconds.cmp(&ga.play_seconds).then(ga.search_key.cmp(&gb.search_key)),
                Sort::Added => gb.added_at.cmp(&ga.added_at).then(ga.search_key.cmp(&gb.search_key)),
            }
        });
        indices
    }

    /// Estadisticas para la cabecera del shell.
    pub fn stats(&self) -> HashMap<&'static str, u64> {
        let mut stats = HashMap::new();
        stats.insert("total", self.games.len() as u64);
        stats.insert("favoritos", self.games.iter().filter(|g| g.favorite).count() as u64);
        stats.insert("horas", self.games.iter().map(|g| g.play_seconds).sum::<u64>() / 3600);
        stats
    }
}

pub fn now_secs() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn exe(path: &str, title: &str) -> Game {
        Game::new(title, Launch::Executable { path: PathBuf::from(path), args: vec![], working_dir: None })
    }

    #[test]
    fn no_se_duplican_entradas() {
        let mut lib = Library::default();
        assert!(lib.add(exe("C:/a.exe", "Doom")));
        assert!(!lib.add(exe("C:/a.exe", "Doom")));
        assert_eq!(lib.games.len(), 1);
    }

    #[test]
    fn la_busqueda_ignora_acentos_y_orden_de_palabras() {
        let mut lib = Library::default();
        lib.add(exe("C:/a.exe", "Pokémon Edición Roja"));
        lib.add(exe("C:/b.exe", "Doom Eternal"));
        assert_eq!(lib.view("pokemon roja", None, Sort::Title, false).len(), 1);
        assert_eq!(lib.view("roja pokemon", None, Sort::Title, false).len(), 1);
        assert_eq!(lib.view("zelda", None, Sort::Title, false).len(), 0);
        assert_eq!(lib.view("", None, Sort::Title, false).len(), 2);
    }

    #[test]
    fn renombrar_actualiza_titulo_y_clave_de_busqueda() {
        let mut lib = Library::default();
        lib.add(exe("C:/a.exe", "Mi Juego v1 2 3 Win64"));
        let id = lib.games[0].id.clone();
        assert!(lib.rename(&id, "Pokemon Edicion Roja"));
        assert_eq!(lib.games[0].title, "Pokemon Edicion Roja");
        assert_eq!(lib.view("pokemon roja", None, Sort::Title, false).len(), 1, "la busqueda ya usa el nuevo titulo");
        assert_eq!(lib.view("mi juego", None, Sort::Title, false).len(), 0, "el titulo viejo ya no encuentra nada");
    }

    #[test]
    fn renombrar_algo_que_no_existe_no_hace_nada() {
        let mut lib = Library::default();
        assert!(!lib.rename("no-existe", "Nuevo"));
    }

    #[test]
    fn ordena_por_ultima_partida() {
        let mut lib = Library::default();
        lib.add(exe("C:/a.exe", "A"));
        lib.add(exe("C:/b.exe", "B"));
        let id_b = lib.games[1].id.clone();
        lib.record_session(&id_b, 120);
        let view = lib.view("", None, Sort::LastPlayed, false);
        assert_eq!(lib.games[view[0]].title, "B");
        assert_eq!(lib.games[view[0]].play_seconds, 120);
    }

    #[test]
    fn deriva_titulos_de_rutas() {
        assert_eq!(Game::title_from_path(Path::new("C:/x/super_mario_world.sfc")), "Super Mario World");
        assert_eq!(Game::title_from_path(Path::new("/x/DOOM.exe")), "DOOM");
    }
}
