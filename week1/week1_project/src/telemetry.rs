//! Almacén de telemetría con buffer circular.

use std::collections::VecDeque;
use crate::sensor::SensorFrame;

pub struct TelemetryBuffer {
    readings: VecDeque<SensorFrame>,
    capacity: usize,
}

pub struct TelemetryStats {
    pub min_temp_mc: i32,
    pub max_temp_mc: i32,
    pub mean_temp_mc: i32,
    pub sample_count: usize,
}

impl TelemetryBuffer {
    pub fn new(capacity: usize) -> Self {
        Self { readings: VecDeque::with_capacity(capacity), capacity }
    }

    pub fn push(&mut self, frame: SensorFrame) {
        if self.readings.len() >= self.capacity {
            self.readings.pop_front();
        }
        self.readings.push_back(frame);
    }

    pub fn stats(&self) -> Option<TelemetryStats> {
        if self.readings.is_empty() { return None; }
        let temps: Vec<i32> = self.readings.iter().map(|f| f.temperature_mc).collect();
        let min = *temps.iter().min().unwrap();
        let max = *temps.iter().max().unwrap();
        let mean = (temps.iter().map(|&t| t as i64).sum::<i64>() / temps.len() as i64) as i32;
        Some(TelemetryStats {
            min_temp_mc: min,
            max_temp_mc: max,
            mean_temp_mc: mean,
            sample_count: temps.len(),
        })
    }
}
