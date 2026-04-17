//! Ejercicio 1 — Solución

#![allow(dead_code)]

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tokio::sync::mpsc;

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

pub struct TmBus {
    routes: HashMap<u16, mpsc::Sender<TmFrame>>,
}

impl TmBus {
    pub fn new() -> Self { Self { routes: HashMap::new() } }

    pub fn subscribe(&mut self, apid: u16) -> mpsc::Receiver<TmFrame> {
        let (tx, rx) = mpsc::channel(16);
        self.routes.insert(apid, tx);
        rx
    }

    pub async fn publish(&self, frame: TmFrame) {
        if let Some(tx) = self.routes.get(&frame.apid) {
            let _ = tx.send(frame).await; // descartar silenciosamente si el receptor está cerrado
        }
    }
}

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

        let f1 = timeout(Duration::from_millis(50), rx_hk.recv()).await.unwrap().unwrap();
        assert_eq!(f1.apid, 0x003);
        assert_eq!(f1.seq_count, 1);

        let f2 = timeout(Duration::from_millis(50), rx_hk.recv()).await.unwrap().unwrap();
        assert_eq!(f2.seq_count, 3);

        let f3 = timeout(Duration::from_millis(50), rx_sensor.recv()).await.unwrap().unwrap();
        assert_eq!(f3.apid, 0x002);
        assert_eq!(f3.seq_count, 2);
    }

    #[tokio::test]
    async fn unregistered_apid_dropped_silently() {
        let bus = TmBus::new();
        bus.publish(TmFrame::new(0x099, 0, 1, 1, vec![])).await;
    }
}

#[tokio::main]
async fn main() {
    println!("Ejecutar pruebas con: cargo test --example ex1_tm_bus_sol");
}
