//! Ejemplo 03 — Tuberías con nombre (FIFOs)
//!
//! Un FIFO es el mecanismo IPC unidireccional más simple. A diferencia de una tubería
//! anónima (usada entre procesos padre e hijo), una tubería con nombre vive en el
//! sistema de archivos y puede ser abierta por procesos no relacionados.
//!
//! Comportamiento clave: la llamada open() SE BLOQUEA hasta que ambos extremos estén abiertos.
//! Esta es una fuente común de confusión — tanto el escritor como el lector deben
//! abrir el FIFO antes de que alguno pueda continuar.
//!
//! Ejecutar con:  cargo run --example 03_named_pipe

use nix::sys::stat::Mode;
use nix::unistd::mkfifo;
use std::path::Path;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::fs::OpenOptions;

const FIFO_PATH: &str = "/tmp/day4_fifo_demo";

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Crear el FIFO si no existe
    let path = Path::new(FIFO_PATH);
    if !path.exists() {
        mkfifo(path, Mode::S_IRUSR | Mode::S_IWUSR)?;
        println!("FIFO creado en {FIFO_PATH}");
    }

    println!("Iniciando tareas de productor y consumidor...");
    println!("Nota: ambas tareas deben abrir el FIFO antes de que alguna pueda continuar.\n");

    // El productor y el consumidor deben abrirse en tareas separadas — si intentas
    // abrir ambos extremos secuencialmente en una sola tarea, ocurre un deadlock (cada apertura bloquea
    // hasta que el otro extremo esté abierto).
    let producer = tokio::spawn(async {
        println!("[productor] abriendo FIFO para escritura (bloqueará hasta que el lector abra)...");
        let mut writer = OpenOptions::new()
            .write(true)
            .open(FIFO_PATH)
            .await
            .expect("abrir FIFO para escritura");
        println!("[productor] FIFO abierto, enviando mensajes");

        for i in 1..=5 {
            let msg = format!("mensaje #{i} del productor\n");
            writer.write_all(msg.as_bytes()).await.expect("escribir");
            println!("[productor] enviado: '{}'", msg.trim());
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        }
        println!("[productor] listo, cerrando FIFO");
        // Descartar writer cierra el extremo de escritura → el consumidor ve EOF
    });

    let consumer = tokio::spawn(async {
        println!("[consumidor] abriendo FIFO para lectura (bloqueará hasta que el escritor abra)...");
        let reader = OpenOptions::new()
            .read(true)
            .open(FIFO_PATH)
            .await
            .expect("abrir FIFO para lectura");
        println!("[consumidor] FIFO abierto, leyendo mensajes");

        let mut lines = BufReader::new(reader).lines();
        while let Some(line) = lines.next_line().await.expect("leer") {
            println!("[consumidor] recibido: '{line}'");
        }
        println!("[consumidor] EOF — el productor cerró el extremo de escritura");
    });

    producer.await?;
    consumer.await?;

    // Limpiar
    let _ = std::fs::remove_file(FIFO_PATH);

    println!("\nCuándo usar FIFOs:");
    println!("  ✓ Streams de datos simples en una dirección (ej., pipeline de logs: daemon → logger)");
    println!("  ✓ Amigable con el shell (cat, nc pueden leer/escribir FIFOs)");
    println!("  ✗ No apto para comunicación bidireccional (se necesitan dos FIFOs)");
    println!("  ✗ No apto para múltiples productores (sin garantía de enmarcado)");

    Ok(())
}
