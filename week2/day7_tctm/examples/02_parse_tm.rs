//! Example 02 — Parse a TM(3,25) Housekeeping Parameter Report
//!
//! TM(3,25) is the workhorse of spacecraft housekeeping telemetry.
//! It carries a snapshot of spacecraft parameters (temperatures, voltages,
//! mode flags) that the ground processes to assess spacecraft health.
//!
//! Run with:  cargo run --example 02_parse_tm

use spacepacket::PusTelemetry;

fn main() {
    // Simulate building a TM packet (as a subsystem would)
    let app_data = build_fake_hk_report();

    let tm = PusTelemetry::new(
        0x300,          // APID: Thermal Control subsystem
        42,             // seq_count: 42nd HK report since boot
        3,              // PUS service 3: Housekeeping
        25,             // subservice 25: HK parameter report
        0xFFFF,         // dest_id: broadcast to all ground stations
        3_600_000,      // OBT: 1 hour into mission (ms since epoch)
        app_data.clone(),
    )
    .expect("valid TM");

    let bytes = tm.to_bytes();
    println!("TM(3,25) Housekeeping Parameter Report");
    println!("  Total size: {} bytes", bytes.len());
    println!();

    // Parse it back (as the ground software would)
    let parsed = PusTelemetry::from_bytes(&bytes).expect("valid TM bytes");

    println!("Parsed fields:");
    println!("  APID:       0x{:03X}", parsed.apid());
    println!("  Seq count:  {}", parsed.seq_count());
    println!("  Service:    {}", parsed.service());
    println!("  Subservice: {}", parsed.subservice());
    println!("  OBT:        {} ms ({:.1} s since epoch)", parsed.obt_ms(), parsed.obt_ms() as f64 / 1000.0);
    println!("  App data:   {} bytes", parsed.app_data().len());

    // Interpret the application data (our fake HK format)
    decode_hk_report(parsed.app_data());
}

/// Encodes a trivial HK report: 3 × u16 parameters, big-endian.
fn build_fake_hk_report() -> Vec<u8> {
    let temperature_mc: i16 = 24_500;    // 24.5 °C in milli-degrees
    let bus_voltage_mv: u16 = 28_200;    // 28.2 V in millivolts
    let mode_flags: u16    = 0x0003;     // nominal mode, HK enabled

    let mut data = Vec::with_capacity(6);
    data.extend_from_slice(&temperature_mc.to_be_bytes());
    data.extend_from_slice(&bus_voltage_mv.to_be_bytes());
    data.extend_from_slice(&mode_flags.to_be_bytes());
    data
}

fn decode_hk_report(data: &[u8]) {
    if data.len() < 6 {
        println!("  [HK data too short to decode]");
        return;
    }
    let temp_mc   = i16::from_be_bytes([data[0], data[1]]);
    let voltage_mv = u16::from_be_bytes([data[2], data[3]]);
    let flags      = u16::from_be_bytes([data[4], data[5]]);

    println!();
    println!("HK Report Contents:");
    println!("  Temperature:    {:.1} °C", temp_mc as f64 / 1000.0);
    println!("  Bus voltage:    {:.3} V", voltage_mv as f64 / 1000.0);
    println!("  Mode flags:     0x{:04X}", flags);
    println!("    Nominal mode: {}", (flags & 0x01) != 0);
    println!("    HK enabled:   {}", (flags & 0x02) != 0);
}
