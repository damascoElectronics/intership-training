//! Example 01 — Property-based testing of CCSDS packet round-trips
//!
//! Property-based testing generates MANY random inputs and checks that
//! your invariants hold for ALL of them.  Compare to example-based testing
//! which only checks a few hand-picked cases.
//!
//! The fundamental property for any codec: parse(serialize(x)) == x
//!
//! Run tests with:  cargo test --example 01_proptest_packet

use proptest::prelude::*;

// ── Inline minimal CCSDS primary header (to avoid cross-crate deps) ────────────

#[derive(Debug, Clone, PartialEq)]
struct Header {
    is_tc: bool,
    apid: u16,       // 11-bit: 0..=0x7FE
    seq_count: u16,  // 14-bit: 0..=0x3FFF
    data_len: u16,   // as stored: actual_len - 1
}

impl Header {
    fn serialize(&self) -> [u8; 6] {
        let mut raw = [0u8; 6];
        let word0: u16 = ((self.is_tc as u16) << 12) | (1 << 11) | (self.apid & 0x07FF);
        raw[0] = (word0 >> 8) as u8;
        raw[1] = word0 as u8;
        let word1: u16 = (0b11u16 << 14) | (self.seq_count & 0x3FFF);
        raw[2] = (word1 >> 8) as u8;
        raw[3] = word1 as u8;
        raw[4] = (self.data_len >> 8) as u8;
        raw[5] = self.data_len as u8;
        raw
    }

    fn parse(raw: [u8; 6]) -> Option<Self> {
        let version = (raw[0] >> 5) & 0x07;
        if version != 0 { return None; }
        let word0 = u16::from_be_bytes([raw[0], raw[1]]);
        let apid = word0 & 0x07FF;
        if apid > 0x7FE { return None; }
        let is_tc = (word0 >> 12) & 1 == 1;
        let word1 = u16::from_be_bytes([raw[2], raw[3]]);
        let seq_count = word1 & 0x3FFF;
        let data_len = u16::from_be_bytes([raw[4], raw[5]]);
        Some(Self { is_tc, apid, seq_count, data_len })
    }
}

// ── Properties ─────────────────────────────────────────────────────────────────

proptest! {
    /// For any valid APID, the serialized header can be parsed back identically.
    #[test]
    fn roundtrip_apid(apid in 0u16..=0x7FEu16) {
        let h = Header { is_tc: true, apid, seq_count: 0, data_len: 0 };
        let parsed = Header::parse(h.serialize()).expect("must parse");
        prop_assert_eq!(parsed.apid, apid);
    }

    /// For any valid sequence count, round-trip preserves it.
    #[test]
    fn roundtrip_seq_count(seq in 0u16..=0x3FFFu16) {
        let h = Header { is_tc: false, apid: 0x100, seq_count: seq, data_len: 0 };
        let parsed = Header::parse(h.serialize()).unwrap();
        prop_assert_eq!(parsed.seq_count, seq);
    }

    /// Packet type flag round-trips correctly.
    #[test]
    fn roundtrip_packet_type(is_tc: bool) {
        let h = Header { is_tc, apid: 0x050, seq_count: 42, data_len: 10 };
        let parsed = Header::parse(h.serialize()).unwrap();
        prop_assert_eq!(parsed.is_tc, is_tc);
    }

    /// parse(serialize(x)) == x for all valid combinations.
    #[test]
    fn full_roundtrip(
        is_tc: bool,
        apid in 0u16..=0x7FEu16,
        seq in 0u16..=0x3FFFu16,
        data_len: u16,
    ) {
        let h = Header { is_tc, apid, seq_count: seq, data_len };
        let parsed = Header::parse(h.serialize()).expect("must parse");
        prop_assert_eq!(parsed, h);
    }

    /// Parsing arbitrary 6 bytes must never PANIC — it may return None.
    #[test]
    fn no_panic_on_arbitrary_bytes(bytes: [u8; 6]) {
        // Header::parse returns Option — must not panic
        let _ = Header::parse(bytes);
    }
}

fn main() {
    println!("Run with: cargo test --example 01_proptest_packet");
    println!();
    println!("proptest will generate 256 random inputs for each property.");
    println!("When a failure is found, it shrinks to the MINIMAL failing case.");
    println!("This is much more powerful than writing 10 hand-picked test cases.");
}
