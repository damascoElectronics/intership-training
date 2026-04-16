//! Ejemplo 02 — Parsear un TM(3,25) Informe de Parámetros de Telemetría de Casa
//!
//! TM(3,25) es el caballo de batalla de la telemetría de housekeeping de una nave espacial.
//! Transporta una instantánea de los parámetros de la nave (temperaturas, voltajes,
//! indicadores de modo) que el segmento terrestre procesa para evaluar el estado de salud.
//!
//! Ejecutar con:  cargo run --example 02_parse_tm

use spacepacket::PusTelemetry;

fn main() {
    // Simular la construcción de un paquete TM (como lo haría un subsistema)
    let app_data = build_fake_hk_report();

    let tm = PusTelemetry::new(
        0x300,          // APID: subsistema de Control Térmico
        42,             // seq_count: 42.º informe HK desde el arranque
        3,              // PUS servicio 3: Housekeeping
        25,             // subservicio 25: informe de parámetros HK
        0xFFFF,         // dest_id: difusión a todas las estaciones terrestres
        3_600_000,      // OBT: 1 hora de misión (ms desde la época)
        app_data.clone(),
    )
    .expect("TM válido");

    let bytes = tm.to_bytes();
    println!("TM(3,25) Informe de Parámetros de Housekeeping");
    println!("  Tamaño total: {} bytes", bytes.len());
    println!();

    // Parsearlo de vuelta (como lo haría el software terrestre)
    let parsed = PusTelemetry::from_bytes(&bytes).expect("bytes TM válidos");

    println!("Campos parseados:");
    println!("  APID:        0x{:03X}", parsed.apid());
    println!("  Cont. sec.:  {}", parsed.seq_count());
    println!("  Servicio:    {}", parsed.service());
    println!("  Subservicio: {}", parsed.subservice());
    println!("  OBT:         {} ms ({:.1} s desde la época)", parsed.obt_ms(), parsed.obt_ms() as f64 / 1000.0);
    println!("  Datos aplic: {} bytes", parsed.app_data().len());

    // Interpretar los datos de aplicación (nuestro formato HK de prueba)
    decode_hk_report(parsed.app_data());
}

/// Codifica un informe HK mínimo: 3 parámetros × u16, big-endian.
fn build_fake_hk_report() -> Vec<u8> {
    let temperature_mc: i16 = 24_500;    // 24,5 °C en miligrados
    let bus_voltage_mv: u16 = 28_200;    // 28,2 V en milivoltios
    let mode_flags: u16    = 0x0003;     // modo nominal, HK habilitado

    let mut data = Vec::with_capacity(6);
    data.extend_from_slice(&temperature_mc.to_be_bytes());
    data.extend_from_slice(&bus_voltage_mv.to_be_bytes());
    data.extend_from_slice(&mode_flags.to_be_bytes());
    data
}

fn decode_hk_report(data: &[u8]) {
    if data.len() < 6 {
        println!("  [Datos HK demasiado cortos para decodificar]");
        return;
    }
    let temp_mc   = i16::from_be_bytes([data[0], data[1]]);
    let voltage_mv = u16::from_be_bytes([data[2], data[3]]);
    let flags      = u16::from_be_bytes([data[4], data[5]]);

    println!();
    println!("Contenido del informe HK:");
    println!("  Temperatura:     {:.1} °C", temp_mc as f64 / 1000.0);
    println!("  Voltaje de bus:  {:.3} V", voltage_mv as f64 / 1000.0);
    println!("  Indicadores:     0x{:04X}", flags);
    println!("    Modo nominal:  {}", (flags & 0x01) != 0);
    println!("    HK habilitado: {}", (flags & 0x02) != 0);
}
