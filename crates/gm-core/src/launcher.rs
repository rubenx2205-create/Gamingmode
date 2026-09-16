//! Lanzamiento y seguimiento del proceso del juego.

use std::path::PathBuf;
use std::process::{Child, Command};
use std::time::Instant;

use crate::config::{Config, GamePriority};
use crate::error::{Error, Result};
use crate::library::{Game, Launch};

/// Una sesion de juego en curso.
#[derive(Debug)]
pub struct Session {
    pub game_id: String,
    pub title: String,
    pub pid: Option<u32>,
    pub started: Instant,
    /// Los atajos los resuelve el sistema, que arranca el programa por su
    /// cuenta: no queda un proceso hijo al que seguirle la pista.
    pub tracked: bool,
    child: Option<Child>,
}

impl Session {
    /// Sigue vivo el juego? Para sesiones sin seguimiento devuelve siempre
    /// `true`: solo el usuario decide cuando ha terminado.
    pub fn is_running(&mut self) -> bool {
        match self.child.as_mut() {
            Some(child) => matches!(child.try_wait(), Ok(None)),
            None => true,
        }
    }

    pub fn elapsed_secs(&self) -> u64 {
        self.started.elapsed().as_secs()
    }

    pub fn terminate(&mut self) {
        if let Some(child) = self.child.as_mut() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

/// Resuelve la linea de comandos de un juego y lo arranca.
pub fn launch(game: &Game, config: &Config) -> Result<Session> {
    let (command, tracked) = match &game.launch {
        Launch::Executable { path, args, working_dir } => {
            if !path.exists() {
                return Err(Error::NotFound(path.display().to_string()));
            }
            let mut cmd = Command::new(path);
            cmd.args(args);
            // Muchos juegos buscan sus datos en el directorio del ejecutable.
            let cwd =
                working_dir.clone().or_else(|| path.parent().map(PathBuf::from)).unwrap_or_else(|| PathBuf::from("."));
            cmd.current_dir(cwd);
            (cmd, true)
        }
        Launch::Shortcut { target } => (shortcut_command(target), false),
    };

    let mut command = command;
    let child = command.spawn()?;
    let pid = child.id();

    if tracked {
        apply_priority(pid, config.power.game_priority);
    }

    Ok(Session {
        game_id: game.id.clone(),
        title: game.title.clone(),
        pid: Some(pid),
        started: Instant::now(),
        tracked,
        child: if tracked { Some(child) } else { None },
    })
}

#[cfg(windows)]
fn shortcut_command(target: &str) -> Command {
    // `start` es un builtin de cmd, no un ejecutable: hay que invocarlo asi.
    // El "" inicial es el titulo de ventana; sin el, cmd toma el destino como
    // titulo cuando va entre comillas.
    let mut cmd = Command::new("cmd");
    cmd.args(["/C", "start", "", target]);
    cmd
}

#[cfg(not(windows))]
fn shortcut_command(target: &str) -> Command {
    let mut cmd = Command::new("xdg-open");
    cmd.arg(target);
    cmd
}

#[cfg(windows)]
fn apply_priority(pid: u32, priority: GamePriority) {
    use windows::Win32::Foundation::CloseHandle;
    use windows::Win32::System::Threading::{
        OpenProcess, SetPriorityClass, ABOVE_NORMAL_PRIORITY_CLASS, HIGH_PRIORITY_CLASS, NORMAL_PRIORITY_CLASS,
        PROCESS_SET_INFORMATION,
    };

    let class = match priority {
        GamePriority::Normal => NORMAL_PRIORITY_CLASS,
        GamePriority::AboveNormal => ABOVE_NORMAL_PRIORITY_CLASS,
        GamePriority::High => HIGH_PRIORITY_CLASS,
    };

    unsafe {
        let Ok(handle) = OpenProcess(PROCESS_SET_INFORMATION, false, pid) else {
            log::warn!("no se pudo abrir el proceso {pid} para fijar la prioridad");
            return;
        };
        if SetPriorityClass(handle, class).is_err() {
            log::warn!("SetPriorityClass fallo para el proceso {pid}");
        }
        let _ = CloseHandle(handle);
    }
}

#[cfg(not(windows))]
fn apply_priority(_pid: u32, _priority: GamePriority) {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::library::Game;

    #[test]
    fn un_ejecutable_que_no_existe_da_not_found() {
        let game = Game::new(
            "Fantasma",
            Launch::Executable { path: PathBuf::from("/no/existe/x.exe"), args: vec![], working_dir: None },
        );
        assert!(matches!(launch(&game, &Config::default()), Err(Error::NotFound(_))));
    }

}
