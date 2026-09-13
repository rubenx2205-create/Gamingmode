//! Entrada unificada para el modo juego: XInput y teclado producen las mismas
//! acciones de navegacion, de forma que la interfaz nunca sabe de donde vino la
//! pulsacion.
//!
//! Dos decisiones importantes para el consumo de recursos:
//!
//! 1. Las ranuras vacias de XInput **no** se sondean en cada frame. Una llamada
//!    a `XInputGetState` sobre una ranura sin mando cuesta del orden de un
//!    milisegundo (recorre el bus USB); hacerlo 4 veces por frame a 60 Hz se
//!    come un nucleo entero. Solo se re-escanean cada `rescan_interval_ms`.
//! 2. Si el `packet_number` no cambia, el mando no se ha movido y se salta el
//!    procesado, incluida la deteccion de flancos.

pub mod backend;
pub mod kind;
pub mod pad;

use std::time::{Duration, Instant};

use backend::{PadBackend, PlatformBackend, MAX_PADS};
pub use kind::PadKind;
use pad::{apply_deadzone, button, PadSnapshot};

/// Acciones de navegacion de alto nivel. Es el unico vocabulario que entiende
/// la interfaz.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NavAction {
    Up,
    Down,
    Left,
    Right,
    /// A / Enter
    Accept,
    /// B / Escape
    Back,
    /// X / F2: acciones sobre el elemento enfocado.
    Context,
    /// Y / F: marcar como favorito.
    Favorite,
    /// Start / F1: menu principal.
    Menu,
    /// Select / Tab: buscar.
    Search,
    /// LB / Shift+Tab
    TabPrev,
    /// RB / Tab
    TabNext,
    /// Gatillo izquierdo / RePag
    PageUp,
    /// Gatillo derecho / AvPag
    PageDown,
    /// Boton central del mando / F12: panel rapido, tambien durante el juego.
    Guide,
}

impl NavAction {
    /// Las direcciones son las unicas acciones con auto-repeticion.
    pub fn is_directional(self) -> bool {
        matches!(self, NavAction::Up | NavAction::Down | NavAction::Left | NavAction::Right)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Dir {
    Up,
    Down,
    Left,
    Right,
}

impl Dir {
    fn action(self) -> NavAction {
        match self {
            Dir::Up => NavAction::Up,
            Dir::Down => NavAction::Down,
            Dir::Left => NavAction::Left,
            Dir::Right => NavAction::Right,
        }
    }
}

/// Resultado de un sondeo.
#[derive(Debug, Clone, Default)]
pub struct InputFrame {
    pub actions: Vec<NavAction>,
    /// Hubo cualquier tipo de actividad (aunque no generase accion). Sirve para
    /// sacar al shell del modo de bajo consumo.
    pub activity: bool,
    pub connected_pads: u8,
}

impl InputFrame {
    pub fn contains(&self, action: NavAction) -> bool {
        self.actions.contains(&action)
    }
}

/// Ritmo de sondeo segun lo que este haciendo el shell.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PollMode {
    /// El usuario esta navegando.
    Active,
    /// Nadie ha tocado nada en un rato.
    Idle,
    /// Hay un juego en primer plano: solo interesa el boton Guia.
    InGame,
}

#[derive(Debug, Clone, Default)]
struct PadTracker {
    connected: bool,
    packet: u32,
    prev_buttons: u16,
    prev_triggers: (bool, bool),
    dir: Option<Dir>,
    next_repeat: Option<Instant>,
}

pub struct InputHub {
    settings: gm_core::config::Input,
    backend: Option<Box<dyn PadBackend>>,
    backend_name: String,
    guide_supported: bool,
    /// Marca del mando conectado (Xbox/PlayStation), para que la interfaz
    /// ensene el boton correcto. Se recalcula solo al conectar un mando
    /// nuevo, no en cada frame: leer el VID via Raw Input recorre la lista
    /// entera de dispositivos de entrada del sistema.
    pad_kind: Option<PadKind>,
    trackers: [PadTracker; MAX_PADS],
    last_rescan: Option<Instant>,
    last_activity: Option<Instant>,
    rumble_stop: Option<Instant>,
}

impl InputHub {
    pub fn new(settings: gm_core::config::Input) -> Self {
        let backend = PlatformBackend::load(settings.capture_guide_button);
        let guide_supported = backend.as_ref().is_some_and(|b| b.guide_supported());
        let backend_name = backend.as_ref().map(|b| b.describe()).unwrap_or_else(|| "sin mandos".to_string());
        Self {
            settings,
            backend: backend.map(|b| Box::new(b) as Box<dyn PadBackend>),
            backend_name,
            guide_supported,
            pad_kind: None,
            trackers: Default::default(),
            last_rescan: None,
            last_activity: None,
            rumble_stop: None,
        }
    }

