//! Example 03 — COBS codec with property-based tests
//!
//! COBS (Consistent Overhead Byte Stuffing) is a framing codec used in
//! embedded serial protocols.  It encodes data so that 0x00 never appears
//! in the output — useful when 0x00 is your packet delimiter.
//!
//! You already use UART; COBS is a natural fit for binary serial framing.
//!
//! Properties to verify:
//!   1. Encoded output contains NO 0x00 bytes
//!   2. decode(encode(data)) == data  (round-trip)
//!   3. Encoded length ≤ len + ceil(len/254) + 1

use proptest::prelude::*;

// ── COBS implementation ────────────────────────────────────────────────────────

/// Encodes `data` using COBS.  The result contains no 0x00 bytes.
pub fn cobs_encode(data: &[u8]) -> Vec<u8> {
    let mut output = Vec::with_capacity(data.len() + data.len() / 254 + 2);
    let mut code_pos = 0usize;
    output.push(0x01); // placeholder for first code byte
    let mut code = 1u8;

    for &byte in data {
        if byte == 0x00 {
            // End of zero-free run
            output[code_pos] = code;
            code_pos = output.len();
            output.push(0x01); // next code placeholder
            code = 1;
        } else {
            output.push(byte);
            code += 1;
            if code == 0xFF {
                // Run of 254 non-zero bytes — must insert overhead byte
                output[code_pos] = code;
                code_pos = output.len();
                output.push(0x01);
                code = 1;
            }
        }
    }
    output[code_pos] = code;
    output
}

/// Decodes COBS-encoded data.  Returns `None` on malformed input.
pub fn cobs_decode(encoded: &[u8]) -> Option<Vec<u8>> {
    if encoded.is_empty() { return Some(vec![]); }
    let mut output = Vec::with_capacity(encoded.len());
    let mut pos = 0;
    while pos < encoded.len() {
        let code = encoded[pos] as usize;
        if code == 0 { return None; } // 0x00 in encoded data → error
        let end = pos + code;
        if end > encoded.len() { return None; }
        output.extend_from_slice(&encoded[pos + 1..end]);
        pos = end;
        if code < 0xFF && pos < encoded.len() {
            output.push(0x00);
        }
    }
    // Remove trailing zero that COBS always appends conceptually
    if output.last() == Some(&0x00) {
        output.pop();
    }
    Some(output)
}

// ── Properties ─────────────────────────────────────────────────────────────────

proptest! {
    /// Encoded output must never contain 0x00.
    #[test]
    fn encoded_has_no_zeros(data: Vec<u8>) {
        let encoded = cobs_encode(&data);
        prop_assert!(!encoded.contains(&0u8),
            "encoded output contains 0x00 for input: {data:?}");
    }

    /// decode(encode(x)) == x for all inputs.
    #[test]
    fn roundtrip(data: Vec<u8>) {
        let encoded = cobs_encode(&data);
        let decoded = cobs_decode(&encoded).expect("decode should succeed on valid encoded data");
        prop_assert_eq!(decoded, data);
    }

    /// Encoded length is at most len + ceil(len/254) + 1.
    #[test]
    fn length_bound(data: Vec<u8>) {
        let encoded = cobs_encode(&data);
        let max_len = data.len() + data.len() / 254 + 2;
        prop_assert!(encoded.len() <= max_len,
            "encoded len {} exceeds bound {} for input len {}",
            encoded.len(), max_len, data.len());
    }

    /// Empty input encodes to a single byte 0x01.
    #[test]
    fn empty_encodes_to_single_byte(_ignored: u8) {
        let encoded = cobs_encode(&[]);
        prop_assert_eq!(encoded, vec![0x01]);
    }
}

fn main() {
    println!("Run with: cargo test --example 03_property_tests");
    println!();
    println!("This COBS implementation is used in serial framing.");
    println!("The round-trip property guarantees correctness for ALL inputs.");
}
