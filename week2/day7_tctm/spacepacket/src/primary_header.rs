//! Cabecera primaria del Paquete Espacial CCSDS 133.0-B-2 (6 octetos).
//!
//! Todos los campos se empaquetan mediante manipulación de bits — no se necesita
//! ningún crate externo. Entender este código requiere únicamente operaciones
//! AND/OR/desplazamiento bit a bit, las mismas que se usan al programar
//! un registro de periférico en un STM32.

use crate::error::PacketError;

/// El campo de tipo de paquete distingue la telemetría de los telecomandos.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PacketType {
    /// Telemetría — datos que fluyen de la nave espacial al segmento terrestre (enlace descendente).
    Tm = 0,
    /// Telecomando — comandos que fluyen del segmento terrestre a la nave (enlace ascendente).
    Tc = 1,
}

/// Los flags de secuencia describen la posición de este paquete dentro de una secuencia.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SeqFlags {
    /// Este paquete es un segmento de continuación.
    Continuation = 0b00,
    /// Este es el primer segmento de un mensaje multi-paquete.
    First = 0b01,
    /// Este es el último segmento de un mensaje multi-paquete.
    Last = 0b10,
    /// Este paquete es autónomo (no segmentado). El más común.
    Standalone = 0b11,
}

/// Cabecera primaria del Paquete Espacial CCSDS 133.0-B-2.
///
/// Almacenada como 6 bytes en bruto; los accesores de campos decodifican al vuelo.
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
    /// Construye una nueva cabecera primaria.
    ///
    /// # Arguments
    /// - `pkt_type` — [`PacketType::Tc`] o [`PacketType::Tm`]
    /// - `apid` — Identificador de Proceso de Aplicación, debe ser `0..=0x7FE`
    /// - `seq_flags` — posición de este paquete en la secuencia; normalmente [`SeqFlags::Standalone`]
    /// - `seq_count` — contador de secuencia de 14 bits, debe ser `0..=0x3FFF`
    /// - `data_len` — número de octetos en el campo de datos del paquete; la cabecera
    ///   almacena `data_len − 1` según CCSDS §4.1.3.4
    ///
    /// # Errores
    /// Devuelve [`PacketError::InvalidApid`] si `apid > 0x7FE`.
    /// Devuelve [`PacketError::InvalidSeqCount`] si `seq_count > 0x3FFF`.
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
            | (1 << 11)        // flag de cabecera secundaria siempre activo para PUS
            | (apid & 0x07FF);
        raw[0] = (word0 >> 8) as u8;
        raw[1] = word0 as u8;

        // Bytes 2–3: seq_flags(2b) | seq_count(14b)
        let word1: u16 = ((seq_flags as u16) << 14) | (seq_count & 0x3FFF);
        raw[2] = (word1 >> 8) as u8;
        raw[3] = word1 as u8;

        // Bytes 4–5: data_length − 1  (CCSDS almacena uno menos que la longitud real)
        let stored_len = data_len.saturating_sub(1);
        raw[4] = (stored_len >> 8) as u8;
        raw[5] = stored_len as u8;

        Ok(Self { raw })
    }

    /// Parsea una cabecera primaria a partir de exactamente 6 bytes.
    ///
    /// # Errores
    /// Devuelve [`PacketError::UnknownVersion`] si el campo de versión de 3 bits es
    /// distinto de cero (CCSDS reserva únicamente la versión 0).
    pub fn from_bytes(bytes: [u8; 6]) -> Result<Self, PacketError> {
        let version = (bytes[0] >> 5) & 0x07;
        if version != 0 {
            return Err(PacketError::UnknownVersion { version });
        }
        Ok(Self { raw: bytes })
    }

    /// Devuelve la representación en bruto de 6 bytes.
    pub fn to_bytes(&self) -> [u8; 6] {
        self.raw
    }

    /// El número de versión CCSDS (siempre 0 para este estándar).
    pub fn version(&self) -> u8 {
        (self.raw[0] >> 5) & 0x07
    }

    /// El tipo de paquete: `Tc` (ascendente) o `Tm` (descendente).
    pub fn packet_type(&self) -> PacketType {
        if (self.raw[0] >> 4) & 0x01 == 1 {
            PacketType::Tc
        } else {
            PacketType::Tm
        }
    }

    /// Devuelve `true` para paquetes de Telecomando.
    pub fn is_tc(&self) -> bool {
        self.packet_type() == PacketType::Tc
    }

    /// Devuelve `true` para paquetes de Telemetría.
    pub fn is_tm(&self) -> bool {
        self.packet_type() == PacketType::Tm
    }

    /// El Identificador de Proceso de Aplicación (11 bits, 0x000–0x7FE).
    pub fn apid(&self) -> u16 {
        let word0 = u16::from_be_bytes([self.raw[0], self.raw[1]]);
        word0 & 0x07FF
    }

    /// Flags de secuencia (autónomo, primero, continuación, último).
    pub fn seq_flags(&self) -> SeqFlags {
        match (self.raw[2] >> 6) & 0x03 {
            0b00 => SeqFlags::Continuation,
            0b01 => SeqFlags::First,
            0b10 => SeqFlags::Last,
            _ => SeqFlags::Standalone,
        }
    }

    /// Contador de secuencia de paquete de 14 bits (0–0x3FFF).
    ///
    /// El contador debe incrementarse con cada paquete que tenga el mismo APID.
    /// Los saltos en la secuencia indican paquetes perdidos.
    pub fn seq_count(&self) -> u16 {
        let word1 = u16::from_be_bytes([self.raw[2], self.raw[3]]);
        word1 & 0x3FFF
    }

    /// Número de octetos en el campo de datos del paquete.
    ///
    /// Nota: la cabecera en bruto almacena `longitud_campo_datos − 1`, por lo que
    /// este accesor suma 1 para devolver el conteo real de bytes.
    pub fn data_field_len(&self) -> u16 {
        let stored = u16::from_be_bytes([self.raw[4], self.raw[5]]);
        stored + 1
    }

    /// Incrementa un contador de secuencia de 14 bits, haciendo wrap en 0x3FFF → 0x0000.
    ///
    /// ```
    /// use spacepacket::primary_header::CcsdsPrimaryHeader;
    /// assert_eq!(CcsdsPrimaryHeader::next_seq(0x3FFE), 0x3FFF);
    /// assert_eq!(CcsdsPrimaryHeader::next_seq(0x3FFF), 0x0000); // hace wrap!
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
