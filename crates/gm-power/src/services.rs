//! Catalogo de servicios de Windows que se pueden detener sin romper nada.
//!
//! Es deliberadamente una lista blanca: el modo juego **solo** puede parar lo
//! que esta aqui. Un "optimizador" que para servicios arbitrarios porque salen
//! altos en un ranking de memoria acaba dejando el equipo sin sonido, sin red o
//! sin mandos. Todo lo de esta lista vuelve a arrancar solo (bajo demanda o al
//! reiniciar) y se restaura ademas al salir del modo juego.

#[derive(Debug, Clone, Copy)]
pub struct ServiceEntry {
    /// Nombre interno del servicio.
    pub name: &'static str,
    /// Nombre legible.
    pub display: &'static str,
    /// Que se pierde mientras esta parado.
    pub effect: &'static str,
    /// Se para de serie al activar el modo juego.
    pub default_on: bool,
}

const fn entry(name: &'static str, display: &'static str, effect: &'static str, default_on: bool) -> ServiceEntry {
    ServiceEntry { name, display, effect, default_on }
}

/// Ordenado a ojo de mas a menos rentable: arriba lo que de verdad se nota.
pub const CATALOG: &[ServiceEntry] = &[
    entry(
        "SysMain",
        "SysMain (Superfetch)",
        "precarga de programas; en un SSD no aporta y compite por disco y RAM",
        true,
    ),
    entry("WSearch", "Windows Search", "el indexador deja de trabajar; buscar en el menu inicio sera mas lento", true),
    entry("DiagTrack", "Experiencias del usuario conectado", "telemetria de Microsoft", true),
    entry("dmwappushservice", "Enrutador de mensajes WAP", "telemetria", true),
    entry("DoSvc", "Optimizacion de entrega", "deja de subir actualizaciones a otros equipos de la red", true),
    entry("wuauserv", "Windows Update", "no se buscan ni instalan actualizaciones mientras juegas", true),
    entry("UsoSvc", "Orquestador de actualizaciones", "igual que el anterior; van de la mano", true),
    entry("Spooler", "Cola de impresion", "no se puede imprimir", true),
    entry("WerSvc", "Informe de errores de Windows", "no se envian informes de fallos", true),
    entry("MapsBroker", "Mapas descargados", "la app de Mapas sin conexion", true),
    entry("RemoteRegistry", "Registro remoto", "acceso al registro desde otro equipo", true),
    entry(
        "BITS",
        "Transferencia inteligente en segundo plano",
        "descargas en segundo plano de Windows y de algunas apps",
        false,
    ),
    entry(
        "PcaSvc",
        "Asistente de compatibilidad de programas",
        "avisos de compatibilidad al abrir programas antiguos",
        false,
    ),
    entry("DPS", "Directiva de diagnostico", "diagnosticos automaticos de Windows", false),
    entry(
        "diagnosticshub.standardcollector.service",
        "Recopilador de diagnosticos",
        "captura de rendimiento para herramientas de desarrollo",
        false,
    ),
    entry("SSDPSRV", "Descubrimiento SSDP", "deteccion de dispositivos UPnP en la red", false),
    entry("upnphost", "Host de dispositivos UPnP", "compartir dispositivos por UPnP", false),
    entry("WMPNetworkSvc", "Uso compartido de Windows Media Player", "compartir la biblioteca multimedia", false),
    entry("lfsvc", "Servicio de geolocalizacion", "las apps no sabran donde estas", false),
    entry("stisvc", "Adquisicion de imagenes", "escaneres y camaras conectadas", false),
    entry("TrkWks", "Seguimiento de vinculos distribuidos", "seguimiento de accesos directos movidos", false),
    entry("SEMgrSvc", "Administrador de pagos NFC", "pagos por NFC", false),
    entry("WbioSrvc", "Servicio biometrico", "huella dactilar y reconocimiento facial", false),
    entry("WpcMonSvc", "Control parental", "supervision familiar", false),
    entry("DusmSvc", "Uso de datos", "estadisticas de consumo de datos", false),
    entry("Fax", "Fax", "nada, en serio", false),
    entry("RetailDemo", "Demo de tienda", "el modo demostracion de los expositores", false),
];

pub fn find(name: &str) -> Option<&'static ServiceEntry> {
    CATALOG.iter().find(|entry| entry.name.eq_ignore_ascii_case(name))
}

/// Solo se puede detener lo que esta en el catalogo.
pub fn is_allowed(name: &str) -> bool {
    find(name).is_some()
}

/// Seleccion por defecto al instalar.
pub fn defaults() -> Vec<String> {
    CATALOG.iter().filter(|entry| entry.default_on).map(|entry| entry.name.to_string()).collect()
}

/// Filtra una lista de servicios dejando solo los permitidos; devuelve tambien
/// los rechazados para poder avisar en el informe.
pub fn sanitize(requested: &[String]) -> (Vec<String>, Vec<String>) {
    let mut allowed = Vec::new();
    let mut rejected = Vec::new();
    for name in requested {
        match find(name) {
            // Se normaliza al nombre del catalogo: Windows distingue poco, pero
            // el informe queda mas legible.
            Some(entry) => {
                if !allowed.iter().any(|other: &String| other == entry.name) {
                    allowed.push(entry.name.to_string());
                }
            }
            None => rejected.push(name.clone()),
        }
    }
    (allowed, rejected)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn el_catalogo_no_tiene_duplicados() {
        for (index, entry) in CATALOG.iter().enumerate() {
            assert!(
                !CATALOG[index + 1..].iter().any(|other| other.name.eq_ignore_ascii_case(entry.name)),
                "{} duplicado",
                entry.name
            );
            assert!(!entry.effect.is_empty(), "{} sin explicacion", entry.name);
        }
    }

    #[test]
    fn solo_se_aceptan_servicios_del_catalogo() {
        let (allowed, rejected) =
            sanitize(&["sysmain".to_string(), "Audiosrv".to_string(), "SysMain".to_string(), "ViGEmBus".to_string()]);
        assert_eq!(allowed, vec!["SysMain"], "se normaliza y no se repite");
        assert_eq!(rejected, vec!["Audiosrv", "ViGEmBus"], "nada fuera de la lista blanca");
    }

    #[test]
    fn los_defaults_son_un_subconjunto_del_catalogo() {
        let defaults = defaults();
        assert!(!defaults.is_empty());
        assert!(defaults.iter().all(|name| is_allowed(name)));
        assert!(defaults.contains(&"SysMain".to_string()));
    }

    #[test]
    fn el_catalogo_no_toca_nada_de_audio_red_o_mandos() {
        // Un servicio de estos en la lista blanca seria un fallo grave.
        for prohibido in ["Audiosrv", "AudioEndpointBuilder", "ViGEmBus", "HidHide", "Dhcp", "Dnscache", "nsi"] {
            assert!(!is_allowed(prohibido), "{prohibido} no deberia poder detenerse");
        }
    }
}
