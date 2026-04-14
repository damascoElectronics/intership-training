//! Example 04 — Sequence counter management
//!
//! Every CCSDS packet carries a 14-bit sequence count (0–0x3FFF per APID).
//! The ground checks these to detect lost packets, duplicate packets, or
//! replay attacks.
//!
//! Key rules:
//!   - Each APID has its OWN independent counter
//!   - The counter increments with every packet
//!   - It wraps 0x3FFF → 0x0000 (not 0xFFFF)
//!   - A gap means packets were lost
//!
//! Run with:  cargo run --example 04_sequence_counter

use spacepacket::primary_header::CcsdsPrimaryHeader;
use std::collections::HashMap;

/// Per-APID sequence counter tracker (spacecraft side — for transmitting).
struct SeqCounters {
    counters: HashMap<u16, u16>,
}

impl SeqCounters {
    fn new() -> Self { Self { counters: HashMap::new() } }

    /// Returns the NEXT sequence count for this APID, then increments.
    fn next(&mut self, apid: u16) -> u16 {
        let counter = self.counters.entry(apid).or_insert(0);
        let value = *counter;
        *counter = CcsdsPrimaryHeader::next_seq(value);
        value
    }
}

/// Ground-side gap detector.
struct GapDetector {
    expected: HashMap<u16, u16>,
    gaps_found: u32,
}

impl GapDetector {
    fn new() -> Self { Self { expected: HashMap::new(), gaps_found: 0 } }

    fn check(&mut self, apid: u16, seq: u16) {
        let expected = self.expected.entry(apid).or_insert(seq);
        if seq != *expected {
            // Calculate how many packets were lost, accounting for wrap-around
            let lost = if seq > *expected {
                seq - *expected
            } else {
                (0x4000 - *expected) + seq  // wrap-around case
            };
            println!(
                "  [GAP DETECTED] APID 0x{apid:03X}: expected seq={expected}, got seq={seq} — {} packet(s) lost",
                lost
            );
            self.gaps_found += 1;
        }
        *expected = CcsdsPrimaryHeader::next_seq(seq);
    }
}

fn main() {
    println!("=== Sequence Counter Demo ===\n");

    let mut counters = SeqCounters::new();
    let mut detector = GapDetector::new();

    // Simulate normal transmission for APID 0x100
    println!("--- Normal packet stream (APID 0x100) ---");
    for _ in 0..5 {
        let seq = counters.next(0x100);
        println!("  Sending APID=0x100 seq={seq}");
        detector.check(0x100, seq);
    }

    // Simulate a gap (packet 5 and 6 are lost, only 7 arrives)
    println!("\n--- Simulating 2 lost packets ---");
    let seq = counters.next(0x100);
    println!("  Packet seq={seq} 'lost in transmission'");
    counters.next(0x100); // also lost
    let seq_after_gap = counters.next(0x100);
    println!("  Sending APID=0x100 seq={seq_after_gap} (ground sees gap)");
    detector.check(0x100, seq_after_gap);

    // Independent counters per APID
    println!("\n--- Independent counters (APID 0x200 starts fresh) ---");
    for i in 0..3 {
        let seq_100 = counters.next(0x100);
        let seq_200 = counters.next(0x200);
        println!("  0x100 seq={seq_100}, 0x200 seq={seq_200} (iteration {i})");
        detector.check(0x100, seq_100);
        detector.check(0x200, seq_200);
    }

    // Demonstrate the 14-bit wrap-around
    println!("\n--- 14-bit wrap-around at 0x3FFF ---");
    let mut wrap_counter: u16 = 0x3FFD;
    for _ in 0..5 {
        println!("  seq = 0x{wrap_counter:04X} ({wrap_counter})");
        wrap_counter = CcsdsPrimaryHeader::next_seq(wrap_counter);
    }

    println!("\n--- Summary ---");
    println!("Gaps detected: {}", detector.gaps_found);
}
