//! # spacepacket — Protocolo de Paquetes Espaciales CCSDS + PUS-C TC/TM
//!
//! Implementa estructuras clave de:
//! - **CCSDS 133.0-B-2** — Protocolo de Paquetes Espaciales (cabecera primaria)
//! - **ECSS-E-ST-70-41C** — Estándar de Utilización de Paquetes (PUS-C TC/TM)
//!
//! ## Estructura del paquete (cabecera primaria CCSDS, 6 octetos)
//!
//! ```text
//! Octeto 0        1        2        3        4        5
//!       ┌────────┬────────┬────────┬────────┬────────┬────────┐
//!       │VVV T S │AAAAAAAA│FF SSSSSS│SSSSSSSS│LLLLLLLL│LLLLLLLL│
//!       └────────┴────────┴────────┴────────┴────────┴────────┘
//!
//! V = Versión (3 bits, siempre 0b000)
//! T = Tipo: 0=TM, 1=TC
//! S = Flag de Cabecera Secundaria (1=presente)
//! A = APID (11 bits, 0x000–0x7FE; 0x7FF = idle)
//! F = Flags de Secuencia (2 bits: 11=autónomo, 01=primero, 10=último, 00=continuación)
//! S = Contador de Secuencia (14 bits, wrap 0–0x3FFF)
//! L = Longitud de Datos (16 bits, valor = octetos_del_campo_de_datos − 1)
//! ```
//!
//! ## Estructura del paquete PUS-C (sobre la cabecera primaria)
//!
//! ```text
//! ┌──────────────────┬─────────────────────────────────┬──────────┐
//! │ Cabecera Primaria│ Cabecera Secundaria PUS          │ PEC      │
//! │ (6 B, CCSDS)     │ (variable, ver abajo)            │ CRC 2 B  │
//! └──────────────────┴─────────────────────────────────┴──────────┘
//!
//! Cabecera secundaria PUS-C TC (5 B):
//!   [versión PUS(4b) + spare(4b)] [Servicio] [Subservicio] [ID Origen (2B)]
//!
//! Cabecera secundaria PUS-C TM (10 B):
//!   [versión PUS(4b) + spare(4b)] [Servicio] [Subservicio] [ID Destino (2B)] [OBT (4+2B)]
//! ```

pub mod apid_router;
pub mod crc;
pub mod error;
pub mod primary_header;
pub mod pus_tc;
pub mod pus_tm;

pub use apid_router::ApidRouter;
pub use error::PacketError;
pub use primary_header::{CcsdsPrimaryHeader, PacketType, SeqFlags};
pub use pus_tc::PusTelecommand;
pub use pus_tm::PusTelemetry;
