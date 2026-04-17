# Proyecto Semana 1 — Daemon Sensor

**Capstone de la Semana 1.** Une async Rust, I/O en Linux, IPC y FDIR en una
mini-pila multiproceso ejecutable.

---

## Arquitectura

```
┌─────────────────┐      Socket Unix        ┌──────────────────────┐
│   sensor-sim    │ ──── SensorFrame ──────► │   sensor-daemon      │
│  (simulador)    │      (prefijo longitud)  │                      │
└─────────────────┘                          │  ┌────────────────┐  │
                                             │  │  HealthTable   │  │
                                             │  │  monitor FDIR  │  │
                                             │  └────────────────┘  │
                                             │  ┌────────────────┐  │
                                             │  │TelemetryBuffer │  │
                                             │  │ stats/archivo  │  │
                                             │  └────────────────┘  │
                                             │  ┌────────────────┐  │
                                             │  │  Supervisor    │  │
                                             │  │ reinicio/retro │  │
                                             │  └────────────────┘  │
                                             └──────────────────────┘
```

---

## Ejecución

**Terminal 1 — iniciar el daemon:**
```bash
cargo run -p week1-project --bin sensor-daemon
```

**Terminal 2 — iniciar el simulador:**
```bash
cargo run -p week1-project --bin sensor-sim

# Para ejercitar la ruta de recuperación ante crash:
cargo run -p week1-project --bin sensor-sim -- --crash
```

O usar el script de conveniencia:
```bash
bash tools/run_obc_stack.sh
```

---

## Qué observar

1. **Operación normal** — el daemon registra las tramas entrantes con lecturas de temperatura y presión
2. **Inyección de fallos** — cada 10ª trama el simulador envía una lectura corrupta; el monitor
   FDIR transiciona el sensor a `Degraded` tras 3 fallos consecutivos
3. **Recuperación ante crash** — con `--crash` el simulador sale tras unas pocas tramas; el supervisor
   lo reinicia con retroceso exponencial (100 ms, 200 ms, 400 ms, …)
4. **Apagado elegante** — pulsar `Ctrl+C`; el daemon drena las tramas en vuelo y termina limpiamente

---

## Mapa de Código Fuente

| Archivo | Responsabilidad |
|---------|----------------|
| `src/main.rs` | Runtime Tokio, manejador SIGTERM, orquestación de tareas |
| `src/sensor.rs` | Tipo `SensorFrame`, lector de socket, protocolo de prefijo de longitud |
| `src/health.rs` | `HealthTable`, transiciones de máquina de estados, logging estructurado |
| `src/telemetry.rs` | `TelemetryBuffer` (buffer circular), `TelemetryStats` (min/max/media) |
| `src/supervisor.rs` | Bucle de reinicio con retroceso exponencial, guardia de máximo de reinicios |
| `simulator/sensor_sim.rs` | Datos de sensor sintéticos, inyección de fallos, flag `--crash` |

---

## Conceptos Practicados

- `tokio::select!` para lecturas concurrentes de socket y señales de apagado
- `CancellationToken` propagado a través de un árbol de tareas
- `UnixListener` + enmarcado con prefijo de longitud (Día 2 + Día 4 combinados)
- Máquina de estados de salud con transiciones registradas (FDIR Día 5)
- Supervisor con retroceso exponencial (FDIR Día 5)
- Logging JSON estructurado con `tracing` + `tracing-subscriber`
