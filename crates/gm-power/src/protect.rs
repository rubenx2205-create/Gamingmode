//! Lista de intocables.
//!
//! El modo juego baja de prioridad lo que estorba, pero hay dos familias de
//! procesos que no se tocan jamas:
//!
//! 1. Los criticos de Windows (sin ellos no hay sesion, ni sonido, ni teclado
//!    en pantalla).
//! 2. Las utilidades de las consolas portatiles. En una Steam Deck (o una ROG
//!    Ally, una Legion Go...) con Windows, el mando, los ventiladores, el TDP y
//!    el giroscopio los gestiona un programa aparte: si se degrada o se cierra,
//!    el aparato se queda sin mandos o achicharrandose. Justo lo contrario de
//!    lo que busca un modo juego.

/// Para que sirve una utilidad, de cara al informe.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HelperRole {
    /// Sin esto no hay mando.
    Controller,
    /// Ventiladores y temperaturas.
    Cooling,
    /// TDP, curva de potencia, brillo.
    Power,
    /// Superposiciones y limitadores de fps.
    Overlay,
    /// Controlador o servicio de dispositivo.
    Driver,
    /// Panel del fabricante del aparato.
    Vendor,
}

impl HelperRole {
    pub fn label(self) -> &'static str {
        match self {
            HelperRole::Controller => "mando",
            HelperRole::Cooling => "ventiladores",
            HelperRole::Power => "potencia/TDP",
            HelperRole::Overlay => "superposicion",
            HelperRole::Driver => "controlador",
            HelperRole::Vendor => "panel del fabricante",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Helper {
    /// Nombre del ejecutable sin extension.
    pub process: &'static str,
    /// Programa al que pertenece.
    pub suite: &'static str,
    pub role: HelperRole,
}

const fn helper(process: &'static str, suite: &'static str, role: HelperRole) -> Helper {
    Helper { process, suite, role }
}

/// Utilidades de consolas portatiles con Windows.
///
/// La lista se puede ampliar desde `config.toml` con `protected_processes`;
/// aqui estan las que vienen de serie.
pub const HANDHELD_HELPERS: &[Helper] = &[
    // --- Steam Deck ---
    helper("SteamController", "Steam Deck Tools", HelperRole::Controller),
    helper("SteamDeckTools.Helper", "Steam Deck Tools", HelperRole::Driver),
    helper("FanControl", "Steam Deck Tools", HelperRole::Cooling),
    helper("PowerControl", "Steam Deck Tools", HelperRole::Power),
    helper("PerformanceOverlay", "Steam Deck Tools", HelperRole::Overlay),
    helper("SWICD", "SWICD", HelperRole::Controller),
    helper("SWICDService", "SWICD", HelperRole::Driver),
    // --- Multiplataforma portatil ---
    helper("HandheldCompanion", "Handheld Companion", HelperRole::Controller),
    helper("ControllerService", "Handheld Companion", HelperRole::Driver),
    helper("HidHideClient", "HidHide", HelperRole::Driver),
    helper("HidHideCLI", "HidHide", HelperRole::Driver),
    helper("ViGEmBus", "ViGEm", HelperRole::Driver),
    helper("GlosSITarget", "GlosSI", HelperRole::Controller),
    helper("GlosSIConfig", "GlosSI", HelperRole::Controller),
    helper("DS4Windows", "DS4Windows", HelperRole::Controller),
    helper("reWASD", "reWASD", HelperRole::Controller),
    helper("reWASDEngine", "reWASD", HelperRole::Controller),
    helper("XOutput", "XOutput", HelperRole::Controller),
    helper("x360ce", "x360ce", HelperRole::Controller),
    // --- Superposiciones y limitadores ---
    helper("RTSS", "RivaTuner Statistics Server", HelperRole::Overlay),
    helper("RTSSHooksLoader64", "RivaTuner Statistics Server", HelperRole::Overlay),
    helper("EncoderServer", "RivaTuner Statistics Server", HelperRole::Overlay),
    helper("MSIAfterburner", "MSI Afterburner", HelperRole::Overlay),
    // --- Fabricantes de portatiles ---
    helper("ArmouryCrate.UserSessionHelper", "Armoury Crate", HelperRole::Vendor),
    helper("ArmouryCrateControlInterface", "Armoury Crate", HelperRole::Vendor),
    helper("ArmourySwAgent", "Armoury Crate", HelperRole::Vendor),
    helper("AsusOSD", "Armoury Crate", HelperRole::Vendor),
    helper("LegionGoQuickSettings", "Legion Space", HelperRole::Vendor),
    helper("LegionSpace", "Legion Space", HelperRole::Vendor),
    helper("MSICenterM", "MSI Center M", HelperRole::Vendor),
    helper("AYASpace", "AYASpace", HelperRole::Vendor),
    helper("OneXConsole", "OneXConsole", HelperRole::Vendor),
    helper("WinControls", "GPD WinControls", HelperRole::Controller),
    // --- Graficos: sus servicios controlan brillo, VRR y escalado ---
    helper("RadeonSoftware", "AMD Software", HelperRole::Driver),
    helper("AMDRSServ", "AMD Software", HelperRole::Driver),
    helper("AMDRSSrcExt", "AMD Software", HelperRole::Driver),
    helper("atieclxx", "AMD Software", HelperRole::Driver),
    helper("atiesrxx", "AMD Software", HelperRole::Driver),
    helper("nvcontainer", "NVIDIA", HelperRole::Driver),
    helper("NVDisplay.Container", "NVIDIA", HelperRole::Driver),
    helper("IGCC", "Intel Graphics", HelperRole::Driver),
    helper("ArcControl", "Intel Arc Control", HelperRole::Driver),
    helper("ArcControlAssist", "Intel Arc Control", HelperRole::Driver),
];

/// Procesos de Windows que no se tocan bajo ningun concepto. Degradar
/// cualquiera de estos va de "se ve a tirones" a "sesion perdida".
pub const CRITICAL_SYSTEM: &[&str] = &[
    "System",
    "Registry",
    "Idle",
    "smss",
    "csrss",
    "wininit",
    "winlogon",
    "services",
    "lsass",
    "lsm",
    "svchost",
    "fontdrvhost",
    "dwm",
    "audiodg",
    "sihost",
    "ctfmon",
    "conhost",
    "WUDFHost",
    "spoolsv",
    // Antivirus: degradarlo no acelera nada y da problemas raros.
    "MsMpEng",
    "NisSrv",
    "SecurityHealthService",
    "SecurityHealthSystray",
    // Teclado en pantalla: en una portatil sin teclado fisico es la unica via
    // de escribir.
    "TextInputHost",
    "TabTip",
    "osk",
];

/// Por que esta protegido un proceso.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Protection {
    System,
    Handheld(Helper),
    User,
    /// Es el propio shell, o el juego que se acaba de lanzar.
    Own(String),
}

impl Protection {
    pub fn reason(&self) -> String {
        match self {
            Protection::System => "proceso critico de Windows".to_string(),
            Protection::Handheld(helper) => format!("{} ({})", helper.suite, helper.role.label()),
            Protection::User => "protegido en la configuracion".to_string(),
            Protection::Own(what) => what.clone(),
        }
    }
}

/// Nombre sin la extension `.exe`, se escriba como se escriba. `rfind` cae
/// siempre en un limite de caracter, asi que un nombre con acentos no rompe.
fn stem(name: &str) -> &str {
    let trimmed = name.trim();
    match trimmed.rfind('.') {
        Some(dot) if trimmed[dot..].eq_ignore_ascii_case(".exe") => &trimmed[..dot],
        _ => trimmed,
    }
}

fn matches(candidate: &str, name: &str) -> bool {
    stem(candidate).eq_ignore_ascii_case(stem(name))
}

/// Esta protegido este proceso? Devuelve el motivo, que acaba en el informe.
pub fn protection_for(name: &str, extra: &[String], protect_handheld: bool) -> Option<Protection> {
    if CRITICAL_SYSTEM.iter().any(|critical| matches(critical, name)) {
        return Some(Protection::System);
    }
    if protect_handheld {
        if let Some(helper) = HANDHELD_HELPERS.iter().find(|helper| matches(helper.process, name)) {
            return Some(Protection::Handheld(*helper));
        }
    }
    if extra.iter().any(|entry| matches(entry, name)) {
        return Some(Protection::User);
    }
    None
}

/// Utilidades de portatil detectadas entre los procesos en ejecucion, sin
/// repetir programa: sirve para decirle al usuario que se le esta respetando.
pub fn detect_helpers<'a>(names: impl Iterator<Item = &'a str>) -> Vec<Helper> {
    let mut found: Vec<Helper> = Vec::new();
    for name in names {
        if let Some(helper) = HANDHELD_HELPERS.iter().find(|helper| matches(helper.process, name)) {
            if !found.iter().any(|other| other.suite == helper.suite) {
                found.push(*helper);
            }
        }
    }
    found
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn los_procesos_del_sistema_estan_siempre_protegidos() {
        assert_eq!(protection_for("csrss", &[], false), Some(Protection::System));
        // Con o sin extension, y sin distinguir mayusculas.
        assert_eq!(protection_for("LSASS.EXE", &[], false), Some(Protection::System));
        assert_eq!(protection_for("TextInputHost", &[], false), Some(Protection::System));
    }

    #[test]
    fn las_utilidades_de_portatil_se_protegen_cuando_toca() {
        let protection = protection_for("HandheldCompanion.exe", &[], true).unwrap();
        assert!(matches!(protection, Protection::Handheld(_)));
        assert!(protection.reason().contains("Handheld Companion"));
        assert!(protection.reason().contains("mando"));

        // Si el usuario desactiva la proteccion, deja de estarlo.
        assert_eq!(protection_for("HandheldCompanion", &[], false), None);
    }

    #[test]
    fn la_extension_da_igual_como_este_escrita() {
        assert_eq!(stem("Juego.EXE"), "Juego");
        assert_eq!(stem("sin_extension"), "sin_extension");
        assert_eq!(stem("varios.puntos.exe"), "varios.puntos");
        // Un nombre con acentos no puede partir un caracter por la mitad.
        assert_eq!(stem("Emulación.exe"), "Emulación");
        assert_eq!(stem("Emulación"), "Emulación");
    }

    #[test]
    fn el_usuario_puede_anadir_los_suyos() {
        let extra = vec!["MiUtilidad".to_string()];
        assert_eq!(protection_for("miutilidad.exe", &extra, true), Some(Protection::User));
        assert_eq!(protection_for("otracosa", &extra, true), None);
    }

    #[test]
    fn la_deteccion_no_repite_programa() {
        let running = ["SteamController", "FanControl", "PowerControl", "notepad", "HandheldCompanion"];
        let found = detect_helpers(running.iter().copied());
        let suites: Vec<&str> = found.iter().map(|helper| helper.suite).collect();
        assert_eq!(suites, vec!["Steam Deck Tools", "Handheld Companion"]);
    }

    #[test]
    fn no_hay_entradas_duplicadas_en_las_listas() {
        for (index, helper) in HANDHELD_HELPERS.iter().enumerate() {
            assert!(
                !HANDHELD_HELPERS[index + 1..].iter().any(|other| other.process == helper.process),
                "{} aparece dos veces",
                helper.process
            );
        }
    }
}
