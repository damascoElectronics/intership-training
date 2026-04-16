//! CRC-CCITT (CRC-16/CCITT-FALSE) según lo exigido por ECSS-E-ST-70-41C §A.3.
//!
//! Parámetros:
//! - Polinomio       : 0x1021
//! - Valor inicial   : 0xFFFF
//! - Reflexión entrada : false
//! - Reflexión salida  : false
//! - XOR salida      : 0x0000
//!
//! Esta es una implementación basada en tabla (O(n) con constante pequeña).
//! La tabla de búsqueda se calcula en tiempo de compilación.

/// Tabla de búsqueda CRC-CCITT precalculada (polinomio 0x1021).
const TABLE: [u16; 256] = {
    let mut table = [0u16; 256];
    let mut i = 0usize;
    while i < 256 {
        let mut crc = (i as u16) << 8;
        let mut j = 0;
        while j < 8 {
            if crc & 0x8000 != 0 {
                crc = (crc << 1) ^ 0x1021;
            } else {
                crc <<= 1;
            }
            j += 1;
        }
        table[i] = crc;
        i += 1;
    }
    table
};

/// Calcula el checksum CRC-CCITT sobre `data`.
///
/// El checksum se añade como los últimos 2 bytes de cada paquete PUS
/// (big-endian, MSB primero). Un receptor verifica ejecutando el CRC sobre el
/// paquete completo *incluyendo* el CRC añadido — el resultado debe ser 0x1D0F
/// (el residuo de este algoritmo).
///
/// # Ejemplos
///
/// ```
/// use spacepacket::crc::crc_ccitt;
///
/// // Vector de prueba ECSS: b"123456789" → 0x29B1
/// assert_eq!(crc_ccitt(b"123456789"), 0x29B1);
/// ```
pub fn crc_ccitt(data: &[u8]) -> u16 {
    let mut crc: u16 = 0xFFFF;
    for &byte in data {
        let pos = ((crc >> 8) ^ u16::from(byte)) as usize;
        crc = (crc << 8) ^ TABLE[pos];
    }
    crc
}

/// Añade el CRC-CCITT de 2 bytes a `buf` (big-endian).
pub fn append_crc(buf: &mut Vec<u8>) {
    let crc = crc_ccitt(buf);
    buf.push((crc >> 8) as u8);
    buf.push(crc as u8);
}

/// Verifica y elimina el CRC de 2 bytes final de `buf`.
///
/// Devuelve `Err` si el CRC no es válido.
pub fn verify_and_strip_crc(buf: &[u8]) -> Result<&[u8], crate::error::PacketError> {
    if buf.len() < 2 {
        return Err(crate::error::PacketError::BufferTooShort { need: 2, got: buf.len() });
    }
    let (payload, crc_bytes) = buf.split_at(buf.len() - 2);
    let received = u16::from_be_bytes([crc_bytes[0], crc_bytes[1]]);
    let computed = crc_ccitt(payload);
    if computed != received {
        return Err(crate::error::PacketError::CrcMismatch { computed, received });
    }
    Ok(payload)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ecss_test_vector() {
        // Vector de prueba CRC-CCITT oficial de ECSS
        assert_eq!(crc_ccitt(b"123456789"), 0x29B1);
    }

    #[test]
    fn roundtrip() {
        let data = b"Hello, spacecraft!";
        let mut buf = data.to_vec();
        append_crc(&mut buf);
        let stripped = verify_and_strip_crc(&buf).unwrap();
        assert_eq!(stripped, data);
    }

    #[test]
    fn detects_corruption() {
        let mut buf = b"some packet data".to_vec();
        append_crc(&mut buf);
        // Invertir un bit
        buf[3] ^= 0x01;
        assert!(verify_and_strip_crc(&buf).is_err());
    }
}
