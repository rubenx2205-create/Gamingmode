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
    /// Las entradas por URI (steam://...) las arranca el lanzador externo, asi
    /// que no tenemos un proceso hijo al que seguirle la pista.
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
        Launch::Rom { path, platform, emulator_id } => {
            if !path.exists() {
                return Err(Error::NotFound(path.display().to_string()));
            }
            let emulator = emulator_id
                .as_deref()
                .and_then(|id| config.emulator_by_id(id))
                .or_else(|| config.emulator_for(platform))
                .ok_or_else(|| Error::Config(format!("no hay emulador configurado para la plataforma '{platform}'")))?;
            if !emulator.exe.exists() {
                return Err(Error::NotFound(emulator.exe.display().to_string()));
            }
            let rom = path.display().to_string();
            let mut cmd = Command::new(&emulator.exe);
            for arg in &emulator.args {
                cmd.arg(arg.replace("{rom}", &rom));
            }
            if let Some(parent) = emulator.exe.parent() {
                cmd.current_dir(parent);
            }
            (cmd, true)
        }
        Launch::Uri { uri } => (uri_command(uri), false),
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
fn uri_command(uri: &str) -> Command {
    // `start` es un builtin de cmd, no un ejecutable: hay que invocarlo asi.
    // El "" inicial es el titulo de ventana, si no cmd interpreta el URI como
    // titulo cuando lleva comillas.
    let mut cmd = Command::new("cmd");
    cmd.args(["/C", "start", "", uri]);
    cmd
}

#[cfg(not(windows))]
fn uri_command(uri: &str) -> Command {
    let mut cmd = Command::new("xdg-open");
    cmd.arg(uri);
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

    #[test]
    fn una_rom_sin_emulador_da_error_de_configuracion() {
        let game = Game::new(
            "Rom",
            Launch::Rom { path: PathBuf::from("/no/existe/x.sfc"), platform: "snes".into(), emulator_id: None },
        );
        // Primero se valida la existencia de la ROM.
        assert!(matches!(launch(&game, &Config::default()), Err(Error::NotFound(_))));
    }
}
