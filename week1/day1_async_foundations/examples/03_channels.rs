// Día 1, Ejemplo 3: Canales de Tokio
//
// Los canales son el equivalente async de las colas de mensajes de FreeRTOS (xQueueSend /
// xQueueReceive), pero con seguridad de tipos y múltiples variantes para distintos patrones.
//
// Tokio proporciona tres tipos de canales. Elegir el incorrecto lleva a bugs o
// problemas de rendimiento — lee la tabla comparativa de abajo con cuidado.
//
// Ejecutar con:
//   cargo run --example 03_channels

use std::time::Duration;
use tokio::sync::{broadcast, mpsc, watch};

// ─────────────────────────────────────────────────────────────────────────────
// Tabla Comparativa de Canales
// ─────────────────────────────────────────────────────────────────────────────
//
// ┌──────────────┬──────────────────┬──────────────────────────────────────────┐
// │ Tipo Canal   │ Productores/Cons.│ Semántica                                │
// ├──────────────┼──────────────────┼──────────────────────────────────────────┤
// │ mpsc         │ Muchos Tx, un Rx │ Cada mensaje consumido por UN receptor   │
// │              │                  │ Acotado (contrapresión) o no acotado     │
// │              │                  │ Usar para: colas de trabajo, telemetría  │
// ├──────────────┼──────────────────┼──────────────────────────────────────────┤
// │ broadcast    │ Uno o Muchos Tx, │ Cada mensaje entregado a TODOS los recep.│
// │              │ muchos Rx        │ Los receptores pueden rezagarse (overflow)│
// │              │                  │ Usar para: eventos de sistema, cambios   │
// ├──────────────┼──────────────────┼──────────────────────────────────────────┤
// │ watch        │ Un Tx, muchos Rx │ Solo se conserva el ÚLTIMO valor         │
// │              │                  │ Valores viejos descartados al llegar uno │
// │              │                  │ Usar para: estado actual (modo, setpoint)│
// └──────────────┴──────────────────┴──────────────────────────────────────────┘
//
// También existe oneshot: exactamente un mensaje, un productor, un consumidor.
// Usar para: patrones petición-respuesta, retornar un resultado al llamador.

#[derive(Debug, Clone)]
struct TelemetryPacket {
    sensor_id: u8,
    value: f32,
    timestamp_ms: u64,
}

#[derive(Debug, Clone)]
enum HealthEvent {
    SensorOnline(u8),
    SensorOffline(u8),
    OverTemperature { zone: u8, temp_c: f32 },
}

#[derive(Debug, Clone, PartialEq)]
enum SystemMode {
    Nominal,
    SafeMode,
    Emergency,
}

#[tokio::main]
async fn main() {
    println!("=== Ejemplo 03: Canales ===\n");

    demo_mpsc().await;
    demo_broadcast().await;
    demo_watch().await;

    println!("\nListo.");
}

// ─────────────────────────────────────────────────────────────────────────────
// mpsc: Multi-Productor Consumidor-Único
//
// Caso de uso clásico: múltiples tareas sensor enviando telemetría a una única
// tarea agregadora. Cada paquete se consume exactamente una vez — sin duplicación.
//
// Equivale a una cola de FreeRTOS donde múltiples tareas empujan, una extrae.
// ─────────────────────────────────────────────────────────────────────────────

