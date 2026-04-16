//! Ejemplo 03 — Enrutamiento de APID
//!
//! Una nave espacial realista recibe un flujo de paquetes con diferentes APIDs,
//! cada uno destinado a un subsistema distinto. El enrutador debe despachar
//! rápidamente cada paquete al manejador correcto sin parsear el payload completo.
//!
//! Ejecutar con:  cargo run --example 03_route_packets

use spacepacket::{ApidRouter, PusTelecommand};
use std::sync::mpsc;
use std::thread;

// Tabla de asignación de APID
const APID_OPS:    u16 = 0x001; // Operaciones a bordo (Servicio 17)
const APID_HK:     u16 = 0x002; // Servicio de Housekeeping (Servicio 3)
const APID_POWER:  u16 = 0x003; // Gestión de Energía
const APID_COMMS:  u16 = 0x004; // Comunicaciones

fn main() {
    // Cada subsistema tiene su propio canal de recepción
    let (tx_ops,   rx_ops)   = mpsc::sync_channel::<Vec<u8>>(16);
    let (tx_hk,    rx_hk)    = mpsc::sync_channel::<Vec<u8>>(16);
    let (tx_power, rx_power) = mpsc::sync_channel::<Vec<u8>>(16);
    let (tx_comms, rx_comms) = mpsc::sync_channel::<Vec<u8>>(16);

    // Construir y conectar el enrutador
    let mut router = ApidRouter::new();
    router.register(APID_OPS,   tx_ops);
    router.register(APID_HK,    tx_hk);
    router.register(APID_POWER, tx_power);
    router.register(APID_COMMS, tx_comms);

    // Lanzar hilos suscriptores para imprimir lo que recibe cada subsistema
    spawn_subscriber("OPS   ", rx_ops);
    spawn_subscriber("HK    ", rx_hk);
    spawn_subscriber("POWER ", rx_power);
    spawn_subscriber("COMMS ", rx_comms);

    // Construir un flujo mixto de 12 paquetes y enrutarlos
    println!("Enrutando {} paquetes:", 12);
    let mut seq: u16 = 0;
    let packets: &[(u16, u8, u8, &str)] = &[
        (APID_OPS,   17,  1, "ping"),
        (APID_HK,     3, 129, "solicitar informe HK"),
        (APID_POWER,  6, 128, "volcar memoria"),
        (APID_COMMS,  9,   1, "establecer hora"),
        (APID_OPS,   17,  1, "ping"),
        (APID_HK,     3, 130, "habilitar HK periódico"),
        (APID_POWER,  6,   2, "cargar memoria"),
        (APID_COMMS,  9,   2, "solicitar hora"),
        (APID_HK,     3, 129, "solicitar informe HK"),
        (APID_OPS,   17,  1, "ping"),
        (APID_POWER,  6, 128, "volcar memoria"),
        (APID_COMMS,  9,   1, "establecer hora"),
    ];

    for &(apid, svc, sub, desc) in packets {
        let pkt = PusTelecommand::new(apid, seq, svc, sub, 0, vec![]).unwrap().to_bytes();
        seq += 1;
        println!("  Enviando TC({svc},{sub}) APID=0x{apid:03X} [{desc}]");
        match router.route(pkt) {
            Ok(()) => {}
            Err(e) => eprintln!("  ERROR DE ENRUTAMIENTO: {e}"),
        }
    }

    // Intentar enrutar a un APID sin registrar
    println!();
    println!("Enviando a APID sin registrar 0x099...");
    let unknown = PusTelecommand::new(0x099, 99, 1, 1, 0, vec![]).unwrap().to_bytes();
    match router.route(unknown) {
        Ok(()) => println!("  (éxito inesperado)"),
        Err(e) => println!("  Error esperado: {e}"),
    }

    // Dar tiempo a los hilos para imprimir
    thread::sleep(std::time::Duration::from_millis(50));
    println!("\nEnrutamiento completado.");
}

fn spawn_subscriber(name: &'static str, rx: mpsc::Receiver<Vec<u8>>) {
    thread::spawn(move || {
        while let Ok(bytes) = rx.recv() {
            // Solo parsear el servicio/subservicio de la cabecera secundaria PUS
            if bytes.len() >= 9 {
                let svc = bytes[7];
                let sub = bytes[8];
                println!("  [{name}] TC({svc},{sub}) recibido — {} bytes", bytes.len());
            }
        }
    });
}
