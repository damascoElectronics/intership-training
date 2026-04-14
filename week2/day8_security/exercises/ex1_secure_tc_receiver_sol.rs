//! Exercise 1 — Solution

#![allow(dead_code)]

use hmac::{Hmac, Mac};
use sha2::Sha256;
use std::collections::HashSet;
use subtle::ConstantTimeEq;

type HmacSha256 = Hmac<Sha256>;

pub const TEST_KEY: &[u8] = b"test-key-for-training-only";

#[derive(Debug, Clone)]
pub struct RawTcWithHmac { pub payload: Vec<u8>, pub mac: [u8; 32] }

impl RawTcWithHmac {
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
    pub fn seq(&self) -> u16 { u16::from_be_bytes([self.payload[2], self.payload[3]]) }
}

#[derive(Debug, Clone)]
pub struct VerifiedTc { pub apid: u16, pub seq: u16, pub service: u8, pub subservice: u8 }

#[derive(Debug, PartialEq)]
pub enum RejectionReason { BadHmac, Replay, TooOld }

pub fn process_tc_stream(packets: &[RawTcWithHmac], key: &[u8]) -> Vec<Result<VerifiedTc, RejectionReason>> {
    let mut seen_seqs: HashSet<u16> = HashSet::new();
    let mut results = Vec::new();

    for pkt in packets {
        // 1. Verify HMAC
        let mut mac = HmacSha256::new_from_slice(key).unwrap();
        mac.update(&pkt.payload);
        let expected: [u8; 32] = mac.finalize().into_bytes().into();
        if !expected.ct_eq(&pkt.mac).into() {
            results.push(Err(RejectionReason::BadHmac));
            continue;
        }

        // 2. Replay check
        let seq = pkt.seq();
        if seen_seqs.contains(&seq) {
            results.push(Err(RejectionReason::Replay));
            continue;
        }
        seen_seqs.insert(seq);

        // 3. Parse and accept
        let apid = u16::from_be_bytes([pkt.payload[0], pkt.payload[1]]);
        let service = pkt.payload[4];
        let subservice = pkt.payload[5];
        results.push(Ok(VerifiedTc { apid, seq, service, subservice }));
    }
    results
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_stream() -> Vec<RawTcWithHmac> {
        vec![
            RawTcWithHmac::new_signed(0x001, 0, 17, 1, TEST_KEY),
            RawTcWithHmac::new_signed(0x001, 1, 17, 1, TEST_KEY),
            { let mut p = RawTcWithHmac::new_signed(0x001, 2, 3, 129, TEST_KEY); p.mac[0] ^= 0xFF; p },
            RawTcWithHmac::new_signed(0x001, 3, 3, 129, TEST_KEY),
            RawTcWithHmac::new_signed(0x001, 1, 17, 1, TEST_KEY),
            RawTcWithHmac::new_signed(0x001, 4, 17, 1, TEST_KEY),
            RawTcWithHmac::new_signed(0x001, 5, 17, 1, TEST_KEY),
        ]
    }

    #[test]
    fn correct_accept_reject_counts() {
        let results = process_tc_stream(&make_stream(), TEST_KEY);
        assert_eq!(results.iter().filter(|r| r.is_ok()).count(), 5);
        assert_eq!(results.iter().filter(|r| r.is_err()).count(), 2);
    }

    #[test]
    fn bad_hmac_rejected() {
        let mut pkt = RawTcWithHmac::new_signed(0x001, 0, 17, 1, TEST_KEY);
        pkt.mac = [0u8; 32];
        assert_eq!(process_tc_stream(&[pkt], TEST_KEY)[0], Err(RejectionReason::BadHmac));
    }

    #[test]
    fn valid_packet_accepted() {
        let pkt = RawTcWithHmac::new_signed(0x001, 0, 17, 1, TEST_KEY);
        let results = process_tc_stream(&[pkt], TEST_KEY);
        assert!(results[0].is_ok());
    }
}

fn main() { println!("Run tests with: cargo test --example ex1_secure_tc_receiver_sol"); }