    /// Hub sin backend: util para pruebas y para arrancar sin mandos.
    pub fn headless(settings: gm_core::config::Input) -> Self {
        Self {
            settings,
            backend: None,
            backend_name: "desactivado".to_string(),
            guide_supported: false,
            pad_kind: None,
            trackers: Default::default(),
            last_rescan: None,
            last_activity: None,
            rumble_stop: None,
        }
    }

    pub fn backend_name(&self) -> &str {
        &self.backend_name
    }

    pub fn guide_supported(&self) -> bool {
        self.guide_supported
    }

    /// Marca del mando conectado ahora mismo. `None` sin mandos conectados.
    pub fn pad_kind(&self) -> Option<PadKind> {
        self.pad_kind
    }

    pub fn settings(&self) -> &gm_core::config::Input {
        &self.settings
    }

    pub fn set_settings(&mut self, settings: gm_core::config::Input) {
        self.settings = settings;
    }

    pub fn connected_pads(&self) -> u8 {
        self.trackers.iter().filter(|t| t.connected).count() as u8
    }

    /// Cada cuanto conviene volver a sondear.
    pub fn poll_interval(&self, mode: PollMode) -> Duration {
        let hz = match mode {
            PollMode::Active => self.settings.poll_hz.max(1),
            PollMode::Idle => 30,
            // Durante el juego el shell solo vigila el boton Guia: 10 Hz basta
            // y deja la CPU entera para el juego.
            PollMode::InGame => 10,
        };
        Duration::from_secs_f32(1.0 / hz as f32)
    }

    pub fn last_activity(&self) -> Option<Instant> {
        self.last_activity
    }

    /// Lee los mandos y traduce a acciones.
    pub fn poll(&mut self, now: Instant) -> InputFrame {
        self.update_rumble(now);

        let rescan_due = self
            .last_rescan
            .is_none_or(|last| now.duration_since(last) >= Duration::from_millis(self.settings.rescan_interval_ms));
        if rescan_due {
            self.last_rescan = Some(now);
        }

        let mut snapshots: [Option<PadSnapshot>; MAX_PADS] = Default::default();
        if let Some(backend) = self.backend.as_mut() {
            for (index, slot) in snapshots.iter_mut().enumerate() {
                // Una ranura vacia solo se consulta en los re-escaneos.
                if !self.trackers[index].connected && !rescan_due {
                    continue;
                }
                *slot = backend.read(index as u32);
            }
        }
        self.process(snapshots, now)
    }

