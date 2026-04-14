//! Example 03 — Replay attack protection with a sliding window
//!
//! Even if HMAC prevents forgery, an attacker can record a valid TC and
//! send it again later (replay attack).  A sequence number window defeats this.
//!
//! Run with:  cargo run --example 03_replay_protection

/// 64-packet sliding window replay protector.
pub struct ReplayWindow {
    last_seq: u16,
    /// Bitmask: bit N set means seq (last_seq − N) was already accepted.
    window: u64,
    initialized: bool,
}

#[derive(Debug, PartialEq)]
pub enum ReplayResult {
    Accept,
    Replay,
    TooOld,
}

impl ReplayWindow {
    pub fn new() -> Self { Self { last_seq: 0, window: 0, initialized: false } }

    pub fn check_and_advance(&mut self, seq: u16) -> ReplayResult {
        if !self.initialized {
            self.last_seq = seq;
            self.window = 1;
            self.initialized = true;
            return ReplayResult::Accept;
        }

        // Use 14-bit arithmetic (CCSDS seq count wraps at 0x3FFF)
        let diff = (seq as i32 - self.last_seq as i32).rem_euclid(0x4000) as u16;

        if diff == 0 {
            return ReplayResult::Replay; // exact duplicate
        }

        if diff <= 64 {
            // Packet is ahead of us — advance window
            self.window = self.window.wrapping_shl(diff as u32) | 1;
            self.last_seq = seq;
            ReplayResult::Accept
        } else if diff > 0x3FC0 {
            // Packet is behind us (diff would be negative in signed arithmetic)
            let back = (0x4000u32 - diff as u32) as usize;
            if back >= 64 { return ReplayResult::TooOld; }
            if self.window & (1u64 << back) != 0 { return ReplayResult::Replay; }
            self.window |= 1u64 << back;
            ReplayResult::Accept
        } else {
            // Very far ahead — large gap in sequence (accept, advance window)
            self.window = 1;
            self.last_seq = seq;
            ReplayResult::Accept
        }
    }
}

fn main() {
    let mut window = ReplayWindow::new();

    println!("=== Replay Window Demo ===\n");

    let scenarios: &[(u16, &str)] = &[
        (100, "first packet"),
        (101, "sequential"),
        (102, "sequential"),
        (104, "gap (103 lost)"),
        (103, "out-of-order but within window"),
        (101, "REPLAY of seq=101"),
        (100, "REPLAY of initial seq=100"),
        (90,  "TOO OLD (> 64 behind current)"),
        (105, "back to normal"),
    ];

    for &(seq, desc) in scenarios {
        let result = window.check_and_advance(seq);
        let status = match result {
            ReplayResult::Accept  => "ACCEPT",
            ReplayResult::Replay  => "REJECT (replay)",
            ReplayResult::TooOld  => "REJECT (too old)",
        };
        println!("  seq={seq:4}  [{status:25}]  {desc}");
    }

    println!();
    println!("The window approach accepts out-of-order packets within 64 of the");
    println!("latest seen — necessary because the RF uplink may reorder packets.");
    println!("It rejects exact duplicates and packets too far in the past.");
}
