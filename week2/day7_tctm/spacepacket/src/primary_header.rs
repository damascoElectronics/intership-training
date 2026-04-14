//! CCSDS 133.0-B-2 Space Packet Primary Header (6 octets).
//!
//! All fields are packed via bit manipulation — no external crate needed.
//! Understanding this code requires only bitwise AND/OR/shift operations,
//! the same operations you'd use programming an STM32 peripheral register.

use crate::error::PacketError;

/// The packet type field distinguishes telemetry from telecommands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PacketType {
    /// Telemetry — data flowing from spacecraft to ground (downlink).
    Tm = 0,
    /// Telecommand — commands flowing from ground to spacecraft (uplink).
    Tc = 1,
}

/// Sequence flags describe where this packet sits in a sequence.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SeqFlags {
    /// This packet is a continuation segment.
    Continuation = 0b00,
    /// This is the first segment of a multi-packet message.
    First = 0b01,
    /// This is the last segment of a multi-packet message.
    Last = 0b10,
    /// This packet is standalone (not segmented). Most common.
    Standalone = 0b11,
}

/// CCSDS 133.0-B-2 Space Packet Primary Header.
///
/// Stored as 6 raw bytes; field accessors decode on the fly.
///
/// ```text
/// Byte 0        Byte 1      Byte 2        Byte 3       Byte 4  Byte 5
/// ┌──────────────────────┬───────────────────────┬──────────────────┐
/// │ VVV T S AAAAAAAAAAA  │ FF SSSSSSSSSSSSSS     │ LLLLLLLLLLLLLLLL │
/// └──────────────────────┴───────────────────────┴──────────────────┘
/// V=version(3b) T=type(1b) S=sec_hdr_flag(1b) A=APID(11b)
/// F=seq_flags(2b) S=seq_count(14b) L=data_len(16b)
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CcsdsPrimaryHeader {
    raw: [u8; 6],
}

impl CcsdsPrimaryHeader {
    /// Constructs a new primary header.
    ///
    /// # Arguments
    /// - `pkt_type` — [`PacketType::Tc`] or [`PacketType::Tm`]
    /// - `apid` — Application Process Identifier, must be `0..=0x7FE`
    /// - `seq_flags` — where this packet sits in a sequence; usually [`SeqFlags::Standalone`]
    /// - `seq_count` — 14-bit sequence counter, must be `0..=0x3FFF`
    /// - `data_len` — number of octets in the packet data field; the header
    ///   stores `data_len − 1` per CCSDS §4.1.3.4
    ///
    /// # Errors
    /// Returns [`PacketError::InvalidApid`] if `apid > 0x7FE`.
    /// Returns [`PacketError::InvalidSeqCount`] if `seq_count > 0x3FFF`.
    pub fn new(
        pkt_type: PacketType,
        apid: u16,
        seq_flags: SeqFlags,
        seq_count: u16,
        data_len: u16,
    ) -> Result<Self, PacketError> {
        if apid > 0x7FE {
            return Err(PacketError::InvalidApid { value: apid });
        }
        if seq_count > 0x3FFF {
            return Err(PacketError::InvalidSeqCount { value: seq_count });
        }

        let mut raw = [0u8; 6];

        // Bytes 0–1: version(3b)=0 | type(1b) | sec_hdr_flag(1b)=1 | apid(11b)
        let word0: u16 = ((pkt_type as u16) << 12)
            | (1 << 11)        // secondary header flag always set for PUS
            | (apid & 0x07FF);
        raw[0] = (word0 >> 8) as u8;
        raw[1] = word0 as u8;

        // Bytes 2–3: seq_flags(2b) | seq_count(14b)
        let word1: u16 = ((seq_flags as u16) << 14) | (seq_count & 0x3FFF);
        raw[2] = (word1 >> 8) as u8;
        raw[3] = word1 as u8;

        // Bytes 4–5: data_length − 1  (CCSDS stores one less than actual length)
        let stored_len = data_len.saturating_sub(1);
        raw[4] = (stored_len >> 8) as u8;
        raw[5] = stored_len as u8;

        Ok(Self { raw })
    }

