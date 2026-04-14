//! Exercise 1 — Implement a PUS Service 3 Housekeeping Report Generator
//!
//! PUS Service 3 is one of the most important services on an OBC.  It allows
//! the ground to request snapshots of spacecraft parameters (temperatures,
//! voltages, modes) packed into TM(3,25) packets.
//!
//! ## Your task
//!
//! Fill in every `todo!()` block below.  When all tests pass, you're done.
//!
//! Run tests with:  cargo test --example ex1_hk_service

#![allow(dead_code, unused_variables)]

use spacepacket::PusTelemetry;

// ─── Pre-written supporting types ────────────────────────────────────────────

/// Identifies a single spacecraft parameter.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HkParameterId {
    TemperatureMc,   // millidegrees Celsius (i16)
    BusVoltageMv,    // millivolts (u16)
    ModeFlags,       // bit field (u16)
}

/// A sampled parameter value.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum HkParameterValue {
    Int16(i16),
    Uint16(u16),
}

impl HkParameterValue {
    /// Serialises the value to big-endian bytes.
    pub fn to_bytes(self) -> [u8; 2] {
        match self {
            Self::Int16(v)  => v.to_be_bytes(),
            Self::Uint16(v) => v.to_be_bytes(),
        }
    }
}

// ─── Your implementation ─────────────────────────────────────────────────────

/// Generates PUS Service 3 housekeeping reports.
pub struct HkService {
    apid: u16,
    seq_count: u16,
    /// Registered parameters: (id, current_value)
    parameters: Vec<(HkParameterId, HkParameterValue)>,
}

impl HkService {
    /// Creates a new `HkService` for the given APID.
    pub fn new(apid: u16) -> Self {
        Self { apid, seq_count: 0, parameters: Vec::new() }
    }

    /// Registers a parameter with its initial value.
    ///
    /// If the parameter is already registered, its value is updated.
    pub fn register_parameter(&mut self, id: HkParameterId, value: HkParameterValue) {
        todo!("find existing entry and update, or push a new (id, value) pair")
    }

    /// Updates the stored value for a registered parameter.
    ///
    /// Returns `false` if the parameter was not registered.
    pub fn update_parameter(&mut self, id: HkParameterId, value: HkParameterValue) -> bool {
        todo!("find the entry by id, update its value, return true; return false if not found")
    }

    /// Builds a TM(3,25) HK Parameter Report containing all registered parameters.
    ///
    /// The app_data format is:
    ///   [parameter_count: u8] [id: u8, value: 2B] × parameter_count
    ///
    /// The sequence count is incremented after each call.
    pub fn build_report(&mut self) -> Result<PusTelemetry, spacepacket::PacketError> {
        todo!("
            1. Build app_data:
               - first byte: number of parameters (as u8)
               - then for each (id, value): push id as u8, push value.to_bytes()
            2. Call PusTelemetry::new(self.apid, self.seq_count, 3, 25, 0, obt_ms, app_data)
            3. Increment self.seq_count (using CcsdsPrimaryHeader::next_seq to wrap at 14 bits)
            4. Return the TM
        ")
    }
}

// ─── Tests (these must pass) ─────────────────────────────────────────────────

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
        // app_data: 1 byte count + 3 × 3 bytes = 10 bytes
        assert_eq!(tm.app_data().len(), 10, "app_data should be 10 bytes");
        assert_eq!(tm.app_data()[0], 3, "first byte should be parameter count");
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
        // Temperature is the first (and only) parameter; id at offset 1, value at 2-3
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
    println!("Run tests with: cargo test --example ex1_hk_service");
}
