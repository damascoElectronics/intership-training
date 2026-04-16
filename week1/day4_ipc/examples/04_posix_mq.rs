//! Ejemplo 04 — Colas de Mensajes POSIX
//!
//! Las MQ POSIX son únicas entre los mecanismos IPC porque admiten PRIORIDAD.
//! Los mensajes de mayor prioridad siempre se reciben antes que los de menor prioridad,
//! independientemente del orden de llegada.
//!
//! Esto es muy relevante para el software de naves espaciales:
//!  - Un ping TC(17,1) debería procesarse antes que una solicitud de HK de baja prioridad
//!  - Un evento de fallo debería adelantarse a un volcado rutinario de telemetría
//!
//! Ejecutar con:  cargo run --example 04_posix_mq

use nix::mqueue::{mq_close, mq_open, mq_receive, mq_send, mq_unlink, MqAttr, OFlag};
use nix::sys::stat::Mode;
use std::ffi::CString;

const MQ_NAME: &str = "/day4_mq_demo";

// Niveles de prioridad (mayor = más urgente, recibido primero)
const PRIO_LOW:    u32 = 0;
const PRIO_MEDIUM: u32 = 5;
const PRIO_HIGH:   u32 = 10;

fn main() {
    println!("=== Demostración de prioridad en Cola de Mensajes POSIX ===\n");

    let mq_name = CString::new(MQ_NAME).unwrap();

    // Limpiar cualquier cola sobrante de la ejecución anterior
    let _ = mq_unlink(&mq_name);

    // Crear la cola: máximo 10 mensajes, cada uno de hasta 256 bytes
    let attrs = MqAttr::new(0, 10, 256, 0);
    let mq = mq_open(
        &mq_name,
        OFlag::O_CREAT | OFlag::O_RDWR,
        Mode::S_IRUSR | Mode::S_IWUSR,
        Some(&attrs),
    )
    .expect("crear mq");

    println!("Cola POSIX MQ '{MQ_NAME}' creada (máx 10 msgs, 256 bytes cada uno)");

    // Enviar mensajes en orden BAJA → MEDIA → ALTA
    // Se recibirán en orden ALTA → MEDIA → BAJA (cola de prioridad)
    let messages = [
        (PRIO_LOW,    "TC(3,129) Solicitar informe HK [baja prioridad]"),
        (PRIO_MEDIUM, "TC(5,1)   Registrar evento de estado [prioridad media]"),
        (PRIO_HIGH,   "TC(17,1)  Ping Are-You-Alive [ALTA prioridad]"),
        (PRIO_LOW,    "TC(3,130) Habilitar HK periódico [baja prioridad]"),
        (PRIO_HIGH,   "TC(9,1)   Sincronizar tiempo [ALTA prioridad]"),
    ];

    println!("\nEnviando mensajes en este orden:");
    for (prio, msg) in &messages {
        println!("  [prio={prio:2}] {msg}");
        mq_send(mq, msg.as_bytes(), *prio).expect("mq_send");
    }

    println!("\nRecibiendo (orden de prioridad — NO orden de inserción):");
    let mut buf = vec![0u8; 256];
    for _ in 0..messages.len() {
        let (len, prio) = mq_receive(mq, &mut buf, None).expect("mq_receive");
        let msg = std::str::from_utf8(&buf[..len]).unwrap();
        println!("  [prio={prio:2}] {msg}");
    }

    mq_close(mq).ok();
    mq_unlink(&mq_name).ok();

    println!();
    println!("Conclusión clave: los mensajes llegaron en orden de prioridad independientemente del orden de envío.");
    println!("Así es como se implementan los carriles de prioridad TC en un enrutador de nave espacial.");
}
