//! Ejercicio 1 — Implementar un Generador de Informes de Housekeeping PUS Servicio 3
//!
//! PUS Servicio 3 es uno de los servicios más importantes en un OBC. Permite que
//! el segmento terrestre solicite instantáneas de los parámetros de la nave
//! (temperaturas, voltajes, modos) empaquetados en paquetes TM(3,25).
//!
//! ## Tu tarea
//!
//! Rellena cada bloque `todo!()` que aparece a continuación. Cuando todas las pruebas
//! pasen, habrás terminado.
//!
//! Ejecutar pruebas con:  cargo test --example ex1_hk_service

#![allow(dead_code, unused_variables)]

use spacepacket::PusTelemetry;

// ─── Tipos de apoyo ya escritos ───────────────────────────────────────────────

/// Identifica un único parámetro de la nave espacial.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HkParameterId {
    TemperatureMc,   // miligrados Celsius (i16)
    BusVoltageMv,    // milivoltios (u16)
    ModeFlags,       // campo de bits (u16)
}

/// Un valor de parámetro muestreado.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum HkParameterValue {
    Int16(i16),
    Uint16(u16),
}

impl HkParameterValue {
    /// Serializa el valor a bytes big-endian.
    pub fn to_bytes(self) -> [u8; 2] {
        match self {
            Self::Int16(v)  => v.to_be_bytes(),
            Self::Uint16(v) => v.to_be_bytes(),
        }
    }
}

// ─── Tu implementación ────────────────────────────────────────────────────────

/// Genera informes de housekeeping PUS Servicio 3.
pub struct HkService {
    apid: u16,
    seq_count: u16,
    /// Parámetros registrados: (id, valor_actual)
    parameters: Vec<(HkParameterId, HkParameterValue)>,
}

impl HkService {
    /// Crea un nuevo `HkService` para el APID dado.
    pub fn new(apid: u16) -> Self {
        Self { apid, seq_count: 0, parameters: Vec::new() }
    }

    /// Registra un parámetro con su valor inicial.
    ///
    /// Si el parámetro ya está registrado, se actualiza su valor.
    pub fn register_parameter(&mut self, id: HkParameterId, value: HkParameterValue) {
        todo!("buscar entrada existente y actualizar, o insertar un nuevo par (id, value)")
    }

    /// Actualiza el valor almacenado para un parámetro registrado.
    ///
    /// Devuelve `false` si el parámetro no estaba registrado.
    pub fn update_parameter(&mut self, id: HkParameterId, value: HkParameterValue) -> bool {
        todo!("buscar la entrada por id, actualizar su valor, devolver true; devolver false si no se encuentra")
    }

    /// Construye un informe TM(3,25) HK que contiene todos los parámetros registrados.
    ///
    /// El formato de app_data es:
    ///   [parameter_count: u8] [id: u8, value: 2B] × parameter_count
    ///
    /// El contador de secuencia se incrementa tras cada llamada.
    pub fn build_report(&mut self) -> Result<PusTelemetry, spacepacket::PacketError> {
        todo!("
            1. Construir app_data:
               - primer byte: número de parámetros (como u8)
               - luego por cada (id, value): insertar id como u8, insertar value.to_bytes()
            2. Llamar PusTelemetry::new(self.apid, self.seq_count, 3, 25, 0, obt_ms, app_data)
            3. Incrementar self.seq_count (usando CcsdsPrimaryHeader::next_seq para hacer wrap en 14 bits)
            4. Devolver el TM
        ")
    }
}

// ─── Pruebas (deben pasar) ────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn register_and_build() {
        let mut svc = HkService::new(0x300);
        svc.register_parameter(HkParameterId::TemperatureMc, HkParameterValue::Int16(22_000));
        svc.register_parameter(HkParameterId::BusVoltageMv, HkParameterValue::Uint16(28_000));
        svc.register_parameter(HkParameterId::ModeFlags, HkParameterValue::Uint16(0x0003));

        let tm = svc.build_report().unwrap();
        assert_eq!(tm.service(), 3);
        assert_eq!(tm.subservice(), 25);
        assert_eq!(tm.apid(), 0x300);
        // app_data: 1 byte de conteo + 3 × 3 bytes = 10 bytes
        assert_eq!(tm.app_data().len(), 10, "app_data debe tener 10 bytes");
        assert_eq!(tm.app_data()[0], 3, "el primer byte debe ser el conteo de parámetros");
    }

    #[test]
    fn seq_count_increments() {
        let mut svc = HkService::new(0x300);
        svc.register_parameter(HkParameterId::ModeFlags, HkParameterValue::Uint16(0));
        let tm1 = svc.build_report().unwrap();
        let tm2 = svc.build_report().unwrap();
        assert_eq!(tm1.seq_count() + 1, tm2.seq_count());
    }

    #[test]
    fn update_changes_value() {
        let mut svc = HkService::new(0x300);
        svc.register_parameter(HkParameterId::TemperatureMc, HkParameterValue::Int16(0));
        let updated = svc.update_parameter(HkParameterId::TemperatureMc, HkParameterValue::Int16(25_000));
        assert!(updated);
        let tm = svc.build_report().unwrap();
        // La temperatura es el primer (y único) parámetro; id en offset 1, valor en 2-3
        let val = i16::from_be_bytes([tm.app_data()[2], tm.app_data()[3]]);
        assert_eq!(val, 25_000);
    }

    #[test]
    fn update_nonexistent_returns_false() {
        let mut svc = HkService::new(0x300);
        assert!(!svc.update_parameter(HkParameterId::ModeFlags, HkParameterValue::Uint16(1)));
    }
}

fn main() {
    println!("Ejecutar pruebas con: cargo test --example ex1_hk_service");
}
