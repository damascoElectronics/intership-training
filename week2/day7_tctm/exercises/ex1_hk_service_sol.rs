//! Exercise 1 — Solution: PUS Service 3 Housekeeping Report Generator

#![allow(dead_code)]

use spacepacket::{primary_header::CcsdsPrimaryHeader, PusTelemetry};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HkParameterId { TemperatureMc, BusVoltageMv, ModeFlags }

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum HkParameterValue {
    Int16(i16),
    Uint16(u16),
}

impl HkParameterValue {
    pub fn to_bytes(self) -> [u8; 2] {
        match self {
            Self::Int16(v)  => v.to_be_bytes(),
            Self::Uint16(v) => v.to_be_bytes(),
        }
    }
}

pub struct HkService {
    apid: u16,
    seq_count: u16,
    parameters: Vec<(HkParameterId, HkParameterValue)>,
}

impl HkService {
    pub fn new(apid: u16) -> Self {
        Self { apid, seq_count: 0, parameters: Vec::new() }
    }

    pub fn register_parameter(&mut self, id: HkParameterId, value: HkParameterValue) {
        // Update existing entry if found, otherwise append
        if let Some(entry) = self.parameters.iter_mut().find(|(i, _)| *i == id) {
            entry.1 = value;
        } else {
            self.parameters.push((id, value));
        }
    }

    pub fn update_parameter(&mut self, id: HkParameterId, value: HkParameterValue) -> bool {
        if let Some(entry) = self.parameters.iter_mut().find(|(i, _)| *i == id) {
            entry.1 = value;
            true
        } else {
            false
        }
    }

    pub fn build_report(&mut self) -> Result<PusTelemetry, spacepacket::PacketError> {
        // Format: [count: u8] + [id: u8, value: 2B] × count
        let mut app_data = Vec::with_capacity(1 + self.parameters.len() * 3);
        app_data.push(self.parameters.len() as u8);
        for (id, value) in &self.parameters {
            app_data.push(*id as u8);
            app_data.extend_from_slice(&value.to_bytes());
        }

        let obt_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);

        let tm = PusTelemetry::new(self.apid, self.seq_count, 3, 25, 0, obt_ms, app_data)?;
        self.seq_count = CcsdsPrimaryHeader::next_seq(self.seq_count);
        Ok(tm)
    }
}

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
        assert_eq!(tm.app_data().len(), 10);
        assert_eq!(tm.app_data()[0], 3);
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
        svc.update_parameter(HkParameterId::TemperatureMc, HkParameterValue::Int16(25_000));
        let tm = svc.build_report().unwrap();
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
    let mut svc = HkService::new(0x300);
    svc.register_parameter(HkParameterId::TemperatureMc, HkParameterValue::Int16(23_500));
    svc.register_parameter(HkParameterId::BusVoltageMv, HkParameterValue::Uint16(28_100));
    let tm = svc.build_report().unwrap();
    println!("Built TM(3,25): {} bytes, seq={}", tm.to_bytes().len(), tm.seq_count());
}
