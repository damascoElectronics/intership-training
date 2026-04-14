# Week 1 Project — Sensor Daemon

**Capstone for Week 1.** Brings together async Rust, Linux I/O, IPC, and FDIR into one
runnable multi-process mini-stack.

---

## Architecture

```
┌─────────────────┐      Unix socket        ┌──────────────────────┐
│   sensor-sim    │ ──── SensorFrame ──────► │   sensor-daemon      │
│  (simulator)    │      (len-prefix)        │                      │
└─────────────────┘                          │  ┌────────────────┐  │
                                             │  │  HealthTable   │  │
                                             │  │  FDIR monitor  │  │
                                             │  └────────────────┘  │
                                             │  ┌────────────────┐  │
                                             │  │TelemetryBuffer │  │
                                             │  │ stats/archive  │  │
                                             │  └────────────────┘  │
                                             │  ┌────────────────┐  │
                                             │  │  Supervisor    │  │
                                             │  │ restart/backoff│  │
                                             │  └────────────────┘  │
                                             └──────────────────────┘
```

---

## Running

**Terminal 1 — start the daemon:**
```bash
cargo run -p week1-project --bin sensor-daemon
```

**Terminal 2 — start the simulator:**
```bash
cargo run -p week1-project --bin sensor-sim

# To exercise the crash-recovery path:
cargo run -p week1-project --bin sensor-sim -- --crash
```

Or use the convenience script:
```bash
bash tools/run_obc_stack.sh
```

---

## What to observe

1. **Normal operation** — the daemon logs incoming frames with temperature and pressure readings
2. **Fault injection** — every 10th frame the simulator sends a corrupted reading; the FDIR
   monitor transitions the sensor to `Degraded` after 3 consecutive faults
3. **Crash recovery** — with `--crash` the simulator exits after a few frames; the supervisor
   restarts it with exponential backoff (100 ms, 200 ms, 400 ms, …)
4. **Graceful shutdown** — press `Ctrl+C`; the daemon drains in-flight frames and exits cleanly

---

## Source Map

| File | Responsibility |
|------|---------------|
| `src/main.rs` | Tokio runtime, SIGTERM handler, task orchestration |
| `src/sensor.rs` | `SensorFrame` type, socket reader, length-prefix protocol |
| `src/health.rs` | `HealthTable`, state machine transitions, structured logging |
| `src/telemetry.rs` | `TelemetryBuffer` (ring buffer), `TelemetryStats` (min/max/mean) |
| `src/supervisor.rs` | Exponential backoff restart loop, max-restart guard |
| `simulator/sensor_sim.rs` | Synthetic sensor data, fault injection, `--crash` flag |

---

## Concepts Practised

- `tokio::select!` for concurrent socket reads and shutdown signals
- `CancellationToken` propagated through a task tree
- `UnixListener` + length-prefix framing (Day 2 + Day 4 combined)
- Health state machine with logged transitions (Day 5 FDIR)
- Supervisor with exponential backoff (Day 5 FDIR)
- Structured JSON logging with `tracing` + `tracing-subscriber`
