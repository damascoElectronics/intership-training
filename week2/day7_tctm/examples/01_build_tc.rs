//! Ejemplo 01 — Construir un TC(17,1) ping "¿Estás vivo?"
//!
//! Servicio 17 (Operaciones a bordo) Subservicio 1 (¿Estás vivo?) es el
//! telecomando más simple posible — un ping a la nave espacial. La respuesta
//! esperada es TM(17,2) "Estoy vivo".
//!
//! Ejecutar con:  cargo run --example 01_build_tc

use spacepacket::PusTelecommand;

fn main() {
    // Tabla de asignación de APID (ejemplo):
    //   0x001  →  Servicio de Operaciones a bordo (PUS 17)
    //   0x100  →  Sistema de Control de Actitud
    //   0x200  →  Gestión de Energía
    //   0x300  →  Comunicaciones
    let apid = 0x001u16;
    let seq_count = 1u16;

    let tc = PusTelecommand::new(
        apid,
        seq_count,
        17,   // PUS servicio 17: Operaciones a bordo
        1,    // Subservicio 1: ¿Estás vivo?
        0,    // source_id 0: estación terrestre #1
        vec![], // sin datos de aplicación para un ping simple
    )
    .expect("TC válido");

    let bytes = tc.to_bytes();

    println!("TC(17,1) — ping ¿Estás vivo?");
    println!("  APID:              0x{:03X}", tc.apid());
    println!("  Contador sec.:     {}", tc.seq_count());
    println!("  Servicio:          {}", tc.service());
    println!("  Subservicio:       {}", tc.subservice());
    println!("  Total de bytes:    {}", bytes.len());
    println!();
    println!("Bytes en bruto (hex):");
    print!("  ");
    for (i, byte) in bytes.iter().enumerate() {
        if i > 0 && i % 8 == 0 { print!("\n  "); }
        print!("{:02X} ", byte);
    }
    println!();
    println!();

    // Anotar cada sección:
    println!("Desglose de campos:");
    println!("  Bytes 0-5:   Cabecera primaria CCSDS");
    println!("    [0-1] ID de paquete (versión|tipo|cab_sec|APID):  {:02X}{:02X}", bytes[0], bytes[1]);
    println!("    [2-3] control de secuencia (flags|seq_count):     {:02X}{:02X}", bytes[2], bytes[3]);
    println!("    [4-5] longitud de datos - 1:                      {:02X}{:02X}", bytes[4], bytes[5]);
    println!("  Bytes 6-10:  Cabecera secundaria PUS-C");
    println!("    [6]   versión PUS + spare:  {:02X}", bytes[6]);
    println!("    [7]   tipo de servicio:     {:02X} ({})", bytes[7], bytes[7]);
    println!("    [8]   tipo de subservicio:  {:02X} ({})", bytes[8], bytes[8]);
    println!("    [9-10] ID de origen:        {:02X}{:02X}", bytes[9], bytes[10]);
    println!("  Últimos 2 bytes: CRC-CCITT:   {:02X}{:02X}", bytes[bytes.len()-2], bytes[bytes.len()-1]);

    // Verificar que podemos hacer un viaje de ida y vuelta
    let parsed = PusTelecommand::from_bytes(&bytes).expect("debe parsear correctamente");
    assert_eq!(parsed.service(), 17);
    assert_eq!(parsed.subservice(), 1);
    println!("\nParseo de ida y vuelta: OK");
}
