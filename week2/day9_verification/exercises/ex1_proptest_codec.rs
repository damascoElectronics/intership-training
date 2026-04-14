//! Exercise 1 — Find the bug with proptest
//!
//! The COBS implementation below has a subtle off-by-one bug.
//! Write proptest properties that find it.
//!
//! Hint: the bug only manifests for input lengths that are exact multiples of 254.
//!
//! Run: cargo test --example ex1_proptest_codec

use proptest::prelude::*;

// ── Buggy COBS implementation ──────────────────────────────────────────────────
// DO NOT FIX this implementation — the goal is to FIND the bug with proptest.

pub fn cobs_encode_buggy(data: &[u8]) -> Vec<u8> {
    let mut output = Vec::with_capacity(data.len() + 2);
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
                // BUG: should push 0x01 here (next code placeholder) but doesn't
                // This corrupts output for inputs where a 254-byte run ends exactly
                code = 1;
            }
        }
    }
    output[code_pos] = code;
    output
}

pub fn cobs_decode_buggy(encoded: &[u8]) -> Option<Vec<u8>> {
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
        if code < 0xFF && pos < encoded.len() {
            output.push(0x00);
        }
    }
    if output.last() == Some(&0x00) { output.pop(); }
    Some(output)
}

// ── Your tests ────────────────────────────────────────────────────────────────

proptest! {
    /// TODO: Write a property that finds the bug.
    ///
    /// The round-trip property will fail for some input.
    /// When proptest shrinks the failing case, you should see
    /// what kind of input triggers the bug.
    #[test]
    fn roundtrip_should_find_bug(data: Vec<u8>) {
        todo!("write: encode, decode, assert equal — proptest will find the bug")
    }

    /// TODO: Write a property checking no 0x00 in output.
    /// Does the bug affect this property too?
    #[test]
    fn no_zeros_in_output(data: Vec<u8>) {
        todo!("encode and assert no 0x00 bytes in result")
    }
}

fn main() {
    println!("Run: cargo test --example ex1_proptest_codec");
    println!("When proptest finds the failing case, it will shrink it.");
    println!("Expected: fails for inputs with a run of 254+ non-zero bytes.");
}