    /// Parses a primary header from exactly 6 bytes.
    ///
    /// # Errors
    /// Returns [`PacketError::UnknownVersion`] if the 3-bit version field is
    /// non-zero (CCSDS reserves only version 0).
    pub fn from_bytes(bytes: [u8; 6]) -> Result<Self, PacketError> {
        let version = (bytes[0] >> 5) & 0x07;
        if version != 0 {
            return Err(PacketError::UnknownVersion { version });
        }
        Ok(Self { raw: bytes })
    }

    /// Returns the raw 6-byte representation.
    pub fn to_bytes(&self) -> [u8; 6] {
        self.raw
    }

    /// The CCSDS version number (always 0 for this standard).
    pub fn version(&self) -> u8 {
        (self.raw[0] >> 5) & 0x07
    }

    /// The packet type: `Tc` (uplink) or `Tm` (downlink).
    pub fn packet_type(&self) -> PacketType {
        if (self.raw[0] >> 4) & 0x01 == 1 {
            PacketType::Tc
        } else {
            PacketType::Tm
        }
    }

    /// Returns `true` for Telecommand packets.
    pub fn is_tc(&self) -> bool {
        self.packet_type() == PacketType::Tc
    }

    /// Returns `true` for Telemetry packets.
    pub fn is_tm(&self) -> bool {
        self.packet_type() == PacketType::Tm
    }

    /// The Application Process Identifier (11 bits, 0x000–0x7FE).
    pub fn apid(&self) -> u16 {
        let word0 = u16::from_be_bytes([self.raw[0], self.raw[1]]);
        word0 & 0x07FF
    }

    /// Sequence flags (standalone, first, continuation, last).
    pub fn seq_flags(&self) -> SeqFlags {
        match (self.raw[2] >> 6) & 0x03 {
            0b00 => SeqFlags::Continuation,
            0b01 => SeqFlags::First,
            0b10 => SeqFlags::Last,
            _ => SeqFlags::Standalone,
        }
    }

    /// 14-bit packet sequence count (0–0x3FFF).
    ///
    /// The count must be incremented for each packet with the same APID.
    /// Gaps in the sequence indicate lost packets.
    pub fn seq_count(&self) -> u16 {
        let word1 = u16::from_be_bytes([self.raw[2], self.raw[3]]);
        word1 & 0x3FFF
    }

    /// Number of octets in the packet data field.
    ///
    /// Note: the raw header stores `data_field_len − 1`, so this accessor
    /// adds 1 back to give the true byte count.
    pub fn data_field_len(&self) -> u16 {
        let stored = u16::from_be_bytes([self.raw[4], self.raw[5]]);
        stored + 1
    }

    /// Increment a 14-bit sequence count, wrapping at 0x3FFF → 0x0000.
    ///
    /// ```
    /// use spacepacket::primary_header::CcsdsPrimaryHeader;
    /// assert_eq!(CcsdsPrimaryHeader::next_seq(0x3FFE), 0x3FFF);
    /// assert_eq!(CcsdsPrimaryHeader::next_seq(0x3FFF), 0x0000); // wraps!
    /// ```
    pub fn next_seq(current: u16) -> u16 {
        (current + 1) & 0x3FFF
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_tc() {
        let hdr = CcsdsPrimaryHeader::new(
            PacketType::Tc,
            0x100,
            SeqFlags::Standalone,
            42,
            20,
        )
        .unwrap();
        assert!(hdr.is_tc());
        assert_eq!(hdr.apid(), 0x100);
        assert_eq!(hdr.seq_count(), 42);
        assert_eq!(hdr.data_field_len(), 20);

        let hdr2 = CcsdsPrimaryHeader::from_bytes(hdr.to_bytes()).unwrap();
        assert_eq!(hdr, hdr2);
    }

    #[test]
    fn roundtrip_tm() {
        let hdr = CcsdsPrimaryHeader::new(PacketType::Tm, 0x200, SeqFlags::Standalone, 0, 5)
            .unwrap();
        assert!(hdr.is_tm());
        assert_eq!(hdr.apid(), 0x200);
    }

    #[test]
    fn rejects_idle_apid() {
        assert!(CcsdsPrimaryHeader::new(PacketType::Tc, 0x7FF, SeqFlags::Standalone, 0, 1)
            .is_err());
    }

    #[test]
    fn seq_wraps_at_14bit() {
        assert_eq!(CcsdsPrimaryHeader::next_seq(0x3FFF), 0);
    }
}
