# Modo Juego

Un *gaming mode* para Windows al estilo del modo juego de SteamOS: una interfaz
de salon a pantalla completa, manejable de principio a fin con mando (XInput) o
con teclado, que ademas **libera recursos del sistema mientras juegas y los
devuelve exactamente como estaban al salir**.

Escrito en Rust con `egui`/`wgpu`. El binario no arrastra ningun runtime: el
shell se mueve en el entorno de los 30-60 MB de RAM y baja a ~0 % de CPU cuando
hay un juego en primer plano.

## Estado

Primera iteracion funcional. Lo que ya esta:

- **Shell navegable con mando y teclado**: rejilla de biblioteca, ficha de
  juego, catalogo, descargas, ajustes y panel rapido, todo con foco visible y
  leyenda de botones.
- **Entrada XInput** con zona muerta radial, auto-repeticion configurable,
  deteccion de conexion en caliente y captura del **boton Guia** (ordinal 100 de
  `xinput1_4.dll`), que abre el panel rapido incluso con un juego a pantalla
  completa.
- **Motor de optimizacion reversible**: plan de energia, servicios de Windows,
  EcoQoS y prioridad de procesos de fondo, liberacion de memoria y, si se
  quiere, cierre de `explorer.exe`.
- **Biblioteca propia**: anade cualquier `.exe` desde un explorador de ficheros
  navegable con la cruceta; tambien ROMs y URIs de lanzador (`steam://...`).
- **Catalogo de ROMs** leyendo los `.jsonl` del repositorio
  [`Roms`](https://github.com/rubenx2205-create/Roms), con busqueda, presupuesto
  de memoria y descargas reanudables.

## Arquitectura

```
crates/
├── gm-core      Configuracion, biblioteca, lanzamiento de procesos
├── gm-input     XInput + teclado -> acciones de navegacion
├── gm-power     Optimizacion de recursos reversible
├── gm-catalog   Indice de los .jsonl del repo Roms + descargas
└── gm-shell     Interfaz (egui/wgpu) y bucle principal
```

Las capas de abajo no saben nada de la interfaz y todo lo especifico de Win32
esta detras de `#[cfg(windows)]`, con un stub para el resto de plataformas: por
eso la logica de navegacion, el indice del catalogo y la maquina de estados del
optimizador se pueden probar en cualquier sistema.

## Compilar

En Windows:

```powershell
cargo build --release
# el binario queda en target\release\gamingmode.exe
```

Desde Linux se puede verificar que todo compila para Windows sin enlazar:

```bash
rustup target add x86_64-pc-windows-msvc
cargo check --target x86_64-pc-windows-msvc --workspace
cargo test --workspace          # las pruebas que no dependen de Win32
```

## Controles

| Mando | Teclado | Accion |
|---|---|---|
| Cruceta / stick izquierdo | Flechas o WASD | Mover el foco |
| A | Enter | Aceptar / jugar / descargar |
| B | Esc | Volver / limpiar busqueda |
| X | F2 | Accion contextual (anadir `.exe`, quitar, cancelar) |
| Y | F | Favorito / limpiar descargas / usar esta carpeta |
| LB / RB | Mayus+Tab / Tab | Cambiar orden / cambiar de seccion |
| Gatillos | RePag / AvPag | Saltar de pagina |
| Select | `/` | Buscar |
| Start | F1 | Panel rapido |
| **Guia (Xbox)** | F12 | Panel rapido, tambien con un juego en marcha |

## Que hace exactamente el modo juego

Al activarlo (panel rapido → *Activar modo juego*, o automaticamente si se
configura), y **siempre apuntando antes en disco lo que va a tocar**:

| Area | Accion | Vuelta atras |
|---|---|---|
| Energia | Cambia al plan configurado (por defecto *maximo rendimiento*, creandolo con `powercfg` si hace falta) | Se restaura el GUID del plan original |
| Servicios | Detiene los de la lista (`SysMain`, `WSearch`, `DiagTrack`, `Spooler`) | Se rearrancan solo los que estaban corriendo |
| Procesos | Baja a prioridad minima los navegadores/lanzadores/chats de la lista y les aplica **EcoQoS** | Se devuelve la prioridad anterior de cada uno |
| Memoria | Vacia el *working set* de esos procesos | No hace falta: Windows lo repagina solo |
| Escritorio | Opcional: cierra `explorer.exe` | Se relanza al salir |
| Juego | Prioridad alta al proceso lanzado | Termina con el proceso |

El estado previo se guarda en `power-snapshot.json` **antes** de aplicar ningun
cambio. Si el shell muere de golpe o se va la luz, al siguiente arranque detecta
el fichero y revierte todo automaticamente. Parar servicios requiere ejecutar el
shell como administrador; sin permisos, el panel rapido lo dice en vez de fallar
en silencio.

### Como ahorra recursos el propio shell

- El ritmo de repintado se adapta: 60 fps navegando, 10 fps en reposo y **1-4 Hz
  con un juego en marcha** (solo vigila el boton Guia).
- Las ranuras de XInput sin mando no se consultan en cada frame, solo cada 2 s:
  sondear una ranura vacia recorre el bus USB y es carisimo.
- Si el `packet_number` de XInput no cambia, se salta el procesado del mando.
- La lista filtrada de la biblioteca solo se reordena cuando cambia algo, no en
  cada frame.
- El catalogo carga las plataformas bajo demanda, en un hilo aparte, y desaloja
  las menos usadas al llegar al presupuesto de memoria (256 MB por defecto).
- Las descargas usan la pila TLS del propio Windows (schannel): no hay ninguna
  libreria de criptografia dentro del binario.

## Configuracion

Todo vive en `%APPDATA%\GamingMode`:

| Fichero | Contenido |
|---|---|
| `config.toml` | Ajustes (tambien editables desde la interfaz) |
| `library.json` | Biblioteca de juegos |
| `power-snapshot.json` | Estado del sistema mientras el modo juego esta activo |
| `gamingmode.log` | Registro |

Los emuladores se declaran en `config.toml`; `{rom}` se sustituye por la ruta:

```toml
[[emulators]]
id = "retroarch"
name = "RetroArch"
exe = 'C:\RetroArch\retroarch.exe'
args = ["-L", 'C:\RetroArch\cores\snes9x_libretro.dll', "{rom}"]
platforms = ["snes", "nes", "gb", "gba"]
```

## Catalogo de ROMs

En *Catalogo* → **X** se elige la carpeta con los `.jsonl` del repositorio
`Roms` (un clon local). Cada `.jsonl` es una plataforma; se cargan de una en una
y lo descargado se anade solo a la biblioteca.

## Pendiente

- Caratulas reales (el modelo ya guarda la ruta; ahora se generan a partir del
  titulo).
- Deteccion automatica de juegos de Steam, Epic, GOG y Xbox.
- Sonidos de interfaz y teclado en pantalla para buscar con el mando.
- Reordenar la cola de descargas.
