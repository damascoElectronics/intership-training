//! Ejercicio 1 — Implementar un receptor TC seguro
//!
//! Conectar la verificación HMAC + la protección de repetición en una función
//! completa de receptor TC.
//!
//! Ejecutar las pruebas:  cargo test --example ex1_secure_tc_receiver

#![allow(dead_code, unused_variables)]

use hmac::{Hmac, Mac};
use sha2::Sha256;
use subtle::ConstantTimeEq;

type HmacSha256 = Hmac<Sha256>;

// ─── Tipos preaparados ────────────────────────────────────────────────────────

pub const TEST_KEY: &[u8] = b"test-key-for-training-only";

/// Un paquete TC crudo con un HMAC de 32 bytes adjunto.
#[derive(Debug, Clone)]
pub struct RawTcWithHmac {
    /// Los bytes del paquete (todo excepto el HMAC).
    pub payload: Vec<u8>,
    /// HMAC-SHA256 de 32 bytes sobre `payload`.
    pub mac: [u8; 32],
}

impl RawTcWithHmac {
    /// Crea un paquete firmado válido.
    pub fn new_signed(apid: u16, seq: u16, service: u8, subservice: u8, key: &[u8]) -> Self {
        let mut payload = Vec::new();
        payload.extend_from_slice(&apid.to_be_bytes());
        payload.extend_from_slice(&seq.to_be_bytes());
        payload.push(service);
        payload.push(subservice);
        let mut hmac = HmacSha256::new_from_slice(key).unwrap();
        hmac.update(&payload);
        let mac: [u8; 32] = hmac.finalize().into_bytes().into();
        Self { payload, mac }
    }

    pub fn seq(&self) -> u16 {
        u16::from_be_bytes([self.payload[2], self.payload[3]])
    }
}

/// Un TC verificado y autenticado (salida del receptor).
#[derive(Debug, Clone)]
pub struct VerifiedTc {
    pub apid: u16,
    pub seq: u16,
    pub service: u8,
    pub subservice: u8,
}

#[derive(Debug, PartialEq)]
pub enum RejectionReason {
    BadHmac,
    Replay,
    TooOld,
}

// ─── Tu implementación ───────────────────────────────────────────────────────

/// Procesa un flujo de paquetes TC crudos y devuelve solo los que pasan
/// la verificación HMAC y la protección de repetición.
///
/// Devuelve una lista de (resultado: Ok/Err) en orden, uno por paquete de entrada.
pub fn process_tc_stream(
    packets: &[RawTcWithHmac],
    key: &[u8],
) -> Vec<Result<VerifiedTc, RejectionReason>> {
    todo!(
        "Para cada paquete:
         1. Verificar HMAC (usar HmacSha256::new_from_slice + update + verificar en tiempo constante)
         2. Comprobar ventana de repetición (implementar un conjunto visto simple o ventana deslizante)
         3. Si ambos pasan: devolver Ok(VerifiedTc { ... })
         4. De lo contrario: devolver Err(RejectionReason::BadHmac | Replay | TooOld)"
    )
}

// ─── Pruebas ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn make_stream() -> Vec<RawTcWithHmac> {
        vec![
            RawTcWithHmac::new_signed(0x001, 0, 17, 1, TEST_KEY),  // válido
            RawTcWithHmac::new_signed(0x001, 1, 17, 1, TEST_KEY),  // válido
            // HMAC incorrecto: corromper el MAC
            { let mut p = RawTcWithHmac::new_signed(0x001, 2, 3, 129, TEST_KEY); p.mac[0] ^= 0xFF; p },
            RawTcWithHmac::new_signed(0x001, 3, 3, 129, TEST_KEY), // válido
            // repetición: mismo seq que el segundo paquete
            RawTcWithHmac::new_signed(0x001, 1, 17, 1, TEST_KEY),  // repetición
            RawTcWithHmac::new_signed(0x001, 4, 17, 1, TEST_KEY),  // válido
            RawTcWithHmac::new_signed(0x001, 5, 17, 1, TEST_KEY),  // válido
        ]
    }

    #[test]
    fn correct_accept_reject_counts() {
        let stream = make_stream();
        let results = process_tc_stream(&stream, TEST_KEY);
        assert_eq!(results.len(), 7);

        let accepted: Vec<_> = results.iter().filter(|r| r.is_ok()).collect();
        let rejected: Vec<_> = results.iter().filter(|r| r.is_err()).collect();

        assert_eq!(accepted.len(), 5, "se deben aceptar 5 TCs válidos");
        assert_eq!(rejected.len(), 2, "se deben rechazar 2 (HMAC incorrecto + repetición)");
    }

    #[test]
    fn bad_hmac_rejected() {
        let mut pkt = RawTcWithHmac::new_signed(0x001, 0, 17, 1, TEST_KEY);
        pkt.mac = [0u8; 32]; // MAC incorrecto
        let results = process_tc_stream(&[pkt], TEST_KEY);
        assert_eq!(results[0], Err(RejectionReason::BadHmac));
    }

    #[test]
    fn valid_packet_accepted() {
        let pkt = RawTcWithHmac::new_signed(0x001, 0, 17, 1, TEST_KEY);
        let results = process_tc_stream(&[pkt], TEST_KEY);
        assert!(results[0].is_ok());
        let tc = results[0].as_ref().unwrap();
        assert_eq!(tc.service, 17);
        assert_eq!(tc.subservice, 1);
    }
}

fn main() {
    println!("Ejecutar las pruebas con: cargo test --example ex1_secure_tc_receiver");
}
