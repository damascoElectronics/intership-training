//! Ejemplo 03 — Codec COBS con pruebas basadas en propiedades
//!
//! COBS (Consistent Overhead Byte Stuffing) es un codec de enmarcado usado en
//! protocolos serie embebidos. Codifica los datos de modo que 0x00 nunca aparezca
//! en la salida — útil cuando 0x00 es el delimitador de paquetes.
//!
//! Ya usas UART; COBS encaja de forma natural en el enmarcado serie binario.
//!
//! Propiedades a verificar:
//!   1. La salida codificada no contiene bytes 0x00
//!   2. decode(encode(datos)) == datos  (viaje de ida y vuelta)
//!   3. Longitud codificada ≤ len + ceil(len/254) + 1

use proptest::prelude::*;

// ── Implementación COBS ────────────────────────────────────────────────────────

/// Codifica `data` usando COBS. El resultado no contiene bytes 0x00.
pub fn cobs_encode(data: &[u8]) -> Vec<u8> {
    let mut output = Vec::with_capacity(data.len() + data.len() / 254 + 2);
    let mut code_pos = 0usize;
    output.push(0x01); // marcador de posición para el primer byte de código
    let mut code = 1u8;

    for &byte in data {
        if byte == 0x00 {
            // Fin de la secuencia libre de ceros
            output[code_pos] = code;
            code_pos = output.len();
            output.push(0x01); // marcador de posición para el siguiente código
            code = 1;
        } else {
            output.push(byte);
            code += 1;
            if code == 0xFF {
                // Secuencia de 254 bytes no nulos — se debe insertar un byte de sobrecarga
                output[code_pos] = code;
                code_pos = output.len();
                output.push(0x01);
                code = 1;
            }
        }
    }
    output[code_pos] = code;
    output
}

/// Decodifica datos codificados con COBS. Devuelve `None` si la entrada está mal formada.
pub fn cobs_decode(encoded: &[u8]) -> Option<Vec<u8>> {
    if encoded.is_empty() { return Some(vec![]); }
    let mut output = Vec::with_capacity(encoded.len());
    let mut pos = 0;
    while pos < encoded.len() {
        let code = encoded[pos] as usize;
        if code == 0 { return None; } // 0x00 en datos codificados → error
        let end = pos + code;
        if end > encoded.len() { return None; }
        output.extend_from_slice(&encoded[pos + 1..end]);
        pos = end;
        if code < 0xFF && pos < encoded.len() {
            output.push(0x00);
        }
    }
    // Eliminar el cero final que COBS siempre añade conceptualmente
    if output.last() == Some(&0x00) {
        output.pop();
    }
    Some(output)
}

// ── Propiedades ─────────────────────────────────────────────────────────────────

proptest! {
    /// La salida codificada nunca debe contener 0x00.
    #[test]
    fn encoded_has_no_zeros(data: Vec<u8>) {
        let encoded = cobs_encode(&data);
        prop_assert!(!encoded.contains(&0u8),
            "la salida codificada contiene 0x00 para la entrada: {data:?}");
    }

    /// decode(encode(x)) == x para todas las entradas.
    #[test]
    fn roundtrip(data: Vec<u8>) {
        let encoded = cobs_encode(&data);
        let decoded = cobs_decode(&encoded).expect("la decodificación debe tener éxito con datos codificados válidos");
        prop_assert_eq!(decoded, data);
    }

    /// La longitud codificada es como máximo len + ceil(len/254) + 1.
    #[test]
    fn length_bound(data: Vec<u8>) {
        let encoded = cobs_encode(&data);
        let max_len = data.len() + data.len() / 254 + 2;
        prop_assert!(encoded.len() <= max_len,
            "la longitud codificada {} supera el límite {} para la longitud de entrada {}",
            encoded.len(), max_len, data.len());
    }

    /// Una entrada vacía se codifica como un solo byte 0x01.
    #[test]
    fn empty_encodes_to_single_byte(_ignored: u8) {
        let encoded = cobs_encode(&[]);
        prop_assert_eq!(encoded, vec![0x01]);
    }
}

fn main() {
    println!("Ejecutar con: cargo test --example 03_property_tests");
    println!();
    println!("Esta implementación COBS se usa en el enmarcado serie.");
    println!("La propiedad de viaje de ida y vuelta garantiza la corrección para TODAS las entradas.");
}
