//! # day6-rustdoc — Codec de Tramas CCSDS
//!
//! Un crate de demostración para el módulo de entrenamiento de rustdoc de la **Semana 2, Día 6**.
//!
//! Este crate implementa un codec mínimo de cabecera primaria de Space Packet CCSDS —
//! suficiente código real del dominio aeroespacial para que los ejemplos de documentación
//! sean significativos, mientras permanece lo suficientemente pequeño para leerlo de una vez.
//!
//! ## Propósito
//!
//! Este crate existe para mostrar:
//! - Cómo escribir comentarios de documentación que cumplan con los estándares de documentación aeroespacial
//! - Cómo funcionan los doc tests y por qué son importantes
//! - Cómo `#![deny(missing_docs)]` impone la cobertura de documentación
//!
//! ## Módulos
//!
//! | Módulo | Propósito |
//! |--------|-----------|
//! | [`frame`] | Tipo [`frame::CcsdsPrimaryHeader`] y accesores de campos |
//! | [`error`] | Variantes de error [`error::CcsdsError`] |
//! | [`codec`] | Serialización de bytes [`codec::encode`] / [`codec::decode`] |
//!
//! ## Inicio Rápido
//!
//! ```rust
//! use day6_rustdoc::frame::CcsdsPrimaryHeader;
//! use day6_rustdoc::codec::{encode, decode};
//!
//! // Construir una cabecera TC para APID 0x100, conteo de secuencia 0, longitud de datos 4
//! let hdr = CcsdsPrimaryHeader::new_tc(0x100, 0, 4)?;
//! assert!(hdr.is_tc());
//! assert_eq!(hdr.apid(), 0x100);
//!
//! // Ida y vuelta: codificar a bytes y decodificar de vuelta
//! let bytes = encode(&hdr);
//! let decoded = decode(&bytes)?;
//! assert_eq!(decoded, hdr);
//! # Ok::<(), day6_rustdoc::error::CcsdsError>(())
//! ```
//!
//! ## Referencias
//!
//! - CCSDS 133.0-B-2: *Space Packet Protocol*, Blue Book
//! - ECSS-E-ST-70-41C: *Packet Utilisation Standard (PUS-C)*

// Denegar documentación faltante en todos los elementos públicos.
//
// POR QUÉ: En un crate de biblioteca usado por otros equipos (software de tierra, OBSW, arnés de pruebas),
// cada elemento público no documentado es una brecha en el contrato de interfaz. Hacer de esto un
// error de compilación — no solo una advertencia de lint — garantiza que la cobertura de documentación
// nunca pueda retroceder silenciosamente.
#![deny(missing_docs)]

pub mod codec;
pub mod error;
pub mod frame;
