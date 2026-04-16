//! Paquete de Telecomando PUS-C (ECSS-E-ST-70-41C).

use crate::{
    crc,
    error::PacketError,
    primary_header::{CcsdsPrimaryHeader, PacketType, SeqFlags},
};

/// Un paquete de Telecomando PUS-C.
///
/// ## Formato en cable
/// ```text
/// ┌─────────────────┬────────────────────────────────┬───────────────┬────────┐
/// │ Cabecera Primaria│ Cabecera Secundaria PUS        │ Datos Aplic.  │ CRC    │
/// │ (6 B)           │ (5 B)                          │ (variable)    │ (2 B)  │
/// └─────────────────┴────────────────────────────────┴───────────────┴────────┘
///
/// Cabecera Secundaria PUS-C TC (5 bytes):
///   Byte 0: versión PUS (bits 7-4) = 0b0010, spare (bits 3-0) = 0
///   Byte 1: tipo de servicio
///   Byte 2: tipo de subservicio
///   Bytes 3–4: ID de origen (u16 big-endian)
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
    /// Construye un nuevo telecomando PUS-C.
    ///
    /// # Arguments
    /// - `apid` — Identificador de Proceso de Aplicación (0x000–0x7FE)
    /// - `seq_count` — contador de secuencia de 14 bits (0–0x3FFF)
    /// - `service` — tipo de servicio PUS (p. ej., `17` para test/ping)
    /// - `subservice` — tipo de subservicio PUS (p. ej., `1` para ping are-you-alive)
    /// - `source_id` — identifica la estación terrestre o aplicación que envía este TC
    /// - `app_data` — bytes de payload específicos de la aplicación
    pub fn new(
        apid: u16,
        seq_count: u16,
        service: u8,
        subservice: u8,
        source_id: u16,
        app_data: Vec<u8>,
    ) -> Result<Self, PacketError> {
        // La cabecera secundaria PUS siempre tiene 5 bytes; app_data + CRC(2) siguen a continuación
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

    /// Serializa el TC a bytes, añadiendo el CRC-CCITT al final.
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(6 + 5 + self.app_data.len() + 2);
        buf.extend_from_slice(&self.primary.to_bytes());
        buf.push(0x20); // versión PUS-C = 0b0010, spare = 0
        buf.push(self.service);
        buf.push(self.subservice);
        buf.push((self.source_id >> 8) as u8);
        buf.push(self.source_id as u8);
        buf.extend_from_slice(&self.app_data);
        crc::append_crc(&mut buf);
        buf
    }

    /// Parsea un TC PUS-C desde bytes, verificando el CRC.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, PacketError> {
        if bytes.len() < 6 + 5 + 2 {
            return Err(PacketError::BufferTooShort { need: 13, got: bytes.len() });
        }
        let payload = crc::verify_and_strip_crc(bytes)?;

        let raw_hdr: [u8; 6] = payload[..6].try_into().unwrap();
        let primary = CcsdsPrimaryHeader::from_bytes(raw_hdr)?;

        // La cabecera secundaria PUS comienza en el byte 6
        let service = payload[7];
        let subservice = payload[8];
        let source_id = u16::from_be_bytes([payload[9], payload[10]]);
        let app_data = payload[11..].to_vec();

        Ok(Self { primary, service, subservice, source_id, app_data })
    }

    /// Tipo de servicio PUS.
    pub fn service(&self) -> u8 { self.service }
    /// Tipo de subservicio PUS.
    pub fn subservice(&self) -> u8 { self.subservice }
    /// Payload de datos de aplicación.
    pub fn app_data(&self) -> &[u8] { &self.app_data }
    /// APID de este paquete.
    pub fn apid(&self) -> u16 { self.primary.apid() }
    /// Contador de secuencia de este paquete.
    pub fn seq_count(&self) -> u16 { self.primary.seq_count() }
    /// Identificador de origen.
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
