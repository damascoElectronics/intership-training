//! PUS-C Telecommand packet (ECSS-E-ST-70-41C).

use crate::{
    crc,
    error::PacketError,
    primary_header::{CcsdsPrimaryHeader, PacketType, SeqFlags},
};

/// A PUS-C Telecommand packet.
///
/// ## Wire format
/// ```text
/// ┌─────────────────┬────────────────────────────────┬───────────────┬────────┐
/// │ Primary Header  │ PUS Secondary Header           │ App Data      │ CRC    │
/// │ (6 B)           │ (5 B)                          │ (variable)    │ (2 B)  │
/// └─────────────────┴────────────────────────────────┴───────────────┴────────┘
///
/// PUS-C TC Secondary Header (5 bytes):
///   Byte 0: PUS version (bits 7-4) = 0b0010, spare (bits 3-0) = 0
///   Byte 1: Service type
///   Byte 2: Subservice type
///   Bytes 3–4: Source ID (big-endian u16)
/// ```
#[derive(Debug, Clone)]
pub struct PusTelecommand {
    primary: CcsdsPrimaryHeader,
    service: u8,
    subservice: u8,
    source_id: u16,
    app_data: Vec<u8>,
}

impl PusTelecommand {
    /// Constructs a new PUS-C telecommand.
    ///
    /// # Arguments
    /// - `apid` — Application Process Identifier (0x000–0x7FE)
    /// - `seq_count` — 14-bit sequence count (0–0x3FFF)
    /// - `service` — PUS service type (e.g., `17` for test/ping)
    /// - `subservice` — PUS subservice type (e.g., `1` for are-you-alive ping)
    /// - `source_id` — identifies the ground station or application sending this TC
    /// - `app_data` — application-specific payload bytes
    pub fn new(
        apid: u16,
        seq_count: u16,
        service: u8,
        subservice: u8,
        source_id: u16,
        app_data: Vec<u8>,
    ) -> Result<Self, PacketError> {
        // PUS secondary header is always 5 bytes; app_data + CRC(2) follow
        let data_field_len = (5 + app_data.len() + 2) as u16;
        let primary = CcsdsPrimaryHeader::new(
            PacketType::Tc,
            apid,
            SeqFlags::Standalone,
            seq_count,
            data_field_len,
        )?;
        Ok(Self { primary, service, subservice, source_id, app_data })
    }

    /// Serialises the TC to bytes, appending the CRC-CCITT at the end.
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(6 + 5 + self.app_data.len() + 2);
        buf.extend_from_slice(&self.primary.to_bytes());
        buf.push(0x20); // PUS-C version = 0b0010, spare = 0
        buf.push(self.service);
        buf.push(self.subservice);
        buf.push((self.source_id >> 8) as u8);
        buf.push(self.source_id as u8);
        buf.extend_from_slice(&self.app_data);
        crc::append_crc(&mut buf);
        buf
    }

    /// Parses a PUS-C TC from bytes, verifying the CRC.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, PacketError> {
        if bytes.len() < 6 + 5 + 2 {
            return Err(PacketError::BufferTooShort { need: 13, got: bytes.len() });
        }
        let payload = crc::verify_and_strip_crc(bytes)?;

        let raw_hdr: [u8; 6] = payload[..6].try_into().unwrap();
        let primary = CcsdsPrimaryHeader::from_bytes(raw_hdr)?;

        // PUS secondary header starts at byte 6
        let service = payload[7];
        let subservice = payload[8];
        let source_id = u16::from_be_bytes([payload[9], payload[10]]);
        let app_data = payload[11..].to_vec();

        Ok(Self { primary, service, subservice, source_id, app_data })
    }

    /// PUS service type.
    pub fn service(&self) -> u8 { self.service }
    /// PUS subservice type.
    pub fn subservice(&self) -> u8 { self.subservice }
    /// Application data payload.
    pub fn app_data(&self) -> &[u8] { &self.app_data }
    /// APID of this packet.
    pub fn apid(&self) -> u16 { self.primary.apid() }
    /// Sequence count of this packet.
    pub fn seq_count(&self) -> u16 { self.primary.seq_count() }
    /// Source identifier.
    pub fn source_id(&self) -> u16 { self.source_id }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip() {
        let tc = PusTelecommand::new(0x001, 7, 17, 1, 0xABCD, vec![0x01, 0x02]).unwrap();
        let bytes = tc.to_bytes();
        let parsed = PusTelecommand::from_bytes(&bytes).unwrap();
        assert_eq!(parsed.apid(), 0x001);
        assert_eq!(parsed.seq_count(), 7);
        assert_eq!(parsed.service(), 17);
        assert_eq!(parsed.subservice(), 1);
        assert_eq!(parsed.source_id(), 0xABCD);
        assert_eq!(parsed.app_data(), &[0x01, 0x02]);
    }
}
