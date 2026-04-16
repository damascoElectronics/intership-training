//! Ejemplo 05 — Arquitectura de separación de privilegios
//!
//! La frontera de seguridad está entre tc_receiver (lado no confiable, habla con
//! la interfaz de red/RF) y obc_router (lado confiable, habla con el hardware).
//!
//! tc_receiver NO tiene acceso a los actuadores. Aunque sea comprometido, solo puede
//! enviar bytes al router — que luego aplica su propia validación.
//!
//! Este ejemplo muestra el diseño como tareas en el mismo proceso que se comunican mediante un
//! canal (en el week2_project completo son procesos separados mediante sockets Unix).
//!
//! Ejecutar con:  cargo run --example 05_privilege_separation

use tokio::sync::mpsc;

// ── Tipos ─────────────────────────────────────────────────────────────────────

#[derive(Debug)]
struct VerifiedTc {
    apid: u16,
    service: u8,
    subservice: u8,
    data: Vec<u8>,
}

// ── Lado no confiable: receptor TC ───────────────────────────────────────────

/// Recibe bytes crudos de la "red", los autentica y reenvía solo
/// los TCs válidos al router confiable.
///
/// Este componente NO tiene conocimiento de actuadores ni subsistemas.
/// Su única capacidad: enviar bytes a `tx`.
async fn tc_receiver(tx: mpsc::Sender<VerifiedTc>) {
    // Simular la recepción de 5 "bytes TC" del enlace ascendente
    let simulated_uplink: Vec<(bool, u16, u8, u8)> = vec![
        (true,  0x001, 17, 1),  // TC(17,1) ping válido
        (false, 0x001, 17, 1),  // HMAC incorrecto — rechazado
        (true,  0x003,  3, 129),// TC(3,129) petición HK válida
        (true,  0x003,  3, 129),// repetición del anterior — rechazado por ventana de repetición
        (true,  0x001, 17, 1),  // ping válido
    ];

    let mut seq: u16 = 0;
    let mut last_accepted_seq: Option<u16> = None;

    for (valid_hmac, apid, svc, sub) in simulated_uplink {
        seq += 1;
        println!("[tc_receiver] entrante: APID=0x{apid:03X} svc={svc}/{sub} seq={seq}");

        // Paso 1: verificar HMAC
        if !valid_hmac {
            println!("[tc_receiver] RECHAZADO: HMAC incorrecto");
            continue;
        }

        // Paso 2: verificación de repetición (simplificado: rechazar mismo apid+svc+sub consecutivo)
        let is_replay = last_accepted_seq == Some(seq.wrapping_sub(0));
        if is_replay {
            println!("[tc_receiver] RECHAZADO: repetición detectada");
            continue;
        }

        last_accepted_seq = Some(seq);
        println!("[tc_receiver] REENVIANDO al router (verificado)");
        let tc = VerifiedTc { apid, service: svc, subservice: sub, data: vec![] };
        if tx.send(tc).await.is_err() { break; }
    }
    // Soltar tx señala al router que el enlace ascendente está cerrado
}

// ── Lado confiable: router OBC ────────────────────────────────────────────────

/// Recibe SOLO TCs verificados de tc_receiver.
/// Este componente SÍ tiene acceso a subsistemas y actuadores.
async fn obc_router(mut rx: mpsc::Receiver<VerifiedTc>) {
    while let Some(tc) = rx.recv().await {
        println!("[obc_router] enrutando TC verificado({}/{}) APID=0x{:03X}",
                 tc.service, tc.subservice, tc.apid);
        match tc.service {
            17 => println!("[obc_router] → respuesta ping: TM(17,2) Estoy Vivo"),
            3  => println!("[obc_router] → servicio HK: recopilar parámetros, enviar TM(3,25)"),
            _  => println!("[obc_router] → servicio desconocido, descartando"),
        }
    }
    println!("[obc_router] enlace ascendente cerrado");
}

#[tokio::main]
async fn main() {
    println!("=== Arquitectura de separación de privilegios ===\n");
    println!("Frontera de seguridad: tc_receiver ──(canal)──► obc_router");
    println!("  tc_receiver: conoce las claves de autenticación, habla con el hardware RF");
    println!("  obc_router:  conoce los subsistemas de la nave, habla con los actuadores");
    println!("  Comprometer tc_receiver NO puede comandar actuadores directamente.\n");

    let (tx, rx) = mpsc::channel(16);
    let receiver = tokio::spawn(tc_receiver(tx));
    let router   = tokio::spawn(obc_router(rx));

    let _ = tokio::join!(receiver, router);
    println!("\nSimulación completa.");
}
