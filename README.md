# Modo Juego

Un modo juego para Windows: una interfaz de salon a pantalla completa,
manejable de principio a fin con mando (XInput) o con teclado, cuyo trabajo de
verdad es **quitarle a Windows de encima todo lo que te esta robando recursos
mientras juegas, y devolverlo exactamente como estaba al salir**.

Es un frontend **neutral**: no habla con ninguna tienda, no pide cuentas y no
privilegia a ningun lanzador. Tu le dices que ejecutable, que ROM o que atajo
quieres arrancar, y el se encarga del resto.

Escrito en Rust con `egui`/`wgpu`. Sin runtime que instalar: el shell se mueve
en el entorno de los 30-60 MB de RAM y baja a practicamente 0 % de CPU cuando
hay un juego en primer plano.

## Lo que lo diferencia

### 1. Busca lo que pesa en *tu* equipo, no en una lista

La mayoria de "optimizadores" llevan una lista de nombres de programas
escrita por su autor. Eso acierta en el PC de su autor. Aqui se hace al reves:
al activarse, el modo juego **mide** los procesos en ejecucion, los ordena por
memoria y degrada los mas pesados que no esten protegidos. Sin listas de
marcas, sin suposiciones.

La vista **Recursos** ensena esa medicion antes de tocar nada: que hay
corriendo, cuanto ocupa cada cosa y que le va a pasar a cada una.

### 2. No toca lo que tu aparato necesita para funcionar

En una Steam Deck (o una ROG Ally, una Legion Go, una AYANEO...) con Windows,
el mando, los ventiladores, el TDP y el giroscopio los gestiona **un programa
aparte**. Si un optimizador lo cierra o lo degrada, te quedas sin mandos o con
el aparato achicharrandose. Justo lo contrario de lo que quieres.

Por eso hay una lista de intocables que viene activada de serie y que cubre:

| Programa | Para que |
|---|---|
| Steam Deck Tools (`SteamController`, `FanControl`, `PowerControl`, `PerformanceOverlay`) | mando, ventiladores, TDP, superposicion |
| Handheld Companion (`HandheldCompanion`, `ControllerService`) | mando, giroscopio, TDP |
| SWICD | mando |
| HidHide, ViGEm, GlosSI, DS4Windows, reWASD, XOutput, x360ce | controladores y emulacion de mando |
| RivaTuner Statistics Server, MSI Afterburner | superposicion y limitador de fps |
| Armoury Crate, Legion Space, MSI Center M, AYASpace, OneXConsole, WinControls | paneles de control del fabricante |
| AMD Software, NVIDIA, Intel Graphics/Arc | brillo, VRR, escalado |

Ademas de los procesos criticos de Windows, incluido el **teclado en pantalla**
(`TextInputHost`, `TabTip`), que en una portatil sin teclado fisico es la unica
forma de escribir.

**Si falta el programa de tu aparato**, se anade a mano en `config.toml`:

```toml
[power]
protected_processes = ["MiUtilidad", "OtraCosa"]
```

y esa utilidad deja de tocarse aunque sea la que mas RAM ocupe del equipo.

### 3. Los servicios, solo desde una lista blanca

Detener servicios de Windows por ranking de memoria es como quitar tornillos al
azar. El modo juego **solo** puede parar servicios de un catalogo revisado
(`SysMain`, `Windows Search`, telemetria, cola de impresion, Windows Update,
Optimizacion de entrega...), cada uno con su explicacion de que se pierde
mientras esta parado. Nada de audio, red, mandos o graficos entra en ese
catalogo, ni se puede meter a mano.

## Estado

Primera iteracion funcional:

- **Shell navegable con mando y teclado**: biblioteca, ficha de juego, catalogo,
  descargas, recursos, ajustes y panel rapido, con foco visible y leyenda de
  botones.
- **Entrada XInput** con zona muerta radial, auto-repeticion configurable,
  conexion en caliente y captura del **boton central del mando** (ordinal 100 de
  `xinput1_4.dll`), que abre el panel rapido incluso con un juego a pantalla
  completa.
- **Detecta la marca del mando** (Xbox, PlayStation, o teclado y raton si no
  hay ninguno conectado) leyendo el VID del dispositivo via Raw Input, y la
  ayuda de botones de toda la interfaz cambia sola: A/B/X/Y en un mando de
  Xbox, ✕○□△ en un DualShock/DualSense, Enter/Esc/F2... con teclado. Cambia en
  caliente segun con que se juega, sin tocar nada.
- **Tema claro/oscuro estrictamente monocromo**: blanco, negro y grises, sin
  acento de color de ninguna marca ni lanzador. Se cambia al momento desde los
  ajustes.
- **Motor de optimizacion reversible** con busqueda automatica de procesos
  pesados, catalogo seguro de servicios y lista de intocables.
- **Biblioteca propia**: cualquier `.exe` desde un explorador de ficheros
  navegable con la cruceta, ROMs con su emulador, o atajos del sistema.
