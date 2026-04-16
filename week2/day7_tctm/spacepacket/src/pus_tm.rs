//! Paquete de Telemetría PUS-C (ECSS-E-ST-70-41C).

use crate::{
    crc,
    error::PacketError,
    primary_header::{CcsdsPrimaryHeader, PacketType, SeqFlags},
};

/// Un paquete de Telemetría PUS-C.
///
/// ## Formato en cable
/// ```text
/// ┌─────────────────┬─────────────────────────────────────────┬──────────┬──────┐
/// │ Cabecera Primaria│ Cabecera Secundaria PUS (10 B)          │ Datos    │ CRC  │
/// │ (6 B)           │                                         │ Aplic.   │ (2B) │
/// └─────────────────┴─────────────────────────────────────────┴──────────┴──────┘
///
/// Cabecera Secundaria PUS-C TM (10 bytes):
///   Byte 0: versión PUS (4b) = 0b0010, spare (4b) = 0
///   Byte 1: tipo de servicio
///   Byte 2: tipo de subservicio
///   Bytes 3–4: ID de destino (u16 big-endian)
///   Bytes 5–10: Tiempo a bordo (OBT) — 4B grueso (segundos) + 2B fino (sub-segundos)
/// ```
#[derive(Debug, Clone)]
pub struct PusTelemetry {
    primary: CcsdsPrimaryHeader,
    service: u8,
    subservice: u8,
    dest_id: u16,
    /// Tiempo a bordo: milisegundos desde la época de la nave espacial.
    obt_ms: u64,
    app_data: Vec<u8>,
}

impl PusTelemetry {
    /// Construye un nuevo paquete de telemetría PUS-C.
    pub fn new(
        apid: u16,
        seq_count: u16,
        service: u8,
        subservice: u8,
        dest_id: u16,
        obt_ms: u64,
        app_data: Vec<u8>,
    ) -> Result<Self, PacketError> {
        // Cabecera secundaria PUS-C TM = 10 B, + app_data + CRC(2)
        let data_field_len = (10 + app_data.len() + 2) as u16;
        let primary = CcsdsPrimaryHeader::new(
            PacketType::Tm,
            apid,
            SeqFlags::Standalone,
            seq_count,
            data_field_len,
        )?;
        Ok(Self { primary, service, subservice, dest_id, obt_ms, app_data })
    }

    /// Serializa el TM a bytes, añadiendo el CRC-CCITT.
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(6 + 10 + self.app_data.len() + 2);
        buf.extend_from_slice(&self.primary.to_bytes());
        buf.push(0x20); // versión PUS-C
        buf.push(self.service);
        buf.push(self.subservice);
        buf.push((self.dest_id >> 8) as u8);
        buf.push(self.dest_id as u8);
        // OBT: 4B grueso (segundos) + 2B fino (milisegundos dentro de ese segundo)
        let coarse = (self.obt_ms / 1000) as u32;
        let fine = ((self.obt_ms % 1000) * 65535 / 999) as u16;
        buf.extend_from_slice(&coarse.to_be_bytes());
        buf.extend_from_slice(&fine.to_be_bytes());
        buf.extend_from_slice(&self.app_data);
        crc::append_crc(&mut buf);
        buf
    }

    /// Parsea un TM PUS-C desde bytes, verificando el CRC.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, PacketError> {
        if bytes.len() < 6 + 10 + 2 {
            return Err(PacketError::BufferTooShort { need: 18, got: bytes.len() });
        }
        let payload = crc::verify_and_strip_crc(bytes)?;
        let raw_hdr: [u8; 6] = payload[..6].try_into().unwrap();
        let primary = CcsdsPrimaryHeader::from_bytes(raw_hdr)?;
        let service = payload[7];
        let subservice = payload[8];
        let dest_id = u16::from_be_bytes([payload[9], payload[10]]);
        let coarse = u32::from_be_bytes(payload[11..15].try_into().unwrap());
        let fine = u16::from_be_bytes([payload[15], payload[16]]);
        let obt_ms = (coarse as u64) * 1000 + (fine as u64) * 999 / 65535;
        let app_data = payload[17..].to_vec();
        Ok(Self { primary, service, subservice, dest_id, obt_ms, app_data })
    }

    pub fn service(&self) -> u8 { self.service }
    pub fn subservice(&self) -> u8 { self.subservice }
    pub fn app_data(&self) -> &[u8] { &self.app_data }
    pub fn apid(&self) -> u16 { self.primary.apid() }
    pub fn seq_count(&self) -> u16 { self.primary.seq_count() }
    pub fn obt_ms(&self) -> u64 { self.obt_ms }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip() {
        let tm = PusTelemetry::new(0x300, 1, 3, 25, 0, 123_456, b"hk_data".to_vec()).unwrap();
        let bytes = tm.to_bytes();
        let parsed = PusTelemetry::from_bytes(&bytes).unwrap();
        assert_eq!(parsed.apid(), 0x300);
        assert_eq!(parsed.service(), 3);
        assert_eq!(parsed.subservice(), 25);
        assert_eq!(parsed.app_data(), b"hk_data");
    }
}
