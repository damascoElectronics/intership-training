//! Byte-level encode/decode for [`CcsdsPrimaryHeader`].
//!
//! These functions form the boundary between the typed Rust world and the
//! raw byte buffers used by DMA controllers and network drivers. They are
//! kept intentionally thin: no allocation, no I/O, no dependencies.
//!
//! ## Why a Separate Module?
//!
//! Separating serialisation from the domain type follows the *Single
//! Responsibility Principle*: [`crate::frame::CcsdsPrimaryHeader`] knows about
//! header fields; this module knows about wire format. If CCSDS ever adds a
//! version-2 header format, only `codec.rs` needs to change.

use crate::error::CcsdsError;
use crate::frame::CcsdsPrimaryHeader;

/// Encodes a [`CcsdsPrimaryHeader`] into a 6-byte big-endian array.
///
/// The output is suitable for direct transmission: it can be written into a
/// DMA buffer, serialised into a UART frame, or prepended to a UDP payload.
///
/// This function is infallible because a [`CcsdsPrimaryHeader`] in memory is
/// always valid by construction (the constructors enforce the invariants).
///
/// # Performance
///
/// This is a 6-byte copy. No allocation occurs. The function inlines
/// to essentially a `memcpy` on release builds.
///
/// # Examples
///
/// ```rust
/// use day6_rustdoc::frame::CcsdsPrimaryHeader;
/// use day6_rustdoc::codec::{encode, decode};
///
/// let hdr = CcsdsPrimaryHeader::new_tc(0x42, 7, 20)?;
/// let bytes = encode(&hdr);
/// assert_eq!(bytes.len(), 6);
///
/// // The encoded bytes can be decoded back to an equivalent header.
/// let decoded = decode(&bytes)?;
/// assert_eq!(decoded.apid(), 0x42);
/// assert_eq!(decoded.seq_count(), 7);
/// # Ok::<(), day6_rustdoc::error::CcsdsError>(())
/// ```
pub fn encode(hdr: &CcsdsPrimaryHeader) -> [u8; 6] {
    // Delegate to the header's own to_bytes() so there is exactly one place
    // where the raw byte layout is authoritative.
    hdr.to_bytes()
}

/// Decodes a 6-byte array into a [`CcsdsPrimaryHeader`].
///
/// Validates the version field and APID. Returns an error if the bytes do not
/// represent a well-formed CCSDS primary header.
///
/// # Arguments
///
/// - `bytes`: Exactly 6 bytes in CCSDS big-endian wire order.
///
/// # Errors
///
/// - [`CcsdsError::UnsupportedVersion`] if bits \[15:13\] of byte 0 are non-zero.
/// - [`CcsdsError::InvalidApid`] if bits \[10:0\] of bytes 0–1 equal `0x7FF`.
///
/// # Examples
///
/// ```rust
/// use day6_rustdoc::frame::CcsdsPrimaryHeader;
/// use day6_rustdoc::codec::{encode, decode};
///
/// // Build → encode → decode round-trip.
/// let original = CcsdsPrimaryHeader::new_tm(0x10, 3, 64)?;
/// let bytes = encode(&original);
/// let recovered = decode(&bytes)?;
/// assert_eq!(original, recovered);
/// # Ok::<(), day6_rustdoc::error::CcsdsError>(())
/// ```
///
/// Handling a corrupt buffer:
///
/// ```rust
/// use day6_rustdoc::codec::decode;
/// use day6_rustdoc::error::CcsdsError;
///
/// // First byte 0x20 = version bits 001 — not 000 as required.
/// let bad_bytes = [0x20, 0x00, 0xC0, 0x00, 0x00, 0x03];
/// match decode(&bad_bytes) {
///     Err(CcsdsError::UnsupportedVersion { version }) => {
///         println!("Rejected corrupt header: version={}", version);
///     }
///     other => panic!("unexpected result: {:?}", other),
/// }
/// ```
pub fn decode(bytes: &[u8; 6]) -> Result<CcsdsPrimaryHeader, CcsdsError> {
    CcsdsPrimaryHeader::from_bytes(*bytes)
}

/// Decodes a primary header from a byte slice, checking the length first.
///
/// This is a convenience wrapper around [`decode`] for situations where the
/// input size is not statically known (e.g., reading from a socket buffer).
///
/// # Errors
///
/// - [`CcsdsError::BufferTooShort`] if `bytes.len() < 6`.
/// - All errors from [`decode`].
///
/// # Examples
///
/// ```rust
/// use day6_rustdoc::codec::decode_slice;
/// use day6_rustdoc::error::CcsdsError;
///
/// let short: &[u8] = &[0x18, 0x01];
/// match decode_slice(short) {
///     Err(CcsdsError::BufferTooShort { got, expected }) => {
///         assert_eq!(got, 2);
///         assert_eq!(expected, 6);
///     }
///     other => panic!("unexpected: {:?}", other),
/// }
/// ```
pub fn decode_slice(bytes: &[u8]) -> Result<CcsdsPrimaryHeader, CcsdsError> {
    if bytes.len() < 6 {
        return Err(CcsdsError::BufferTooShort {
            got: bytes.len(),
            expected: 6,
        });
    }
    // SAFETY: we just checked that bytes.len() >= 6.
    let arr: [u8; 6] = bytes[..6].try_into().expect("slice is exactly 6 bytes");
    decode(&arr)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::CcsdsError;

    #[test]
    fn encode_decode_round_trip() {
        let hdr = CcsdsPrimaryHeader::new_tc(0x100, 10, 50).unwrap();
        let bytes = encode(&hdr);
        let decoded = decode(&bytes).unwrap();
        assert_eq!(hdr, decoded);
    }

    #[test]
    fn decode_slice_too_short() {
        let short = [0u8; 4];
        match decode_slice(&short) {
            Err(CcsdsError::BufferTooShort { got: 4, expected: 6 }) => {}
            other => panic!("unexpected: {:?}", other),
        }
    }

    #[test]
    fn decode_slice_exact_length() {
        let hdr = CcsdsPrimaryHeader::new_tm(0x20, 99, 8).unwrap();
        let bytes: Vec<u8> = hdr.to_bytes().to_vec();
        let decoded = decode_slice(&bytes).unwrap();
        assert_eq!(hdr, decoded);
    }
}