    /// Nucleo puro: de instantaneas a acciones. Separado del backend para poder
    /// probar la repeticion y las zonas muertas en cualquier plataforma.
    pub fn process(&mut self, snapshots: [Option<PadSnapshot>; MAX_PADS], now: Instant) -> InputFrame {
        let mut frame = InputFrame::default();

        for (index, snapshot) in snapshots.into_iter().enumerate() {
            let Some(snapshot) = snapshot else {
                let tracker = &mut self.trackers[index];
                if tracker.connected {
                    log::info!("mando {index} desconectado");
                }
                *tracker = PadTracker::default();
                continue;
            };

            let was_connected = self.trackers[index].connected;
            self.trackers[index].connected = true;
            if !was_connected {
                log::info!("mando {index} conectado");
                // Se relee la marca del mando: puede haber cambiado el unico
                // conectado (por ejemplo, un DualSense sustituyendo a un
                // mando de Xbox), y es una lectura barata frente al resto del
                // sondeo, que ya se hace solo en reconexiones.
                self.pad_kind = kind::detect_pad_kind();
                // No se procesa el primer paquete: si el usuario tenia un boton
                // pulsado al enchufar, no debe contar como pulsacion.
                self.trackers[index].packet = snapshot.packet;
                self.trackers[index].prev_buttons = snapshot.buttons;
                continue;
            }

            if snapshot.packet == self.trackers[index].packet {
                continue; // nada se ha movido en este mando
            }
            self.trackers[index].packet = snapshot.packet;

            if !snapshot.is_idle(self.settings.stick_deadzone, self.settings.trigger_threshold) {
                frame.activity = true;
                self.last_activity = Some(now);
            }

            self.process_pad(index, &snapshot, now, &mut frame);
        }

        frame.connected_pads = self.connected_pads();
        if frame.connected_pads == 0 {
            self.pad_kind = None;
        }
        if !frame.actions.is_empty() {
            frame.activity = true;
            self.last_activity = Some(now);
        }
        frame
    }

    fn process_pad(&mut self, index: usize, snapshot: &PadSnapshot, now: Instant, frame: &mut InputFrame) {
        let tracker = &mut self.trackers[index];
        let just_pressed = snapshot.buttons & !tracker.prev_buttons;
        tracker.prev_buttons = snapshot.buttons;

        for (mask, action) in [
            (button::A, NavAction::Accept),
            (button::B, NavAction::Back),
            (button::X, NavAction::Context),
            (button::Y, NavAction::Favorite),
            (button::START, NavAction::Menu),
            (button::BACK, NavAction::Search),
            (button::LEFT_SHOULDER, NavAction::TabPrev),
            (button::RIGHT_SHOULDER, NavAction::TabNext),
            (button::GUIDE, NavAction::Guide),
        ] {
            if just_pressed & mask != 0 {
                frame.actions.push(action);
            }
        }

        // Gatillos tratados como botones, con flanco propio.
        let threshold = self.settings.trigger_threshold;
        let triggers = (snapshot.left_trigger >= threshold, snapshot.right_trigger >= threshold);
        if triggers.0 && !tracker.prev_triggers.0 {
            frame.actions.push(NavAction::PageUp);
        }
        if triggers.1 && !tracker.prev_triggers.1 {
            frame.actions.push(NavAction::PageDown);
        }
        tracker.prev_triggers = triggers;

        // Direccion: la cruceta manda; si no, el stick izquierdo.
        let dir = dpad_dir(snapshot.buttons)
            .or_else(|| stick_dir(snapshot.left_x, snapshot.left_y, self.settings.stick_deadzone));

        match dir {
            None => {
                tracker.dir = None;
                tracker.next_repeat = None;
            }
            Some(dir) if tracker.dir != Some(dir) => {
                // Cambio de direccion: dispara ya y arranca el retardo largo.
                tracker.dir = Some(dir);
                tracker.next_repeat = Some(now + Duration::from_millis(self.settings.repeat_delay_ms));
                frame.actions.push(dir.action());
            }
            Some(dir) => {
                if tracker.next_repeat.is_some_and(|deadline| now >= deadline) {
                    tracker.next_repeat = Some(now + Duration::from_millis(self.settings.repeat_interval_ms));
                    frame.actions.push(dir.action());
                }
            }
        }
    }

    /// Pulso corto de vibracion (por ejemplo al lanzar un juego).
    pub fn rumble(&mut self, strength: f32, duration: Duration, now: Instant) {
        if !self.settings.rumble_feedback {
            return;
        }
        let level = (strength.clamp(0.0, 1.0) * u16::MAX as f32) as u16;
        if let Some(backend) = self.backend.as_mut() {
            for index in 0..MAX_PADS {
                if self.trackers[index].connected {
                    backend.set_rumble(index as u32, level, level);
                }
            }
        }
        self.rumble_stop = Some(now + duration);
    }

