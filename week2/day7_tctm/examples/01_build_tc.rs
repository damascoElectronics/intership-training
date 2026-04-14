//! Example 01 — Build a TC(17,1) "Are You Alive" ping
//!
//! Service 17 (On-Board Operations) Subservice 1 (Are You Alive) is the
//! simplest possible telecommand — a spacecraft ping.  The expected response
//! is TM(17,2) "I Am Alive".
//!
//! Run with:  cargo run --example 01_build_tc

use spacepacket::PusTelecommand;

fn main() {
    // APID allocation (example):
    //   0x001  →  On-Board Operations service (PUS 17)
    //   0x100  →  Attitude Control System
    //   0x200  →  Power Management
    //   0x300  →  Communications
    let apid = 0x001u16;
    let seq_count = 1u16;

    let tc = PusTelecommand::new(
        apid,
        seq_count,
        17,   // PUS service 17: On-Board Operations
        1,    // Subservice 1: Are You Alive
        0,    // source_id 0: ground station #1
        vec![], // no app data for a simple ping
    )
    .expect("valid TC");

    let bytes = tc.to_bytes();

    println!("TC(17,1) — Are You Alive ping");
    println!("  APID:         0x{:03X}", tc.apid());
    println!("  Seq count:    {}", tc.seq_count());
    println!("  Service:      {}", tc.service());
    println!("  Subservice:   {}", tc.subservice());
    println!("  Total bytes:  {}", bytes.len());
    println!();
    println!("Raw bytes (hex):");
    print!("  ");
    for (i, byte) in bytes.iter().enumerate() {
        if i > 0 && i % 8 == 0 { print!("\n  "); }
        print!("{:02X} ", byte);
    }
    println!();
    println!();

    // Annotate each section:
    println!("Field breakdown:");
    println!("  Bytes 0-5:   CCSDS primary header");
    println!("    [0-1] packet ID (version|type|sec_hdr|APID): {:02X}{:02X}", bytes[0], bytes[1]);
    println!("    [2-3] sequence control (flags|seq_count):    {:02X}{:02X}", bytes[2], bytes[3]);
    println!("    [4-5] data length - 1:                       {:02X}{:02X}", bytes[4], bytes[5]);
    println!("  Bytes 6-10:  PUS-C secondary header");
    println!("    [6]   PUS version + spare:  {:02X}", bytes[6]);
    println!("    [7]   service type:         {:02X} ({})", bytes[7], bytes[7]);
    println!("    [8]   subservice type:      {:02X} ({})", bytes[8], bytes[8]);
    println!("    [9-10] source ID:           {:02X}{:02X}", bytes[9], bytes[10]);
    println!("  Last 2 bytes: CRC-CCITT:      {:02X}{:02X}", bytes[bytes.len()-2], bytes[bytes.len()-1]);

    // Verify we can round-trip it
    let parsed = PusTelecommand::from_bytes(&bytes).expect("should parse cleanly");
    assert_eq!(parsed.service(), 17);
    assert_eq!(parsed.subservice(), 1);
    println!("\nRound-trip parse: OK");
}
