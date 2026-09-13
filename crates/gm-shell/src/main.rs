//! Modo juego para Windows: un shell de salon manejable con mando o teclado
//! que ademas se aparta (y aparta a Windows) cuando arranca un juego.

// Sin consola en las compilaciones de release: esto es una aplicacion de salon.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod browser;
mod covers;
mod nav;
mod theme;
mod views;

use gm_core::config::Config;

fn main() -> eframe::Result {
    let _ = gm_core::paths::ensure_dirs();
    init_logging();

    // La configuracion se lee aqui para saber como abrir la ventana; `App` la
    // vuelve a leer para quedarse con su copia.
    let config = Config::load().unwrap_or_default();
    log::info!("arrancando modo juego {}", env!("CARGO_PKG_VERSION"));

    let mut viewport = egui::ViewportBuilder::default()
        .with_title("Modo Juego")
        .with_app_id("gamingmode")
        .with_min_inner_size([960.0, 540.0])
        .with_inner_size([1280.0, 720.0]);
    if config.general.fullscreen {
        viewport = viewport.with_fullscreen(true).with_decorations(false);
    }

    let options = eframe::NativeOptions {
        viewport,
        // Con vsync el shell nunca dibuja mas rapido que la pantalla; el resto
        // del ahorro lo hace `request_repaint_after`.
        vsync: true,
        persist_window: false,
        ..Default::default()
    };

    eframe::run_native("Modo Juego", options, Box::new(|cc| Ok(Box::new(app::App::new(&cc.egui_ctx)))))
}

/// Log a fichero: en release no hay consola donde mirar.
fn init_logging() {
    let mut builder = env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"));
    match std::fs::OpenOptions::new().create(true).append(true).open(gm_core::paths::log_file()) {
        Ok(file) => {
            builder.target(env_logger::Target::Pipe(Box::new(file)));
        }
        Err(e) => eprintln!("no se pudo abrir el log: {e}"),
    }
    let _ = builder.try_init();
}
