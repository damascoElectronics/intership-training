//! # spacepacket — CCSDS Space Packet Protocol + PUS-C TC/TM
//!
//! Implements key structures from:
//! - **CCSDS 133.0-B-2** — Space Packet Protocol (primary header)
//! - **ECSS-E-ST-70-41C** — Packet Utilization Standard (PUS-C TC/TM)
//!
//! ## Packet structure (CCSDS primary header, 6 octets)
//!
//! ```text
//! Octet  0        1        2        3        4        5
//!       ┌────────┬────────┬────────┬────────┬────────┬────────┐
//!       │VVV T S │AAAAAAAA│FF SSSSSS│SSSSSSSS│LLLLLLLL│LLLLLLLL│
//!       └────────┴────────┴────────┴────────┴────────┴────────┘
//!
//! V = Version (3 bits, always 0b000)
//! T = Type: 0=TM, 1=TC
//! S = Secondary Header Flag (1=present)
//! A = APID (11 bits, 0x000–0x7FE; 0x7FF = idle)
//! F = Sequence Flags (2 bits: 11=standalone, 01=first, 10=last, 00=continuation)
//! S = Sequence Count (14 bits, wraps 0–0x3FFF)
//! L = Data Length (16 bits, value = packet_data_field_octets − 1)
//! ```
//!
//! ## PUS-C packet structure (on top of primary header)
//!
//! ```text
//! ┌──────────────────┬─────────────────────────────────┬──────────┐
//! │ Primary Header   │ PUS Secondary Header             │ PEC      │
//! │ (6 B, CCSDS)     │ (variable, see below)            │ CRC 2 B  │
//! └──────────────────┴─────────────────────────────────┴──────────┘
//!
//! PUS-C TC secondary header (5 B):
//!   [PUS version(4b) + spare(4b)] [Service] [Subservice] [Source ID (2B)]
//!
//! PUS-C TM secondary header (10 B):
//!   [PUS version(4b) + spare(4b)] [Service] [Subservice] [Dest ID (2B)] [OBT (4+2B)]
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
