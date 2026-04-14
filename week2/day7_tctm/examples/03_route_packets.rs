//! Example 03 — APID routing
//!
//! A realistic spacecraft receives a stream of packets with different APIDs,
//! each destined for a different subsystem.  The router must quickly dispatch
//! each packet to the correct handler without parsing the full payload.
//!
//! Run with:  cargo run --example 03_route_packets

use spacepacket::{ApidRouter, PusTelecommand};
use std::sync::mpsc;
use std::thread;

// APID allocation table
const APID_OPS:    u16 = 0x001; // On-Board Operations (Service 17)
const APID_HK:     u16 = 0x002; // Housekeeping service (Service 3)
const APID_POWER:  u16 = 0x003; // Power Management
const APID_COMMS:  u16 = 0x004; // Communications

fn main() {
    // Each subsystem has its own receive channel
    let (tx_ops,   rx_ops)   = mpsc::sync_channel::<Vec<u8>>(16);
    let (tx_hk,    rx_hk)    = mpsc::sync_channel::<Vec<u8>>(16);
    let (tx_power, rx_power) = mpsc::sync_channel::<Vec<u8>>(16);
    let (tx_comms, rx_comms) = mpsc::sync_channel::<Vec<u8>>(16);

    // Build and wire the router
    let mut router = ApidRouter::new();
    router.register(APID_OPS,   tx_ops);
    router.register(APID_HK,    tx_hk);
    router.register(APID_POWER, tx_power);
    router.register(APID_COMMS, tx_comms);

    // Spawn subscriber threads to print what each subsystem receives
    spawn_subscriber("OPS   ", rx_ops);
    spawn_subscriber("HK    ", rx_hk);
    spawn_subscriber("POWER ", rx_power);
    spawn_subscriber("COMMS ", rx_comms);

    // Build a mixed stream of 12 packets and route them
    println!("Routing {} packets:", 12);
    let mut seq: u16 = 0;
    let packets: &[(u16, u8, u8, &str)] = &[
        (APID_OPS,   17,  1, "ping"),
        (APID_HK,     3, 129, "request HK report"),
        (APID_POWER,  6, 128, "dump memory"),
        (APID_COMMS,  9,   1, "set time"),
        (APID_OPS,   17,  1, "ping"),
        (APID_HK,     3, 130, "enable periodic HK"),
        (APID_POWER,  6,   2, "load memory"),
        (APID_COMMS,  9,   2, "request time"),
        (APID_HK,     3, 129, "request HK report"),
        (APID_OPS,   17,  1, "ping"),
        (APID_POWER,  6, 128, "dump memory"),
        (APID_COMMS,  9,   1, "set time"),
    ];

    for &(apid, svc, sub, desc) in packets {
        let pkt = PusTelecommand::new(apid, seq, svc, sub, 0, vec![]).unwrap().to_bytes();
        seq += 1;
        println!("  Sending TC({svc},{sub}) APID=0x{apid:03X} [{desc}]");
        match router.route(pkt) {
            Ok(()) => {}
            Err(e) => eprintln!("  ROUTING ERROR: {e}"),
        }
    }

    // Try routing to an unregistered APID
    println!();
    println!("Sending to unregistered APID 0x099...");
    let unknown = PusTelecommand::new(0x099, 99, 1, 1, 0, vec![]).unwrap().to_bytes();
    match router.route(unknown) {
        Ok(()) => println!("  (unexpectedly succeeded)"),
        Err(e) => println!("  Expected error: {e}"),
    }

    // Give threads a moment to print
    thread::sleep(std::time::Duration::from_millis(50));
    println!("\nRouting complete.");
}

fn spawn_subscriber(name: &'static str, rx: mpsc::Receiver<Vec<u8>>) {
    thread::spawn(move || {
        while let Ok(bytes) = rx.recv() {
            // Only parse the service/subservice from the PUS secondary header
            if bytes.len() >= 9 {
                let svc = bytes[7];
                let sub = bytes[8];
                println!("  [{name}] received TC({svc},{sub}) — {} bytes", bytes.len());
            }
        }
    });
}
