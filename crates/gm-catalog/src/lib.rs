//! Integracion con SteamGridDB para las caratulas de la biblioteca.
//!
//! El modo juego no cataloga ni descarga nada por su cuenta: la unica pieza
//! que habla con un servicio externo es esta, y solo para traer arte de
//! caratula de una base de datos comunitaria (SteamGridDB no es una tienda,
//! no requiere cuenta para jugar y no lanza nada).

pub mod steamgrid;
