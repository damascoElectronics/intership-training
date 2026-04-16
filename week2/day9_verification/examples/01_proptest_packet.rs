//! Ejemplo 01 — Pruebas basadas en propiedades para viajes de ida y vuelta de paquetes CCSDS
//!
//! Las pruebas basadas en propiedades generan MUCHAS entradas aleatorias y verifican que
//! tus invariantes se cumplan para TODAS ellas. Compáralas con las pruebas basadas en ejemplos,
//! que solo verifican unos pocos casos seleccionados manualmente.
//!
//! La propiedad fundamental para cualquier codec: parse(serialize(x)) == x
//!
//! Ejecutar pruebas con:  cargo test --example 01_proptest_packet

use proptest::prelude::*;

// ── Cabecera primaria CCSDS mínima en línea (para evitar dependencias entre crates) ────────────

#[derive(Debug, Clone, PartialEq)]
struct Header {
    is_tc: bool,
    apid: u16,       // 11 bits: 0..=0x7FE
    seq_count: u16,  // 14 bits: 0..=0x3FFF
    data_len: u16,   // tal como se almacena: longitud_real - 1
}

impl Header {
    fn serialize(&self) -> [u8; 6] {
        let mut raw = [0u8; 6];
        let word0: u16 = ((self.is_tc as u16) << 12) | (1 << 11) | (self.apid & 0x07FF);
        raw[0] = (word0 >> 8) as u8;
        raw[1] = word0 as u8;
        let word1: u16 = (0b11u16 << 14) | (self.seq_count & 0x3FFF);
        raw[2] = (word1 >> 8) as u8;
        raw[3] = word1 as u8;
        raw[4] = (self.data_len >> 8) as u8;
        raw[5] = self.data_len as u8;
        raw
    }

    fn parse(raw: [u8; 6]) -> Option<Self> {
        let version = (raw[0] >> 5) & 0x07;
        if version != 0 { return None; }
        let word0 = u16::from_be_bytes([raw[0], raw[1]]);
        let apid = word0 & 0x07FF;
        if apid > 0x7FE { return None; }
        let is_tc = (word0 >> 12) & 1 == 1;
        let word1 = u16::from_be_bytes([raw[2], raw[3]]);
        let seq_count = word1 & 0x3FFF;
        let data_len = u16::from_be_bytes([raw[4], raw[5]]);
        Some(Self { is_tc, apid, seq_count, data_len })
    }
}

// ── Propiedades ─────────────────────────────────────────────────────────────────

proptest! {
    /// Para cualquier APID válido, la cabecera serializada puede ser analizada de vuelta de forma idéntica.
    #[test]
    fn roundtrip_apid(apid in 0u16..=0x7FEu16) {
        let h = Header { is_tc: true, apid, seq_count: 0, data_len: 0 };
        let parsed = Header::parse(h.serialize()).expect("debe poder analizarse");
        prop_assert_eq!(parsed.apid, apid);
    }

    /// Para cualquier contador de secuencia válido, el viaje de ida y vuelta lo preserva.
    #[test]
    fn roundtrip_seq_count(seq in 0u16..=0x3FFFu16) {
        let h = Header { is_tc: false, apid: 0x100, seq_count: seq, data_len: 0 };
        let parsed = Header::parse(h.serialize()).unwrap();
        prop_assert_eq!(parsed.seq_count, seq);
    }

    /// El indicador de tipo de paquete sobrevive correctamente al viaje de ida y vuelta.
    #[test]
    fn roundtrip_packet_type(is_tc: bool) {
        let h = Header { is_tc, apid: 0x050, seq_count: 42, data_len: 10 };
        let parsed = Header::parse(h.serialize()).unwrap();
        prop_assert_eq!(parsed.is_tc, is_tc);
    }

    /// parse(serialize(x)) == x para todas las combinaciones válidas.
    #[test]
    fn full_roundtrip(
        is_tc: bool,
        apid in 0u16..=0x7FEu16,
        seq in 0u16..=0x3FFFu16,
        data_len: u16,
    ) {
        let h = Header { is_tc, apid, seq_count: seq, data_len };
        let parsed = Header::parse(h.serialize()).expect("debe poder analizarse");
        prop_assert_eq!(parsed, h);
    }

    /// Analizar 6 bytes arbitrarios NUNCA debe entrar en PÁNICO — puede devolver None.
    #[test]
    fn no_panic_on_arbitrary_bytes(bytes: [u8; 6]) {
        // Header::parse devuelve Option — no debe entrar en pánico
        let _ = Header::parse(bytes);
    }
}

fn main() {
    println!("Ejecutar con: cargo test --example 01_proptest_packet");
    println!();
    println!("proptest generará 256 entradas aleatorias para cada propiedad.");
    println!("Cuando encuentre un fallo, lo reducirá al caso mínimo que falla.");
    println!("Esto es mucho más potente que escribir 10 casos de prueba seleccionados manualmente.");
}
