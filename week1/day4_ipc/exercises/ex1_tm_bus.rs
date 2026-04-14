//! Exercise 1 — TM bus: fan-out telemetry to subscribers
//!
//! Implement a simple telemetry bus that routes TM frames by APID.
//! Multiple producers push frames; a router dispatches to subscribers.
//!
//! Run tests:  cargo test --example ex1_tm_bus

#![allow(dead_code, unused_variables)]

use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;

// ─── Pre-written types ────────────────────────────────────────────────────────

/// A simple telemetry frame (stripped-down CCSDS-inspired structure).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TmFrame {
    pub apid: u16,
    pub seq_count: u16,
    pub service: u8,
    pub subservice: u8,
    pub data: Vec<u8>,
}

impl TmFrame {
    pub fn new(apid: u16, seq: u16, service: u8, subservice: u8, data: Vec<u8>) -> Self {
        Self { apid, seq_count: seq, service, subservice, data }
    }
}

// ─── Your implementation ─────────────────────────────────────────────────────

/// A TM bus router that forwards frames to registered subscribers by APID.
pub struct TmBus {
    /// TODO: add a HashMap<u16, Sender<TmFrame>> for routing
}

impl TmBus {
    /// Creates a new empty TM bus.
    pub fn new() -> Self {
        todo!("return Self with empty routing table")
    }

    /// Registers a subscriber for a specific APID.
    /// Returns the [`mpsc::Receiver`] end; the bus keeps the sender.
    pub fn subscribe(&mut self, apid: u16) -> mpsc::Receiver<TmFrame> {
        todo!("create mpsc::channel(16), store sender in routing table, return receiver")
    }

    /// Routes a frame to the subscriber registered for its APID.
    /// If no subscriber is registered, the frame is silently dropped.
    pub async fn publish(&self, frame: TmFrame) {
        todo!("look up frame.apid in routing table, send the frame if found")
    }
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::time::{timeout, Duration};

    #[tokio::test]
    async fn routes_to_correct_subscriber() {
        let mut bus = TmBus::new();
        let mut rx_hk     = bus.subscribe(0x003);
        let mut rx_sensor = bus.subscribe(0x002);

        bus.publish(TmFrame::new(0x003, 1, 3, 25, vec![1, 2, 3])).await;
        bus.publish(TmFrame::new(0x002, 2, 2,  1, vec![4, 5, 6])).await;
        bus.publish(TmFrame::new(0x003, 3, 3, 25, vec![7, 8, 9])).await;

        // rx_hk should receive APID 0x003 frames
        let f1 = timeout(Duration::from_millis(50), rx_hk.recv()).await.unwrap().unwrap();
        assert_eq!(f1.apid, 0x003);
        assert_eq!(f1.seq_count, 1);

        let f2 = timeout(Duration::from_millis(50), rx_hk.recv()).await.unwrap().unwrap();
        assert_eq!(f2.seq_count, 3);

        // rx_sensor should receive APID 0x002 frames
        let f3 = timeout(Duration::from_millis(50), rx_sensor.recv()).await.unwrap().unwrap();
        assert_eq!(f3.apid, 0x002);
        assert_eq!(f3.seq_count, 2);
    }

    #[tokio::test]
    async fn unregistered_apid_dropped_silently() {
        let bus = TmBus::new(); // no subscribers
        // Should not panic or block
        bus.publish(TmFrame::new(0x099, 0, 1, 1, vec![])).await;
    }
}

#[tokio::main]
async fn main() {
    println!("Run tests with: cargo test --example ex1_tm_bus");
}
