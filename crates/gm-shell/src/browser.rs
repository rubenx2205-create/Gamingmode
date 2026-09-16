//! Explorador de ficheros navegable con mando.
//!
//! Anadir un `.exe` escribiendo la ruta con un teclado en pantalla es un
//! suplicio; con la cruceta y el boton A se hace en cinco segundos.

use std::path::{Path, PathBuf};

use gm_input::NavAction;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Filter {
    /// Programas lanzables.
    Executables,
    /// Solo carpetas (para elegir una carpeta a importar).
    Directories,
}

const EXECUTABLE_EXTS: &[&str] = &["exe", "lnk", "bat", "cmd", "url", "msi"];

#[derive(Debug, Clone)]
pub struct Item {
    pub name: String,
    pub path: PathBuf,
    pub is_dir: bool,
    pub size: u64,
}

pub struct Browser {
    /// `None` significa "raiz": la lista de unidades en Windows.
    pub cwd: Option<PathBuf>,
    pub items: Vec<Item>,
    pub selected: usize,
    pub filter: Filter,
    pub error: Option<String>,
}

impl Browser {
    pub fn new(filter: Filter, start: Option<PathBuf>) -> Self {
        let mut browser = Self { cwd: start, items: Vec::new(), selected: 0, filter, error: None };
        browser.refresh();
        browser
    }

    pub fn title(&self) -> String {
        match &self.cwd {
            Some(path) => path.display().to_string(),
            None => "Equipo".to_string(),
        }
    }

    pub fn refresh(&mut self) {
        self.error = None;
        self.items.clear();
        match self.cwd.clone() {
            None => self.items = list_roots(),
            Some(dir) => match std::fs::read_dir(&dir) {
                Ok(entries) => {
                    for entry in entries.flatten() {
                        let path = entry.path();
                        let metadata = entry.metadata().ok();
                        let is_dir = metadata.as_ref().map(|m| m.is_dir()).unwrap_or(false);
                        let name = entry.file_name().to_string_lossy().to_string();
                        // Los ocultos de Unix y los de sistema solo estorban.
                        if name.starts_with('.') {
                            continue;
                        }
                        if !is_dir && !self.accepts(&path) {
                            continue;
                        }
                        self.items.push(Item { name, path, is_dir, size: metadata.map(|m| m.len()).unwrap_or(0) });
                    }
                    // Carpetas primero, luego por nombre sin distinguir mayusculas.
                    self.items.sort_by(|a, b| {
                        b.is_dir.cmp(&a.is_dir).then(a.name.to_lowercase().cmp(&b.name.to_lowercase()))
                    });
                }
                Err(e) => self.error = Some(format!("no se puede abrir {}: {e}", dir.display())),
            },
        }
        self.selected = 0;
    }

    fn accepts(&self, path: &Path) -> bool {
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
        match self.filter {
            Filter::Executables => EXECUTABLE_EXTS.contains(&ext.as_str()),
            Filter::Directories => false,
        }
    }

    pub fn navigate(&mut self, action: NavAction) {
        self.selected = crate::nav::move_list(self.selected, self.items.len(), action);
    }

    /// Sube un nivel. En la raiz de una unidad pasa a la lista de unidades.
    pub fn go_up(&mut self) -> bool {
        let Some(current) = self.cwd.clone() else { return false };
        match current.parent() {
            Some(parent) if parent != current => self.cwd = Some(parent.to_path_buf()),
            _ => self.cwd = None,
        }
        self.refresh();
        true
    }

    /// Abre lo seleccionado. Si es una carpeta entra en ella y devuelve `None`;
    /// si es un fichero devuelve su ruta.
    pub fn open_selected(&mut self) -> Option<PathBuf> {
        let item = self.items.get(self.selected)?.clone();
        if item.is_dir {
            self.cwd = Some(item.path);
            self.refresh();
            None
        } else {
            Some(item.path)
        }
    }

    /// Carpeta actual (para el modo "elegir directorio").
    pub fn current_dir(&self) -> Option<PathBuf> {
        self.cwd.clone()
    }
}

#[cfg(windows)]
fn list_roots() -> Vec<Item> {
    ('A'..='Z')
        .map(|letter| PathBuf::from(format!("{letter}:\\")))
        .filter(|path| path.is_dir())
        .map(|path| Item { name: path.display().to_string(), path, is_dir: true, size: 0 })
        .collect()
}

#[cfg(not(windows))]
fn list_roots() -> Vec<Item> {
    let mut roots = vec![Item { name: "/".to_string(), path: PathBuf::from("/"), is_dir: true, size: 0 }];
    if let Some(home) = std::env::var_os("HOME") {
        roots.push(Item { name: "Carpeta personal".to_string(), path: PathBuf::from(home), is_dir: true, size: 0 });
    }
    roots
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Cada prueba necesita su propio arbol: se ejecutan en paralelo y una
    /// borraria los ficheros de la otra a mitad.
    fn sample_tree(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("gm-browser-{}-{tag}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("subcarpeta")).unwrap();
        std::fs::write(dir.join("juego.exe"), b"x").unwrap();
        std::fs::write(dir.join("mario.sfc"), b"x").unwrap();
        std::fs::write(dir.join("notas.txt"), b"x").unwrap();
        std::fs::write(dir.join(".oculto"), b"x").unwrap();
        dir
    }

    #[test]
    fn filtra_por_tipo_y_pone_las_carpetas_primero() {
        let dir = sample_tree("filtros");
        let browser = Browser::new(Filter::Executables, Some(dir.clone()));
        let names: Vec<&str> = browser.items.iter().map(|i| i.name.as_str()).collect();
        assert_eq!(names, vec!["subcarpeta", "juego.exe"]);

        let browser = Browser::new(Filter::Directories, Some(dir.clone()));
        assert_eq!(browser.items.len(), 1, "en modo carpeta solo se ven carpetas");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn entrar_y_salir_de_carpetas() {
        let dir = sample_tree("navegar");
        let mut browser = Browser::new(Filter::Executables, Some(dir.clone()));
        assert_eq!(browser.open_selected(), None, "abrir una carpeta no devuelve fichero");
        assert_eq!(browser.cwd, Some(dir.join("subcarpeta")));

        browser.go_up();
        assert_eq!(browser.cwd, Some(dir.clone()));

        browser.navigate(NavAction::Down);
        assert_eq!(browser.open_selected(), Some(dir.join("juego.exe")));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn una_carpeta_ilegible_deja_mensaje_en_vez_de_panico() {
        let browser = Browser::new(Filter::Executables, Some(PathBuf::from("/no/existe/jamas")));
        assert!(browser.error.is_some());
        assert!(browser.items.is_empty());
    }
}
