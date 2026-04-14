//! Exercise 1 — Solution (fixed codec + tests that found the bug)

use proptest::prelude::*;

// ── Fixed COBS implementation ──────────────────────────────────────────────────

pub fn cobs_encode(data: &[u8]) -> Vec<u8> {
    let mut output = Vec::with_capacity(data.len() + data.len() / 254 + 2);
    let mut code_pos = 0usize;
    output.push(0x01);
    let mut code = 1u8;
    for &byte in data {
        if byte == 0x00 {
            output[code_pos] = code;
            code_pos = output.len();
            output.push(0x01);
            code = 1;
        } else {
            output.push(byte);
            code += 1;
            if code == 0xFF {
                output[code_pos] = code;
                code_pos = output.len();
                output.push(0x01); // FIX: this was missing in the buggy version
                code = 1;
            }
        }
    }
    output[code_pos] = code;
    output
}

pub fn cobs_decode(encoded: &[u8]) -> Option<Vec<u8>> {
    if encoded.is_empty() { return Some(vec![]); }
    let mut output = Vec::with_capacity(encoded.len());
    let mut pos = 0;
    while pos < encoded.len() {
        let code = encoded[pos] as usize;
        if code == 0 { return None; }
        let end = pos + code;
        if end > encoded.len() { return None; }
        output.extend_from_slice(&encoded[pos + 1..end]);
        pos = end;
        if code < 0xFF && pos < encoded.len() { output.push(0x00); }
    }
    if output.last() == Some(&0x00) { output.pop(); }
    Some(output)
}

proptest! {
    #[test]
    fn roundtrip(data: Vec<u8>) {
        let encoded = cobs_encode(&data);
        let decoded = cobs_decode(&encoded).expect("decode should succeed");
        prop_assert_eq!(decoded, data);
    }

    #[test]
    fn no_zeros_in_output(data: Vec<u8>) {
        let encoded = cobs_encode(&data);
        prop_assert!(!encoded.contains(&0u8));
    }
}

// ── Regression test: exact 254-byte non-zero run ──────────────────────────────
#[test]
fn regression_254_byte_run() {
    let data: Vec<u8> = (1u8..=254).collect(); // 254 non-zero bytes
    let encoded = cobs_encode(&data);
    assert!(!encoded.contains(&0u8));
    let decoded = cobs_decode(&encoded).unwrap();
    assert_eq!(decoded, data);
}

fn main() {
    println!("The bug: when a run of exactly 254 non-zero bytes is encoded,");
    println!("the next code byte placeholder was not pushed.");
    println!("proptest found this by generating [1..254] as a shrunken input.");
}
