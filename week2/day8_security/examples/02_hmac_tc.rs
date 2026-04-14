//! Example 02 — HMAC-SHA256 for TC authentication
//!
//! HMAC authenticates that a TC came from someone who holds the key AND
//! that the bytes weren't modified in transit.  It does NOT encrypt.
//!
//! Critical: ALWAYS use constant-time comparison (subtle crate) — otherwise
//! a timing attack can recover the expected MAC byte by byte.
//!
//! Run with:  cargo run --example 02_hmac_tc

use hmac::{Hmac, Mac};
use sha2::Sha256;
use subtle::ConstantTimeEq;

type HmacSha256 = Hmac<Sha256>;

/// Signs `packet_bytes` with HMAC-SHA256, returning the 32-byte MAC.
pub fn sign_tc(packet_bytes: &[u8], key: &[u8]) -> [u8; 32] {
    let mut mac = HmacSha256::new_from_slice(key)
        .expect("HMAC accepts keys of any length");
    mac.update(packet_bytes);
    mac.finalize().into_bytes().into()
}

/// Verifies a TC's HMAC.  Uses constant-time comparison.
///
/// # Timing attacks
/// A naive `if computed == expected` leaks timing information: it returns
/// early on the first mismatched byte.  An attacker can send millions of
/// packets with different MACs and measure which byte positions cause early
/// returns, eventually reconstructing the expected MAC.
///
/// `ConstantTimeEq` always compares all 32 bytes regardless of content.
pub fn verify_tc(packet_bytes: &[u8], claimed_mac: &[u8; 32], key: &[u8]) -> bool {
    let expected = sign_tc(packet_bytes, key);
    // WRONG: expected == *claimed_mac  ← timing attack!
    // RIGHT: constant-time comparison:
    expected.ct_eq(claimed_mac).into()
}

/// TC packet structure for this demo.
struct TcPacket {
    apid: u16,
    service: u8,
    subservice: u8,
    data: Vec<u8>,
}

impl TcPacket {
    fn serialize(&self) -> Vec<u8> {
        let mut buf = Vec::new();
        buf.extend_from_slice(&self.apid.to_be_bytes());
        buf.push(self.service);
        buf.push(self.subservice);
        buf.extend_from_slice(&self.data);
        buf
    }
}

fn main() {
    let key = b"spacecraft-shared-secret-key-256b";
    let wrong_key = b"wrong-key-someone-else-signed-it";

    let tc = TcPacket {
        apid: 0x001,
        service: 17,
        subservice: 1,
        data: vec![0xDE, 0xAD, 0xBE, 0xEF],
    };
    let packet_bytes = tc.serialize();

    println!("=== HMAC-SHA256 TC Authentication ===\n");
    println!("Packet: {:02X?}", packet_bytes);

    // Sign with the correct key
    let mac = sign_tc(&packet_bytes, key);
    println!("\nMAC (32 bytes): {:02X?}", &mac[..8]);
    println!("              (showing first 8 of 32 bytes)");

    // Verify scenarios
    println!("\nVerification scenarios:");
    println!("  correct key:  {}", if verify_tc(&packet_bytes, &mac, key) { "✓ VALID" } else { "✗ INVALID" });
    println!("  wrong key:    {}", if verify_tc(&packet_bytes, &mac, wrong_key) { "✓ VALID" } else { "✗ INVALID" });

    // Modified packet (bit flip)
    let mut tampered = packet_bytes.clone();
    tampered[2] ^= 0x01;
    println!("  tampered pkt: {}", if verify_tc(&tampered, &mac, key) { "✓ VALID" } else { "✗ INVALID" });

    println!("\nKey insight: the MAC covers the ENTIRE payload.");
    println!("Flipping a single bit in the packet makes verification fail.");
    println!("This protects against both tampering AND injection of new commands.");
}
