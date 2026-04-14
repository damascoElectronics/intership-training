//! Exercise 1 — Implement a secure TC receiver
//!
//! Wire together HMAC verification + replay protection into a complete
//! TC receiver function.
//!
//! Run tests:  cargo test --example ex1_secure_tc_receiver

#![allow(dead_code, unused_variables)]

use hmac::{Hmac, Mac};
use sha2::Sha256;
use subtle::ConstantTimeEq;

type HmacSha256 = Hmac<Sha256>;

// ─── Pre-written types ────────────────────────────────────────────────────────

pub const TEST_KEY: &[u8] = b"test-key-for-training-only";

/// A raw TC packet with an appended 32-byte HMAC.
#[derive(Debug, Clone)]
pub struct RawTcWithHmac {
    /// The packet bytes (everything except the HMAC).
    pub payload: Vec<u8>,
    /// 32-byte HMAC-SHA256 over `payload`.
    pub mac: [u8; 32],
}

impl RawTcWithHmac {
    /// Creates a valid signed packet.
    pub fn new_signed(apid: u16, seq: u16, service: u8, subservice: u8, key: &[u8]) -> Self {
        let mut payload = Vec::new();
        payload.extend_from_slice(&apid.to_be_bytes());
        payload.extend_from_slice(&seq.to_be_bytes());
        payload.push(service);
        payload.push(subservice);
        let mut hmac = HmacSha256::new_from_slice(key).unwrap();
        hmac.update(&payload);
        let mac: [u8; 32] = hmac.finalize().into_bytes().into();
        Self { payload, mac }
    }

    pub fn seq(&self) -> u16 {
        u16::from_be_bytes([self.payload[2], self.payload[3]])
    }
}

/// A verified, authenticated TC (output of the receiver).
#[derive(Debug, Clone)]
pub struct VerifiedTc {
    pub apid: u16,
    pub seq: u16,
    pub service: u8,
    pub subservice: u8,
}

#[derive(Debug, PartialEq)]
pub enum RejectionReason {
    BadHmac,
    Replay,
    TooOld,
}

// ─── Your implementation ─────────────────────────────────────────────────────

/// Processes a stream of raw TC packets and returns only those that pass
/// HMAC verification and replay protection.
///
/// Returns a list of (result: Ok/Err) in order, one per input packet.
pub fn process_tc_stream(
    packets: &[RawTcWithHmac],
    key: &[u8],
) -> Vec<Result<VerifiedTc, RejectionReason>> {
    todo!(
        "For each packet:
         1. Verify HMAC (use HmacSha256::new_from_slice + update + verify constant-time)
         2. Check replay window (implement a simple seen-set or sliding window)
         3. If both pass: return Ok(VerifiedTc { ... })
         4. Otherwise: return Err(RejectionReason::BadHmac | Replay | TooOld)"
    )
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn make_stream() -> Vec<RawTcWithHmac> {
        vec![
            RawTcWithHmac::new_signed(0x001, 0, 17, 1, TEST_KEY),  // valid
            RawTcWithHmac::new_signed(0x001, 1, 17, 1, TEST_KEY),  // valid
            // bad HMAC: corrupt the MAC
            { let mut p = RawTcWithHmac::new_signed(0x001, 2, 3, 129, TEST_KEY); p.mac[0] ^= 0xFF; p },
            RawTcWithHmac::new_signed(0x001, 3, 3, 129, TEST_KEY), // valid
            // replay: same seq as second packet
            RawTcWithHmac::new_signed(0x001, 1, 17, 1, TEST_KEY),  // replay
            RawTcWithHmac::new_signed(0x001, 4, 17, 1, TEST_KEY),  // valid
            RawTcWithHmac::new_signed(0x001, 5, 17, 1, TEST_KEY),  // valid
        ]
    }

    #[test]
    fn correct_accept_reject_counts() {
        let stream = make_stream();
        let results = process_tc_stream(&stream, TEST_KEY);
        assert_eq!(results.len(), 7);

        let accepted: Vec<_> = results.iter().filter(|r| r.is_ok()).collect();
        let rejected: Vec<_> = results.iter().filter(|r| r.is_err()).collect();

        assert_eq!(accepted.len(), 5, "should accept 5 valid TCs");
        assert_eq!(rejected.len(), 2, "should reject 2 (bad HMAC + replay)");
    }

    #[test]
    fn bad_hmac_rejected() {
        let mut pkt = RawTcWithHmac::new_signed(0x001, 0, 17, 1, TEST_KEY);
        pkt.mac = [0u8; 32]; // wrong MAC
        let results = process_tc_stream(&[pkt], TEST_KEY);
        assert_eq!(results[0], Err(RejectionReason::BadHmac));
    }

    #[test]
    fn valid_packet_accepted() {
        let pkt = RawTcWithHmac::new_signed(0x001, 0, 17, 1, TEST_KEY);
        let results = process_tc_stream(&[pkt], TEST_KEY);
        assert!(results[0].is_ok());
        let tc = results[0].as_ref().unwrap();
        assert_eq!(tc.service, 17);
        assert_eq!(tc.subservice, 1);
    }
}

fn main() {
    println!("Run tests with: cargo test --example ex1_secure_tc_receiver");
}
