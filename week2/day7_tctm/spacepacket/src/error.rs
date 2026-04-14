/// Errors that can occur when parsing or building CCSDS/PUS packets.
#[derive(Debug, thiserror::Error)]
pub enum PacketError {
    /// APID must be in range 0x000–0x7FE.
    ///
    /// 0x7FF is reserved for the idle (fill) packet APID (CCSDS 133.0-B-2 §4.1.2.3.2).
    #[error("APID {value:#05X} is out of range; must be 0x000–0x7FE")]
    InvalidApid { value: u16 },

    /// Sequence count exceeds 14-bit maximum (0x3FFF).
    #[error("sequence count {value} exceeds 14-bit maximum (0x3FFF)")]
    InvalidSeqCount { value: u16 },

    /// Buffer is too short to contain a valid primary header (need ≥ 6 bytes).
    #[error("buffer too short: need ≥ {need} bytes, got {got}")]
    BufferTooShort { need: usize, got: usize },

    /// CRC mismatch: packet is corrupted or tampered.
    #[error("CRC mismatch: computed {computed:#06X}, received {received:#06X}")]
    CrcMismatch { computed: u16, received: u16 },

    /// The CCSDS version field is non-zero (should always be 0b000).
    #[error("unknown CCSDS version {version}; expected 0")]
    UnknownVersion { version: u8 },

    /// Router has no route for the given APID.
    #[error("no route for APID {apid:#05X}")]
    NoRoute { apid: u16 },
}
