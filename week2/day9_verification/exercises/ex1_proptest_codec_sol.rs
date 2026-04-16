//! Ejercicio 1 — Solución (codec corregido + pruebas que encontraron el error)

use proptest::prelude::*;

// ── Implementación COBS corregida ──────────────────────────────────────────────────

pub fn cobs_encode(data: &[u8]) -> Vec<u8> {
    let mut output = Vec::with_capacity(data.len() + data.len() / 254 + 2);
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
                output.push(0x01); // CORRECCIÓN: esto faltaba en la versión con error
                code = 1;
            }
        }
    }
    output[code_pos] = code;
    output
}

pub fn cobs_decode(encoded: &[u8]) -> Option<Vec<u8>> {
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
        if code < 0xFF && pos < encoded.len() { output.push(0x00); }
    }
    if output.last() == Some(&0x00) { output.pop(); }
    Some(output)
}

proptest! {
    #[test]
    fn roundtrip(data: Vec<u8>) {
        let encoded = cobs_encode(&data);
        let decoded = cobs_decode(&encoded).expect("la decodificación debe tener éxito");
        prop_assert_eq!(decoded, data);
    }

    #[test]
    fn no_zeros_in_output(data: Vec<u8>) {
        let encoded = cobs_encode(&data);
        prop_assert!(!encoded.contains(&0u8));
    }
}

// ── Prueba de regresión: secuencia de exactamente 254 bytes no nulos ──────────────────────────────
#[test]
fn regression_254_byte_run() {
    let data: Vec<u8> = (1u8..=254).collect(); // 254 bytes no nulos
    let encoded = cobs_encode(&data);
    assert!(!encoded.contains(&0u8));
    let decoded = cobs_decode(&encoded).unwrap();
    assert_eq!(decoded, data);
}

fn main() {
    println!("El error: cuando se codifica una secuencia de exactamente 254 bytes no nulos,");
    println!("no se insertaba el marcador de posición del siguiente byte de código.");
    println!("proptest lo encontró generando [1..254] como entrada reducida.");
}
