//! Ejercicio 1 — Encontrar el error con proptest
//!
//! La implementación COBS a continuación tiene un error sutil de desfase en uno.
//! Escribe propiedades proptest que lo encuentren.
//!
//! Pista: el error solo se manifiesta para longitudes de entrada que son múltiplos exactos de 254.
//!
//! Ejecutar: cargo test --example ex1_proptest_codec

use proptest::prelude::*;

// ── Implementación COBS con error ──────────────────────────────────────────────────
// NO CORRIJAS esta implementación — el objetivo es ENCONTRAR el error con proptest.

pub fn cobs_encode_buggy(data: &[u8]) -> Vec<u8> {
    let mut output = Vec::with_capacity(data.len() + 2);
    let mut code_pos = 0usize;
    output.push(0x01);
    let mut code = 1u8;

    for &byte in data {
        if byte == 0x00 {
            output[code_pos] = code;
            code_pos = output.len();
            output.push(0x01);
            code = 1;
        } else {
            output.push(byte);
            code += 1;
            if code == 0xFF {
                output[code_pos] = code;
                code_pos = output.len();
                // ERROR: aquí debería hacerse push de 0x01 (marcador del siguiente código) pero no se hace
                // Esto corrompe la salida para entradas donde una secuencia de 254 bytes termina exactamente
                code = 1;
            }
        }
    }
    output[code_pos] = code;
    output
}

pub fn cobs_decode_buggy(encoded: &[u8]) -> Option<Vec<u8>> {
    if encoded.is_empty() { return Some(vec![]); }
    let mut output = Vec::with_capacity(encoded.len());
    let mut pos = 0;
    while pos < encoded.len() {
        let code = encoded[pos] as usize;
        if code == 0 { return None; }
        let end = pos + code;
        if end > encoded.len() { return None; }
        output.extend_from_slice(&encoded[pos + 1..end]);
        pos = end;
        if code < 0xFF && pos < encoded.len() {
            output.push(0x00);
        }
    }
    if output.last() == Some(&0x00) { output.pop(); }
    Some(output)
}

// ── Tus pruebas ────────────────────────────────────────────────────────────────

proptest! {
    /// TODO: Escribe una propiedad que encuentre el error.
    ///
    /// La propiedad de viaje de ida y vuelta fallará para alguna entrada.
    /// Cuando proptest reduzca el caso que falla, deberías ver
    /// qué tipo de entrada provoca el error.
    #[test]
    fn roundtrip_should_find_bug(data: Vec<u8>) {
        todo!("escribir: codificar, decodificar, afirmar igualdad — proptest encontrará el error")
    }

    /// TODO: Escribe una propiedad que verifique que no hay 0x00 en la salida.
    /// ¿El error también afecta a esta propiedad?
    #[test]
    fn no_zeros_in_output(data: Vec<u8>) {
        todo!("codificar y afirmar que no hay bytes 0x00 en el resultado")
    }
}

fn main() {
    println!("Ejecutar: cargo test --example ex1_proptest_codec");
    println!("Cuando proptest encuentre el caso que falla, lo reducirá.");
    println!("Esperado: falla para entradas con una secuencia de 254+ bytes no nulos.");
}
