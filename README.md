# Rust Embedded Spacecraft Software — Preparación para Pasantía de 2 Semanas

Un repositorio de entrenamiento estructurado y práctico para el rol de **Ingeniero de Software Embebido con use en Rust**.
Cada tema está fundamentado en patrones reales de software.

---

## Requisitos Previos

| Herramienta | Versión | Instalación |
|-------------|---------|-------------|
| Rust (stable) | ≥ 1.78 | `curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs \| sh` |
| cargo-watch | latest | `cargo install cargo-watch` |
| libdbus | sistema | `sudo apt-get install libdbus-1-dev pkg-config` |
| gcc | sistema | `sudo apt-get install build-essential` |

```bash
# Verificar la configuración del entorno
cargo --version        # should print cargo 1.78+
rustc --version        # same toolchain
cargo clippy --version
```

---

## Estructura del Repositorio

```
intership-training/
├── week1/              # Fontanería de sistemas Linux en Rust
│   ├── day1_async_foundations/   tokio, tasks, channels, graceful shutdown
│   ├── day2_linux_io/            sysfs, serial/UART, character devices, inotify
│   ├── day3_ffi/                 C/Rust FFI, bindgen, cbindgen, unsafe contracts
│   ├── day4_ipc/                 Unix sockets, pipes, POSIX MQ, D-Bus
│   ├── day5_fdir/                watchdog, circuit breaker, supervisor, safe state
│   └── week1_project/            Mini-proyecto: demonio de sensores que integra los días 1–5
│
├── week2/              # Rust de calidad espacial
│   ├── day6_rustdoc/             rustdoc, doc tests, deny(missing_docs)
│   ├── day7_tctm/                Paquetes CCSDS, PUS-C TC/TM, enrutamiento APID
│   ├── day8_security/            capabilities, seccomp, autenticación HMAC, auditoría unsafe
│   ├── day9_verification/        proptest, Loom, Miri, cargo-fuzz
│   └── week2_project/            Proyecto final: pila de software OBC simplificada
│
├── reference/          # Hojas de referencia rápida: introducción a CCSDS, catálogo PUS, matriz IPC
└── tools/              # Scripts de utilidad
```

---

## Cómo Usar Este Repositorio

### Flujo de trabajo día a día

```bash
# 1. Read the day's README.md first — concepts before code
# 2. Run all examples for that day:
cd week1/day1_async_foundations
cargo run --example 01_basic_runtime
cargo run --example 02_spawn_tasks
# ... etc

# 3. Attempt the exercise (file has TODO blocks and a failing #[test])
cargo test --example ex1_heartbeat   # should fail at first

# 4. Fill in the TODOs until the test passes
# 5. Compare your solution with ex1_*_sol.rs
```

### Compilar todo el workspace

```bash
# From repo root:
cargo build --workspace
cargo test --workspace
cargo clippy --workspace --all-targets
cargo doc --workspace --no-deps --open
```

### Ejecutar verificaciones de CI localmente

```bash
./tools/check_workspace.sh
```

---

## Cronograma de 2 Semanas

### Semana 1 — Fontanería de Sistemas Linux

| Día | Tema | Conceptos Clave de Rust |
|-----|------|-------------------------|
| 1 | Fundamentos de Asincronía | tokio, spawn, channels, select!, shutdown |
| 2 | E/S en Linux | sysfs, tty/serial, character devices, inotify |
| 3 | C/Rust FFI | extern "C", repr(C), bindgen, build.rs, unsafe |
| 4 | IPC | Unix sockets, named pipes, POSIX MQ, D-Bus |
| 5 | FDIR | watchdog, circuit breaker, árbol de supervisión |
| 5+ | Proyecto Semana 1 | Demonio de sensores — integra todo lo anterior |

### Semana 2 — Rust de Calidad Espacial

| Día | Tema | Conceptos Clave de Rust |
|-----|------|-------------------------|
| 6 | Documentación de API | rustdoc, doc tests, enlaces intra-doc |
| 7 | Sistemas TC/TM | CCSDS, PUS-C, enrutamiento APID, contadores de secuencia |
| 8 | Seguridad | Linux capabilities, seccomp, HMAC, auditoría unsafe |
| 9 | Verificación | proptest, Loom, Miri, cargo-fuzz |
| 10 | Proyecto Final Semana 2 | Pila de software OBC — todos los temas integrados |

---

## Mapa de Brechas de Habilidades

| Brecha (de la descripción del puesto) | Dónde se cubre |
|---------------------------------------|----------------|
| Demonios Rust + protocolos de bajo nivel | day2, day4, week1_project |
| Comunicación entre proceso embebido y procesos de alto nivel | day4 (IPC), week2_project |
| Integrar código C con Rust | day3 (FFI) |
| Sistemas tolerantes a fallos | day5 (FDIR), week2_project |
| Documentación de API | day6 (rustdoc) |
| Protocolos IPC | day4 |
| Sistemas de telemetría y comandos | day7 (TC/TM) |
| Ciberseguridad para embebidos | day8 |
| Métodos de verificación formal | day9 |

---

## Estándares y Referencias

- **CCSDS 133.0-B-2** — Space Packet Protocol (estructura de paquetes TC/TM)
- **ECSS-E-ST-70-41C** — Packet Utilization Standard (servicios PUS)
- **ECSS-Q-ST-80C** — Garantía de calidad del producto software
- [The Rust Reference — Unsafe Code](https://doc.rust-lang.org/reference/unsafe-code.html)
- [Tokio Tutorial](https://tokio.rs/tokio/tutorial)
- [Linux man-pages — capabilities(7)](https://man7.org/linux/man-pages/man7/capabilities.7.html)
- [The Rustonomicon](https://doc.rust-lang.org/nomicon/) — Internals de Rust inseguro
