//! Ejercicio 1 — Bus TM: distribuir telemetría a suscriptores
//!
//! Implementa un bus de telemetría simple que enruta tramas TM por APID.
//! Múltiples productores envían tramas; un enrutador las despacha a los suscriptores.
//!
//! Ejecutar pruebas:  cargo test --example ex1_tm_bus

#![allow(dead_code, unused_variables)]

use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;

// ─── Tipos pre-escritos ────────────────────────────────────────────────────────

/// Una trama de telemetría simple (estructura inspirada en CCSDS simplificada).
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

// ─── Tu implementación ─────────────────────────────────────────────────────────

/// Un enrutador de bus TM que reenvía tramas a los suscriptores registrados por APID.
pub struct TmBus {
    /// TODO: agregar un HashMap<u16, Sender<TmFrame>> para el enrutamiento
}

impl TmBus {
    /// Crea un nuevo bus TM vacío.
    pub fn new() -> Self {
        todo!("devolver Self con tabla de enrutamiento vacía")
    }

    /// Registra un suscriptor para un APID específico.
    /// Devuelve el extremo [`mpsc::Receiver`]; el bus guarda el sender.
    pub fn subscribe(&mut self, apid: u16) -> mpsc::Receiver<TmFrame> {
        todo!("crear mpsc::channel(16), almacenar el sender en la tabla de enrutamiento, devolver el receiver")
    }

    /// Enruta una trama al suscriptor registrado para su APID.
    /// Si no hay suscriptor registrado, la trama se descarta silenciosamente.
    pub async fn publish(&self, frame: TmFrame) {
        todo!("buscar frame.apid en la tabla de enrutamiento, enviar la trama si se encuentra")
    }
}

// ─── Pruebas ───────────────────────────────────────────────────────────────────

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

        // rx_hk debería recibir tramas de APID 0x003
        let f1 = timeout(Duration::from_millis(50), rx_hk.recv()).await.unwrap().unwrap();
        assert_eq!(f1.apid, 0x003);
        assert_eq!(f1.seq_count, 1);

        let f2 = timeout(Duration::from_millis(50), rx_hk.recv()).await.unwrap().unwrap();
        assert_eq!(f2.seq_count, 3);

        // rx_sensor debería recibir tramas de APID 0x002
        let f3 = timeout(Duration::from_millis(50), rx_sensor.recv()).await.unwrap().unwrap();
        assert_eq!(f3.apid, 0x002);
        assert_eq!(f3.seq_count, 2);
    }

    #[tokio::test]
    async fn unregistered_apid_dropped_silently() {
        let bus = TmBus::new(); // sin suscriptores
        // No debería entrar en pánico ni bloquearse
        bus.publish(TmFrame::new(0x099, 0, 1, 1, vec![])).await;
    }
}

#[tokio::main]
async fn main() {
    println!("Ejecutar pruebas con: cargo test --example ex1_tm_bus");
}