    fn update_rumble(&mut self, now: Instant) {
        if let Some(stop) = self.rumble_stop {
            if now >= stop {
                if let Some(backend) = self.backend.as_mut() {
                    for index in 0..MAX_PADS {
                        if self.trackers[index].connected {
                            backend.set_rumble(index as u32, 0, 0);
                        }
                    }
                }
                self.rumble_stop = None;
            }
        }
    }

    /// Para los motores pase lo que pase. Se llama al cerrar el shell: un mando
    /// vibrando eternamente es la peor despedida posible.
    pub fn stop_rumble(&mut self) {
        if let Some(backend) = self.backend.as_mut() {
            for index in 0..MAX_PADS {
                backend.set_rumble(index as u32, 0, 0);
            }
        }
        self.rumble_stop = None;
    }
}

fn dpad_dir(buttons: u16) -> Option<Dir> {
    if buttons & button::DPAD_UP != 0 {
        Some(Dir::Up)
    } else if buttons & button::DPAD_DOWN != 0 {
        Some(Dir::Down)
    } else if buttons & button::DPAD_LEFT != 0 {
        Some(Dir::Left)
    } else if buttons & button::DPAD_RIGHT != 0 {
        Some(Dir::Right)
    } else {
        None
    }
}

fn stick_dir(x: f32, y: f32, deadzone: f32) -> Option<Dir> {
    let (x, y) = apply_deadzone(x, y, deadzone);
    if x == 0.0 && y == 0.0 {
        return None;
    }
    // El eje dominante gana; en diagonal exacta se prefiere el vertical, que es
    // el que mas se usa en una rejilla.
    if y.abs() >= x.abs() {
        Some(if y > 0.0 { Dir::Up } else { Dir::Down })
    } else {
        Some(if x > 0.0 { Dir::Right } else { Dir::Left })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hub() -> InputHub {
        InputHub::headless(gm_core::config::Input::default())
    }

    fn snap(packet: u32, buttons: u16) -> Option<PadSnapshot> {
        Some(PadSnapshot { packet, buttons, ..Default::default() })
    }

    fn pads(first: Option<PadSnapshot>) -> [Option<PadSnapshot>; MAX_PADS] {
        [first, None, None, None]
    }

    /// El primer paquete tras conectar no debe generar acciones.
    fn connect(hub: &mut InputHub, now: Instant) {
        hub.process(pads(snap(1, 0)), now);
    }

    #[test]
    fn el_primer_paquete_tras_conectar_no_dispara_nada() {
        let mut hub = hub();
        let now = Instant::now();
        // Mando enchufado con A pulsado: no debe contar como pulsacion.
        let frame = hub.process(pads(snap(1, button::A)), now);
        assert!(frame.actions.is_empty());
        assert_eq!(frame.connected_pads, 1);
    }

    #[test]
    fn detecta_flanco_de_botones_una_sola_vez() {
        let mut hub = hub();
        let now = Instant::now();
        connect(&mut hub, now);

        let frame = hub.process(pads(snap(2, button::A)), now);
        assert_eq!(frame.actions, vec![NavAction::Accept]);

        // Mantenerlo pulsado no repite: solo las direcciones se repiten.
        let frame = hub.process(pads(snap(3, button::A)), now + Duration::from_millis(500));
        assert!(frame.actions.is_empty());
    }

    #[test]
    fn la_repeticion_direccional_respeta_retardo_e_intervalo() {
        let mut hub = hub();
        let t0 = Instant::now();
        connect(&mut hub, t0);
        let delay = Duration::from_millis(hub.settings.repeat_delay_ms);
        let interval = Duration::from_millis(hub.settings.repeat_interval_ms);

        // Primera pulsacion: inmediata.
        let frame = hub.process(pads(snap(2, button::DPAD_DOWN)), t0);
        assert_eq!(frame.actions, vec![NavAction::Down]);

        // Antes del retardo no se repite.
        let frame = hub.process(pads(snap(3, button::DPAD_DOWN)), t0 + delay - Duration::from_millis(10));
        assert!(frame.actions.is_empty());

        // Cumplido el retardo, primera repeticion.
        let frame = hub.process(pads(snap(4, button::DPAD_DOWN)), t0 + delay);
        assert_eq!(frame.actions, vec![NavAction::Down]);

        // Y luego al ritmo corto.
        let frame = hub.process(pads(snap(5, button::DPAD_DOWN)), t0 + delay + interval);
        assert_eq!(frame.actions, vec![NavAction::Down]);
    }

    #[test]
    fn cambiar_de_direccion_dispara_de_inmediato() {
        let mut hub = hub();
        let t0 = Instant::now();
        connect(&mut hub, t0);
        hub.process(pads(snap(2, button::DPAD_DOWN)), t0);
        let frame = hub.process(pads(snap(3, button::DPAD_RIGHT)), t0 + Duration::from_millis(20));
        assert_eq!(frame.actions, vec![NavAction::Right]);
    }

    #[test]
    fn el_stick_navega_como_la_cruceta() {
        let mut hub = hub();
        let t0 = Instant::now();
        connect(&mut hub, t0);
        let tilted = PadSnapshot { packet: 2, left_y: -0.9, ..Default::default() };
        let frame = hub.process(pads(Some(tilted)), t0);
        assert_eq!(frame.actions, vec![NavAction::Down]);

        // Dentro de la zona muerta se suelta la direccion.
        let centered = PadSnapshot { packet: 3, left_y: -0.1, ..Default::default() };
        let frame = hub.process(pads(Some(centered)), t0 + Duration::from_millis(10));
        assert!(frame.actions.is_empty());
    }

    #[test]
    fn el_paquete_repetido_se_ignora() {
        let mut hub = hub();
        let t0 = Instant::now();
        connect(&mut hub, t0);
        hub.process(pads(snap(2, button::A)), t0);
        // Mismo packet_number: XInput dice que no ha cambiado nada.
        let frame = hub.process(pads(snap(2, button::B)), t0 + Duration::from_millis(10));
        assert!(frame.actions.is_empty());
    }

    #[test]
    fn los_gatillos_son_botones_con_flanco() {
        let mut hub = hub();
        let t0 = Instant::now();
        connect(&mut hub, t0);
        let pulled = PadSnapshot { packet: 2, right_trigger: 0.9, ..Default::default() };
        assert_eq!(hub.process(pads(Some(pulled)), t0).actions, vec![NavAction::PageDown]);
        let held = PadSnapshot { packet: 3, right_trigger: 0.9, ..Default::default() };
        assert!(hub.process(pads(Some(held)), t0 + Duration::from_millis(50)).actions.is_empty());
    }

    #[test]
    fn desconectar_limpia_el_estado() {
        let mut hub = hub();
        let t0 = Instant::now();
        connect(&mut hub, t0);
        hub.process(pads(snap(2, button::DPAD_UP)), t0);
        assert_eq!(hub.connected_pads(), 1);

        hub.process(pads(None), t0 + Duration::from_millis(10));
        assert_eq!(hub.connected_pads(), 0);

        // Al volver, otra vez paquete de cortesia sin acciones.
        let frame = hub.process(pads(snap(9, button::DPAD_UP)), t0 + Duration::from_millis(20));
        assert!(frame.actions.is_empty());
    }

    #[test]
    fn el_sondeo_baja_de_ritmo_durante_el_juego() {
        let hub = hub();
        assert!(hub.poll_interval(PollMode::InGame) > hub.poll_interval(PollMode::Active));
        assert!(hub.poll_interval(PollMode::Idle) > hub.poll_interval(PollMode::Active));
    }
}
