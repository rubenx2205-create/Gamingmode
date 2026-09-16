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

fn main() -> eframe::Result {
    let _ = gm_core::paths::ensure_dirs();
    init_logging();
    log::info!("arrancando modo juego {}", env!("CARGO_PKG_VERSION"));

    // El modo de pantalla (fullscreen o no) NO se pide aqui: `winit` recuerda
    // el valor con el que crea la ventana, y si coincide con el que `App` le
    // vuelve a pedir en su primer frame (ver app.rs) lo descarta por
    // considerarlo un no-op, dejando la ventana atascada en su tamano normal.
    // Se crea siempre en modo ventana normal, oculta, y es `App` quien decide
    // el modo real una vez viva.
    let viewport = egui::ViewportBuilder::default()
        .with_title("Modo Juego")
        .with_app_id("gamingmode")
        .with_icon(load_icon())
        .with_min_inner_size([960.0, 540.0])
        .with_inner_size([1280.0, 720.0])
        // La ventana nace oculta: `App` la ensena en cuanto ha aplicado el
        // modo de pantalla en su primer frame. Sin esto se alcanza a ver,
        // durante una fraccion de segundo, la ventana a su tamano de ventana
        // normal antes de que salte a pantalla completa.
        .with_visible(false);

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

/// Icono de la ventana (barra de titulo, barra de tareas mientras corre).
/// El del propio fichero .exe -el que se ve en el Explorador sin ejecutarlo-
/// se incrusta aparte, en tiempo de compilacion (ver `build.rs`).
fn load_icon() -> egui::IconData {
    let bytes = include_bytes!("../../../assets/icon_256.png");
    let image = image::load_from_memory(bytes).expect("el icono incrustado deberia ser un PNG valido").to_rgba8();
    let (width, height) = image.dimensions();
    egui::IconData { rgba: image.into_raw(), width, height }
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