async fn demo_mpsc() {
    println!("--- mpsc: Agregación de Telemetría ---");

    // Canal acotado: buffer de hasta 32 paquetes.
    // Si el buffer está lleno, send().await bloqueará (contrapresión).
    // Importante: evita que un productor rápido abrume a un consumidor lento.
    // En términos embedded: es control de flujo, como el hardware flow control de UART pero en software.
    let (tx, mut rx) = mpsc::channel::<TelemetryPacket>(32);

    // Lanzar dos tareas sensor, cada una con un clon del sender.
    // mpsc::Sender es barato de clonar — todos los clones comparten el mismo canal subyacente.
    let tx1 = tx.clone();
    let sensor1 = tokio::spawn(async move {
        for i in 0..3 {
            let packet = TelemetryPacket {
                sensor_id: 1,
                value: 23.5 + i as f32 * 0.1,
                timestamp_ms: i * 100,
            };
            // send() retorna Err si el receptor ha sido descartado (canal cerrado).
            // En un daemon real manejarías esto como señal de apagado.
            if tx1.send(packet).await.is_err() {
                println!("  Sensor 1: canal cerrado, deteniendo");
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        println!("  Sensor 1: envío completado");
    });

    let tx2 = tx.clone();
    let sensor2 = tokio::spawn(async move {
        for i in 0..3 {
            let packet = TelemetryPacket {
                sensor_id: 2,
                value: 3.3 - i as f32 * 0.05,
                timestamp_ms: i * 100 + 50,
            };
            if tx2.send(packet).await.is_err() {
                println!("  Sensor 2: canal cerrado, deteniendo");
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        println!("  Sensor 2: envío completado");
    });

    // Descartar el sender original. El canal permanece abierto mientras exista ALGÚN clon del sender.
    // Cuando se descarta el ÚLTIMO sender, el receptor obtiene None de recv() — canal cerrado.
    drop(tx);

    // Tarea agregadora: recibir todos los paquetes hasta que se cierre el canal.
    // En código real esto construiría un informe de telemetría o escribiría en una base de datos.
    let aggregator = tokio::spawn(async move {
        let mut total = 0usize;
        // recv() retorna None cuando todos los Senders han sido descartados
        while let Some(pkt) = rx.recv().await {
            println!(
                "  Agregador recibió: sensor={} value={:.2} ts={}ms",
                pkt.sensor_id, pkt.value, pkt.timestamp_ms
            );
            total += 1;
        }
        println!("  Agregador: canal cerrado, recibidos {total} paquetes en total");
    });

    tokio::join!(sensor1, sensor2, aggregator).0.unwrap();
    println!();
}

// ─────────────────────────────────────────────────────────────────────────────
// broadcast: Publicar Eventos de Salud a Múltiples Suscriptores
//
// Usar cuando múltiples consumidores independientes necesitan ver TODOS los eventos.
// Ejemplo: un monitor de salud publica eventos; tanto el logger como el
// manejador de comandos necesitan reaccionar a ellos de forma independiente.
//
// A diferencia de mpsc, broadcast usa un buffer circular. Si un receptor es lento y
// el buffer se llena, los mensajes viejos se sobreescriben. El receptor obtiene
// RecvError::Lagged(n) indicando que perdió n mensajes.
// ─────────────────────────────────────────────────────────────────────────────

async fn demo_broadcast() {
    println!("--- broadcast: Distribución de Eventos de Salud ---");

    // Capacidad del buffer circular: 16 eventos.
    // Cuando está lleno, el mensaje no leído más antiguo se sobreescribe.
    let (tx, _) = broadcast::channel::<HealthEvent>(16);

    // Cada suscriptor obtiene su propio receptor llamando a .subscribe().
    // Los receptores son independientes: un receptor lento no bloquea a los demás.
    let mut rx_logger = tx.subscribe();
    let mut rx_cmd_handler = tx.subscribe();

    // Logger: registra todos los eventos
    let logger = tokio::spawn(async move {
        loop {
            match rx_logger.recv().await {
                Ok(event) => println!("  Logger: {:?}", event),
                Err(broadcast::error::RecvError::Lagged(n)) => {
                    println!("  Logger: AVISO — perdidos {n} eventos (desbordamiento de buffer)");
                }
                Err(broadcast::error::RecvError::Closed) => {
                    println!("  Logger: canal cerrado, deteniendo");
                    break;
                }
            }
        }
    });

    // Manejador de comandos: solo reacciona a eventos críticos
    let cmd_handler = tokio::spawn(async move {
        loop {
            match rx_cmd_handler.recv().await {
                Ok(HealthEvent::OverTemperature { zone, temp_c }) => {
                    println!("  CmdHandler: ¡ALERTA! Zona {zone} sobre temperatura a {temp_c:.1}°C — iniciando modo seguro");
                }
                Ok(HealthEvent::SensorOnline(id)) => {
                    println!("  CmdHandler: Sensor {id} en línea — nominal");
                }
                Ok(HealthEvent::SensorOffline(id)) => {
                    println!("  CmdHandler: Sensor {id} fuera de línea — modo degradado");
                }
                Err(broadcast::error::RecvError::Closed) => break,
                Err(broadcast::error::RecvError::Lagged(n)) => {
                    println!("  CmdHandler: perdidos {n} eventos");
                }
            }
        }
    });

    // Publicar eventos. Tanto el logger como el cmd_handler los reciben TODOS.
    let events = vec![
        HealthEvent::SensorOnline(1),
        HealthEvent::SensorOnline(2),
        HealthEvent::OverTemperature {
            zone: 3,
            temp_c: 95.7,
        },
        HealthEvent::SensorOffline(1),
    ];

    for event in events {
        // send() retorna el número de receptores que recibieron el mensaje.
        // Retorna Err si no hay receptores (todos descartados).
        let receivers = tx.send(event).expect("Sin receptores");
        println!("  Publisher: enviado a {receivers} receptores");
        tokio::time::sleep(Duration::from_millis(10)).await;
    }

    // Descartar el sender para cerrar el canal
    drop(tx);
    tokio::join!(logger, cmd_handler).0.unwrap();
    println!();
}

// ─────────────────────────────────────────────────────────────────────────────
// watch: Rastrear el Último Estado
//
// Usar cuando te importa el valor ACTUAL, no cada actualización histórica.
// Ejemplo: modo operativo del sistema. Si el modo cambia dos veces antes de que una tarea
// lo compruebe, la tarea solo necesita saber el modo actual — no el historial.
//
// Es como una variable global compartida, pero:
// - La escritura es atómica y consistente (sin lecturas parciales)
// - Los lectores pueden esperar eficientemente a que el valor cambie (changed().await)
// - Múltiples lectores, un escritor
// ─────────────────────────────────────────────────────────────────────────────

async fn demo_watch() {
    println!("--- watch: Estado del Modo del Sistema ---");

    // Valor inicial es SystemMode::Nominal
    let (tx, rx) = watch::channel(SystemMode::Nominal);

    // Tarea que reacciona a cambios de modo (p. ej., tarea de telemetría ajustando la tasa)
    let mut rx1 = rx.clone();
    let telemetry_task = tokio::spawn(async move {
        loop {
            // changed() espera hasta que el valor haya cambiado desde la última comprobación.
            // Es eficiente: sin sondeo, el sender nos notifica directamente.
            if rx1.changed().await.is_err() {
                println!("  Telemetría: canal watch cerrado");
                break;
            }
            // borrow() da una referencia al valor actual (no la propiedad).
            // El bloqueo de lectura se mantiene mientras exista el Ref — suéltalo rápido.
            let mode = rx1.borrow().clone();
            match &mode {
                SystemMode::Nominal => println!("  Telemetría: modo nominal — tasa normal"),
                SystemMode::SafeMode => println!("  Telemetría: modo seguro — tasa reducida"),
                SystemMode::Emergency => println!("  Telemetría: EMERGENCIA — solo datos críticos"),
            }
        }
    });

    // Otra tarea que también rastrea el modo
    let mut rx2 = rx.clone();
    let power_task = tokio::spawn(async move {
        loop {
            if rx2.changed().await.is_err() {
                break;
            }
            let mode = rx2.borrow().clone();
            if mode == SystemMode::SafeMode || mode == SystemMode::Emergency {
                println!("  Energía: entrando en modo de bajo consumo");
            } else {
                println!("  Energía: consumo nominal");
            }
        }
    });

    // Controlador de modo: simular transiciones de estado
    tokio::time::sleep(Duration::from_millis(10)).await;

    println!("  Controlador: transitando a SafeMode");
    tx.send(SystemMode::SafeMode).unwrap();
    tokio::time::sleep(Duration::from_millis(20)).await;

    println!("  Controlador: transitando a Emergency");
    tx.send(SystemMode::Emergency).unwrap();
    tokio::time::sleep(Duration::from_millis(20)).await;

    println!("  Controlador: volviendo a Nominal");
    tx.send(SystemMode::Nominal).unwrap();
    tokio::time::sleep(Duration::from_millis(20)).await;

    // Cerrar el sender (drop) hace que changed() retorne Err, terminando las tareas
    drop(tx);
    tokio::join!(telemetry_task, power_task).0.unwrap();
    println!();
}
