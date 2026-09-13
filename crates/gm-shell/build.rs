//! Incrusta el icono en el .exe (el que se ve en el Explorador, la barra de
//! tareas y Alt+Tab). Sin esto Windows le pone el icono generico de "programa
//! sin icono" al binario, aunque la ventana en ejecucion ya muestre el suyo.
//!
//! Fuera de Windows no hace nada: `cargo check`/`cargo test` en Linux (o al
//! compilar cruzado a otra plataforma) no necesitan `rc.exe` ni `windres`.

fn main() {
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    let mut resource = winresource::WindowsResource::new();
    resource.set_icon("../../assets/icon.ico");
    resource.set("ProductName", "Modo Juego");
    resource.set("FileDescription", "Modo juego para Windows");
    if let Err(e) = resource.compile() {
        // No se aborta la compilacion por esto: un binario sin icono sigue
        // siendo un binario que funciona.
        println!("cargo:warning=no se pudo incrustar el icono del .exe: {e}");
    }
}