- **Catalogo de ROMs** leyendo los `.jsonl` del repositorio
  [`Roms`](https://github.com/rubenx2205-create/Roms), con busqueda, presupuesto
  de memoria y descargas reanudables.
- **Caratulas reales via SteamGridDB** (opcional): se buscan solas en segundo
  plano para los juegos que aparecen en pantalla, sin ventanas ni dialogos que
  interrumpan la pantalla completa.

## Arquitectura

```
crates/
├── gm-core      Configuracion, biblioteca, lanzamiento de procesos
├── gm-input     XInput + teclado -> acciones de navegacion
├── gm-power     Medicion, proteccion y optimizacion reversible
├── gm-catalog   Indice de los .jsonl del repo Roms + descargas
└── gm-shell     Interfaz (egui/wgpu) y bucle principal
```

Las capas de abajo no saben nada de la interfaz y todo lo especifico de Win32
esta detras de `#[cfg(windows)]`, con un stub para el resto de plataformas: por
eso la seleccion de procesos, la lista de intocables, el catalogo de servicios y
la maquina de estados del optimizador se pueden probar en cualquier sistema.

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
cargo test --workspace
```

## Controles

La ayuda de botones de la barra inferior ensena siempre el glifo correcto para
lo que se este usando; aqui va la tabla completa con las tres variantes:

| Xbox | PlayStation | Teclado | Accion |
|---|---|---|---|
| Cruceta / stick | Cruceta / stick | Flechas o WASD | Mover el foco |
| A | ✕ | Enter | Aceptar / jugar / descargar / marcar |
| B | ○ | Esc | Volver / limpiar busqueda |
| X | □ | F2 | Accion contextual (anadir `.exe`, quitar, volver a medir) |
| Y | △ | F | Favorito / limpiar descargas / seleccion recomendada |
| LB / RB | L1 / R1 | Mayus+Tab / Tab | Cambiar orden / cambiar de seccion |
| LT / RT | L2 / R2 | RePag / AvPag | Saltar de pagina |
| Select | Share | `/` | Buscar |
| Start | Options | F1 | Panel rapido |
| **Boton central del mando** | **PS** | F12 | Panel rapido, tambien con un juego en marcha |

No hay que elegir nada: el shell detecta la marca del mando leyendo su VID por
Raw Input (Sony = PlayStation, cualquier otra cosa = Xbox, que es el esquema al
que XInput normaliza de todas formas) y cambia de variante sola en cuanto se
pulsa un boton. Sin ningun mando conectado se ensenan las teclas.

## Que hace exactamente el modo juego

Al activarlo (panel rapido → *Activar modo juego*), y **siempre apuntando antes
en disco lo que va a tocar**:

| Area | Accion | Vuelta atras |
|---|---|---|
| Energia | Cambia al plan configurado (por defecto *maximo rendimiento*, creandolo con `powercfg` si hace falta) | Se restaura el GUID del plan original |
| Servicios | Detiene los marcados del catalogo seguro | Se rearrancan solo los que estaban corriendo |
| Procesos | Mide, ordena por RAM y degrada los mas pesados no protegidos, con **EcoQoS** | Se devuelve la prioridad anterior de cada uno |
| Memoria | Vacia el *working set* de esos procesos | No hace falta: Windows lo repagina solo |
| Escritorio | Opcional: cierra `explorer.exe` | Se relanza al salir |
| Juego | Prioridad alta al proceso lanzado, y nunca se degrada | Termina con el proceso |

El estado previo se guarda en `power-snapshot.json` **antes** de aplicar ningun
cambio. Si el shell muere de golpe o se va la luz, al siguiente arranque detecta
el fichero y revierte todo automaticamente. Parar servicios requiere ejecutar el
shell como administrador; sin permisos lo dice en el informe en vez de fallar en
silencio.

### Como ahorra recursos el propio shell

- El ritmo de repintado se adapta: 60 fps navegando, 10 fps en reposo y **1-4 Hz
  con un juego en marcha** (solo vigila el boton del mando).
- Las ranuras de XInput sin mando no se consultan en cada frame, solo cada 2 s:
  sondear una ranura vacia recorre el bus USB y es carisimo.
- Si el `packet_number` de XInput no cambia, se salta el procesado del mando.
- La lista filtrada de la biblioteca solo se reordena cuando cambia algo.
- La medicion del sistema corre en un hilo aparte y se refresca como mucho cada
  10 s, y solo mientras la vista de Recursos esta abierta.
- El catalogo carga las plataformas bajo demanda y desaloja las menos usadas al
  llegar al presupuesto de memoria (256 MB por defecto).
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

Y los ajustes de rendimiento:

```toml
[power]
auto_throttle = true          # medir y degradar lo mas pesado
auto_throttle_max = 12        # cuantos procesos como mucho
auto_throttle_min_mb = 120    # a partir de cuanta RAM entra en el ranking
protect_handheld_helpers = true
protected_processes = []      # los tuyos, ademas de los de serie
extra_throttle = []           # degradar siempre estos, pesen lo que pesen
stop_services = true
services = []                 # vacio = seleccion recomendada del catalogo
```

Y el tema y las caratulas:

```toml
[general]
theme = "dark"                 # "dark" o "light"; tambien desde los ajustes

[covers]
steamgrid_api_key = "..."      # gratis en steamgriddb.com/profile/preferences/api
auto_fetch = true              # sin clave, no hace nada
```

## Catalogo de ROMs

En *Catalogo* → **X** se elige la carpeta con los `.jsonl` del repositorio
`Roms` (un clon local). Cada `.jsonl` es una plataforma; se cargan de una en una
y lo descargado se anade solo a la biblioteca.

## Caratulas (SteamGridDB)

[SteamGridDB](https://www.steamgriddb.com) es una base de datos comunitaria de
arte de caratulas, no una tienda: no hace falta cuenta para jugar, no lanza
nada y no sabe de compras. Con una clave gratuita puesta en `config.toml` (ver
arriba), el shell busca sola la caratula de cada juego que aparece en
pantalla, en un hilo aparte, y la sustituye por la generada en cuanto llega.
Sin clave, esta pieza simplemente no hace nada: no hay ninguna otra
dependencia de red en el shell aparte de las descargas del catalogo.

## Pendiente

- Detectar el modelo de portatil para ajustar los valores por defecto.
- Sonidos de interfaz y teclado en pantalla para buscar con el mando.
- Bordes redondeados en las caratulas reales (ahora mismo se recortan en
  rectangulo; egui no da rondeado nativo al recortar una imagen).
