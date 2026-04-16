//! Ejemplo 02 — HMAC-SHA256 para autenticación de TC
//!
//! HMAC autentica que un TC proviene de alguien que posee la clave Y que
//! los bytes no fueron modificados en tránsito. NO cifra.
//!
//! Crítico: SIEMPRE usar comparación en tiempo constante (crate subtle) — de lo contrario
//! un ataque de temporización puede recuperar el MAC esperado byte a byte.
//!
//! Ejecutar con:  cargo run --example 02_hmac_tc

use hmac::{Hmac, Mac};
use sha2::Sha256;
use subtle::ConstantTimeEq;

type HmacSha256 = Hmac<Sha256>;

/// Firma `packet_bytes` con HMAC-SHA256, devolviendo el MAC de 32 bytes.
pub fn sign_tc(packet_bytes: &[u8], key: &[u8]) -> [u8; 32] {
    let mut mac = HmacSha256::new_from_slice(key)
        .expect("HMAC acepta claves de cualquier longitud");
    mac.update(packet_bytes);
    mac.finalize().into_bytes().into()
}

/// Verifica el HMAC de un TC. Usa comparación en tiempo constante.
///
/// # Ataques de temporización
/// Un `if computed == expected` ingenuo filtra información de temporización: devuelve
/// antes en el primer byte que no coincide. Un atacante puede enviar millones de
/// paquetes con MACs diferentes y medir qué posiciones de byte causan retornos tempranos,
/// reconstruyendo eventualmente el MAC esperado.
///
/// `ConstantTimeEq` siempre compara los 32 bytes independientemente del contenido.
pub fn verify_tc(packet_bytes: &[u8], claimed_mac: &[u8; 32], key: &[u8]) -> bool {
    let expected = sign_tc(packet_bytes, key);
    // MAL: expected == *claimed_mac  ← ¡ataque de temporización!
    // BIEN: comparación en tiempo constante:
    expected.ct_eq(claimed_mac).into()
}

/// Estructura de paquete TC para esta demo.
struct TcPacket {
    apid: u16,
    service: u8,
    subservice: u8,
    data: Vec<u8>,
}

impl TcPacket {
    fn serialize(&self) -> Vec<u8> {
        let mut buf = Vec::new();
        buf.extend_from_slice(&self.apid.to_be_bytes());
        buf.push(self.service);
        buf.push(self.subservice);
        buf.extend_from_slice(&self.data);
        buf
    }
}

fn main() {
    let key = b"spacecraft-shared-secret-key-256b";
    let wrong_key = b"wrong-key-someone-else-signed-it";

    let tc = TcPacket {
        apid: 0x001,
        service: 17,
        subservice: 1,
        data: vec![0xDE, 0xAD, 0xBE, 0xEF],
    };
    let packet_bytes = tc.serialize();

    println!("=== Autenticación TC con HMAC-SHA256 ===\n");
    println!("Paquete: {:02X?}", packet_bytes);

    // Firmar con la clave correcta
    let mac = sign_tc(&packet_bytes, key);
    println!("\nMAC (32 bytes): {:02X?}", &mac[..8]);
    println!("              (mostrando los primeros 8 de 32 bytes)");

    // Escenarios de verificación
    println!("\nEscenarios de verificación:");
    println!("  clave correcta:   {}", if verify_tc(&packet_bytes, &mac, key) { "✓ VÁLIDO" } else { "✗ INVÁLIDO" });
    println!("  clave incorrecta: {}", if verify_tc(&packet_bytes, &mac, wrong_key) { "✓ VÁLIDO" } else { "✗ INVÁLIDO" });

    // Paquete modificado (inversión de bit)
    let mut tampered = packet_bytes.clone();
    tampered[2] ^= 0x01;
    println!("  paquete alterado: {}", if verify_tc(&tampered, &mac, key) { "✓ VÁLIDO" } else { "✗ INVÁLIDO" });

    println!("\nConcepto clave: el MAC cubre el PAYLOAD COMPLETO.");
    println!("Invertir un solo bit en el paquete hace fallar la verificación.");
    println!("Esto protege tanto contra la alteración como contra la inyección de nuevos comandos.");
}
