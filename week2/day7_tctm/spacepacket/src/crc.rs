//! CRC-CCITT (CRC-16/CCITT-FALSE) as required by ECSS-E-ST-70-41C §A.3.
//!
//! Parameters:
//! - Polynomial : 0x1021
//! - Initial value: 0xFFFF
//! - Input reflection : false
//! - Output reflection: false
//! - XOR out : 0x0000
//!
//! This is a table-driven implementation (O(n) with small constant).
//! The lookup table is computed at compile time.

/// Pre-computed CRC-CCITT lookup table (polynomial 0x1021).
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

/// Computes the CRC-CCITT checksum over `data`.
///
/// The checksum is appended as the last 2 bytes of every PUS packet
/// (big-endian, MSB first).  A receiver verifies by running CRC over the
/// entire packet *including* the appended CRC — the result should be 0x1D0F
/// (the residue of this algorithm).
///
/// # Examples
///
/// ```
/// use spacepacket::crc::crc_ccitt;
///
/// // ECSS test vector: b"123456789" → 0x29B1
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

/// Appends the 2-byte CRC-CCITT to `buf` (big-endian).
pub fn append_crc(buf: &mut Vec<u8>) {
    let crc = crc_ccitt(buf);
    buf.push((crc >> 8) as u8);
    buf.push(crc as u8);
}

/// Verifies and strips the trailing 2-byte CRC from `buf`.
///
/// Returns `Err` if the CRC is invalid.
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
        // Official ECSS CRC-CCITT test vector
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
        // Flip a bit
        buf[3] ^= 0x01;
        assert!(verify_and_strip_crc(&buf).is_err());
    }
}
