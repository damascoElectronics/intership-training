//! Ejemplo 04 — Gestión del contador de secuencia
//!
//! Cada paquete CCSDS lleva un contador de secuencia de 14 bits (0–0x3FFF por APID).
//! El segmento terrestre los verifica para detectar paquetes perdidos, duplicados o
//! ataques de repetición.
//!
//! Reglas clave:
//!   - Cada APID tiene su PROPIO contador independiente
//!   - El contador se incrementa con cada paquete
//!   - Hace wrap en 0x3FFF → 0x0000 (no en 0xFFFF)
//!   - Un salto indica que se perdieron paquetes
//!
//! Ejecutar con:  cargo run --example 04_sequence_counter

use spacepacket::primary_header::CcsdsPrimaryHeader;
use std::collections::HashMap;

/// Rastreador del contador de secuencia por APID (lado nave — para transmitir).
struct SeqCounters {
    counters: HashMap<u16, u16>,
}

impl SeqCounters {
    fn new() -> Self { Self { counters: HashMap::new() } }

    /// Devuelve el PRÓXIMO contador de secuencia para este APID y luego lo incrementa.
    fn next(&mut self, apid: u16) -> u16 {
        let counter = self.counters.entry(apid).or_insert(0);
        let value = *counter;
        *counter = CcsdsPrimaryHeader::next_seq(value);
        value
    }
}

/// Detector de saltos en el segmento terrestre.
struct GapDetector {
    expected: HashMap<u16, u16>,
    gaps_found: u32,
}

impl GapDetector {
    fn new() -> Self { Self { expected: HashMap::new(), gaps_found: 0 } }

    fn check(&mut self, apid: u16, seq: u16) {
        let expected = self.expected.entry(apid).or_insert(seq);
        if seq != *expected {
            // Calcular cuántos paquetes se perdieron, teniendo en cuenta el wrap-around
            let lost = if seq > *expected {
                seq - *expected
            } else {
                (0x4000 - *expected) + seq  // caso de wrap-around
            };
            println!(
                "  [SALTO DETECTADO] APID 0x{apid:03X}: se esperaba seq={expected}, se recibió seq={seq} — {} paquete(s) perdido(s)",
                lost
            );
            self.gaps_found += 1;
        }
        *expected = CcsdsPrimaryHeader::next_seq(seq);
    }
}

fn main() {
    println!("=== Demo de Contador de Secuencia ===\n");

    let mut counters = SeqCounters::new();
    let mut detector = GapDetector::new();

    // Simular transmisión normal para APID 0x100
    println!("--- Flujo de paquetes normal (APID 0x100) ---");
    for _ in 0..5 {
        let seq = counters.next(0x100);
        println!("  Enviando APID=0x100 seq={seq}");
        detector.check(0x100, seq);
    }

    // Simular un salto (los paquetes 5 y 6 se pierden, solo llega el 7)
    println!("\n--- Simulando 2 paquetes perdidos ---");
    let seq = counters.next(0x100);
    println!("  Paquete seq={seq} 'perdido en transmisión'");
    counters.next(0x100); // también perdido
    let seq_after_gap = counters.next(0x100);
    println!("  Enviando APID=0x100 seq={seq_after_gap} (el segmento terrestre detecta el salto)");
    detector.check(0x100, seq_after_gap);

    // Contadores independientes por APID
    println!("\n--- Contadores independientes (APID 0x200 empieza desde cero) ---");
    for i in 0..3 {
        let seq_100 = counters.next(0x100);
        let seq_200 = counters.next(0x200);
        println!("  0x100 seq={seq_100}, 0x200 seq={seq_200} (iteración {i})");
        detector.check(0x100, seq_100);
        detector.check(0x200, seq_200);
    }

    // Demostrar el wrap-around de 14 bits
    println!("\n--- Wrap-around de 14 bits en 0x3FFF ---");
    let mut wrap_counter: u16 = 0x3FFD;
    for _ in 0..5 {
        println!("  seq = 0x{wrap_counter:04X} ({wrap_counter})");
        wrap_counter = CcsdsPrimaryHeader::next_seq(wrap_counter);
    }

    println!("\n--- Resumen ---");
    println!("Saltos detectados: {}", detector.gaps_found);
}
