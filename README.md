# Rust Embedded Spacecraft Software — 2-Week Internship Prep

A structured, hands-on training repository for the **Spacecraft Embedded Software Engineer (Rust)** role.
Every topic is grounded in real spacecraft software patterns.

---

## Prerequisites

| Tool | Version | Install |
|------|---------|---------|
| Rust (stable) | ≥ 1.78 | `curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs \| sh` |
| cargo-watch | latest | `cargo install cargo-watch` |
| libdbus | system | `sudo apt-get install libdbus-1-dev pkg-config` |
| gcc | system | `sudo apt-get install build-essential` |

```bash
# Verify your setup
cargo --version        # should print cargo 1.78+
rustc --version        # same toolchain
cargo clippy --version
```

---

## Repository Structure

```
intership-training/
├── week1/              # Linux systems plumbing in Rust
│   ├── day1_async_foundations/   tokio, tasks, channels, graceful shutdown
│   ├── day2_linux_io/            sysfs, serial/UART, character devices, inotify
│   ├── day3_ffi/                 C/Rust FFI, bindgen, cbindgen, unsafe contracts
│   ├── day4_ipc/                 Unix sockets, pipes, POSIX MQ, D-Bus
│   ├── day5_fdir/                watchdog, circuit breaker, supervisor, safe state
│   └── week1_project/            Mini-project: sensor daemon integrating days 1–5
│
├── week2/              # Spacecraft-grade Rust
│   ├── day6_rustdoc/             rustdoc, doc tests, deny(missing_docs)
│   ├── day7_tctm/                CCSDS packets, PUS-C TC/TM, APID routing
│   ├── day8_security/            capabilities, seccomp, HMAC auth, unsafe audit
│   ├── day9_verification/        proptest, Loom, Miri, cargo-fuzz
│   └── week2_project/            Capstone: simplified OBC software stack
│
├── reference/          # Cheat sheets: CCSDS primer, PUS catalog, IPC matrix
└── tools/              # Helper scripts
```

---

## How to Use This Repo

### Day-by-day workflow

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

### Build the whole workspace

```bash
# From repo root:
cargo build --workspace
cargo test --workspace
cargo clippy --workspace --all-targets
cargo doc --workspace --no-deps --open
```

### Run CI checks locally

```bash
./tools/check_workspace.sh
```

---

## 2-Week Schedule

### Week 1 — Linux Systems Plumbing

| Day | Topic | Key Rust Concepts |
|-----|-------|-------------------|
| 1 | Async Foundations | tokio, spawn, channels, select!, shutdown |
| 2 | Linux I/O | sysfs, tty/serial, character devices, inotify |
| 3 | C/Rust FFI | extern "C", repr(C), bindgen, build.rs, unsafe |
| 4 | IPC | Unix sockets, named pipes, POSIX MQ, D-Bus |
| 5 | FDIR | watchdog, circuit breaker, supervisor tree |
| 5+ | Week 1 Project | Sensor daemon — integrates everything above |

### Week 2 — Spacecraft-Grade Rust

| Day | Topic | Key Rust Concepts |
|-----|-------|-------------------|
| 6 | API Documentation | rustdoc, doc tests, intra-doc links |
| 7 | TC/TM Systems | CCSDS, PUS-C, APID routing, sequence counters |
| 8 | Security | Linux capabilities, seccomp, HMAC, unsafe audit |
| 9 | Verification | proptest, Loom, Miri, cargo-fuzz |
| 10 | Week 2 Capstone | OBC software stack — all topics integrated |

---

## Skill Gap Map

| Gap (from job description) | Where it's covered |
|----------------------------|--------------------|
| Rust daemons + low-level protocols | day2, day4, week1_project |
| Embedded ↔ higher-level process comms | day4 (IPC), week2_project |
| Integrate C code with Rust | day3 (FFI) |
| Fault-tolerant systems | day5 (FDIR), week2_project |
| API documentation | day6 (rustdoc) |
| IPC protocols | day4 |
| Telemetry & command systems | day7 (TC/TM) |
| Cybersecurity for embedded | day8 |
| Formal verification methods | day9 |

---

## Standards & References

- **CCSDS 133.0-B-2** — Space Packet Protocol (TC/TM packet structure)
- **ECSS-E-ST-70-41C** — Packet Utilization Standard (PUS services)
- **ECSS-Q-ST-80C** — Software product assurance
- [The Rust Reference — Unsafe Code](https://doc.rust-lang.org/reference/unsafe-code.html)
- [Tokio Tutorial](https://tokio.rs/tokio/tutorial)
- [Linux man-pages — capabilities(7)](https://man7.org/linux/man-pages/man7/capabilities.7.html)
- [The Rustonomicon](https://doc.rust-lang.org/nomicon/) — unsafe Rust internals
