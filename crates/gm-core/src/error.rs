use std::fmt;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug)]
pub enum Error {
    Io(std::io::Error),
    Json(serde_json::Error),
    Config(String),
    /// Una operacion que solo existe en Windows, invocada en otra plataforma.
    Unsupported(&'static str),
    /// La llamada al sistema fallo; incluye el codigo de error nativo.
    Os {
        call: &'static str,
        code: u32,
    },
    NotFound(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Io(e) => write!(f, "error de E/S: {e}"),
            Error::Json(e) => write!(f, "JSON invalido: {e}"),
            Error::Config(m) => write!(f, "configuracion invalida: {m}"),
            Error::Unsupported(m) => write!(f, "no disponible en esta plataforma: {m}"),
            Error::Os { call, code } => write!(f, "{call} fallo (codigo {code})"),
            Error::NotFound(m) => write!(f, "no encontrado: {m}"),
        }
    }
}

impl std::error::Error for Error {}

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        Error::Io(e)
    }
}

impl From<serde_json::Error> for Error {
    fn from(e: serde_json::Error) -> Self {
        Error::Json(e)
    }
}

impl From<toml::de::Error> for Error {
    fn from(e: toml::de::Error) -> Self {
        Error::Config(e.to_string())
    }
}

impl From<toml::ser::Error> for Error {
    fn from(e: toml::ser::Error) -> Self {
        Error::Config(e.to_string())
    }
}
