//! Space packet types (simplified CCSDS 133.0-B-2).

use serde::{Deserialize, Serialize};

/// A simplified Space Packet for IPC use.
///
/// In a real system this would be built on top of the full `spacepacket` crate
/// introduced in Day 7.  Here we keep a flat, owned structure that serialises
/// cleanly with `bincode`.
///
/// # Telecommand vs Telemetry
///
/// The convention used throughout this stack is:
/// * **TC (uplink)** – `apid` has bit 12 set (0x1xxx).
/// * **TM (downlink)** – `apid` has bit 12 clear (0x0xxx).
///
/// This mirrors the CCSDS packet-type bit (bit 4 of the primary-header first
/// octet) but encoded in the APID field for simplicity.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpacePacket {
    /// Application Process Identifier (11-bit CCSDS field, stored as u16).
    pub apid: u16,
    /// Source sequence count (wraps at 0x3FFF per CCSDS).
    pub seq_count: u16,
    /// PUS service type.
    pub service: u8,
    /// PUS service subtype.
    pub subservice: u8,
    /// Source identifier (process/application that created the packet).
    pub source_id: u16,
    /// Mission elapsed time in milliseconds since epoch.
    pub timestamp_ms: u64,
    /// Application data (payload).
    pub data: Vec<u8>,
    /// HMAC-SHA256 authentication tag, present only on TC packets from ground.
    pub hmac: Option<[u8; 32]>,
}

impl SpacePacket {
    /// Create a new telecommand packet.
    ///
    /// The APID is stored with bit 12 set to mark it as a TC.
    pub fn new_tc(
        apid: u16,
        seq_count: u16,
        service: u8,
        subservice: u8,
        data: Vec<u8>,
    ) -> Self {
        Self {
            apid: apid | 0x1000, // set TC marker bit
            seq_count,
            service,
            subservice,
            source_id: 0,
            timestamp_ms: timestamp_now_ms(),
            data,
            hmac: None,
        }
    }

    /// Create a new telemetry packet.
    ///
    /// The APID is stored with bit 12 clear to mark it as TM.
    pub fn new_tm(
        apid: u16,
        seq_count: u16,
        service: u8,
        subservice: u8,
        data: Vec<u8>,
    ) -> Self {
        Self {
            apid: apid & !0x1000, // clear TC marker bit
            seq_count,
            service,
            subservice,
            source_id: 1, // OBC source
            timestamp_ms: timestamp_now_ms(),
            data,
            hmac: None,
        }
    }

    /// Returns `true` if this packet is a telecommand (uplink).
    #[inline]
    pub fn is_tc(&self) -> bool {
        self.apid & 0x1000 != 0
    }

    /// Returns the bare APID without the TC marker bit.
    #[inline]
    pub fn bare_apid(&self) -> u16 {
        self.apid & 0x0FFF
    }
}

/// Returns the current UNIX time in milliseconds.
///
/// Falls back to 0 on platforms where `SystemTime` is not available.
fn timestamp_now_ms() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// PUS-C service identifiers used in this stack.
///
/// See `reference/pus_service_catalog.md` for the full table.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum PusService {
    /// Service 1 – TC Verification
    TcVerification = 1,
    /// Service 3 – Housekeeping
    Housekeeping = 3,
    /// Service 5 – Event Reporting
    Event = 5,
    /// Service 17 – On-Board Operations (ping/pong)
    Test = 17,
}

impl PusService {
    /// Try to convert a raw service number to a [`PusService`].
    pub fn from_u8(v: u8) -> Option<Self> {
        match v {
            1 => Some(Self::TcVerification),
            3 => Some(Self::Housekeeping),
            5 => Some(Self::Event),
            17 => Some(Self::Test),
            _ => None,
        }
    }
}
