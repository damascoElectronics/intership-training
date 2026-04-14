# Day 5: FDIR — Fault Detection, Isolation, and Recovery

## Overview

FDIR is a discipline born in spacecraft software engineering. When a satellite is in orbit
hundreds of kilometres above Earth, no engineer can reach over and reboot it. The software
must detect problems, contain their blast radius, and nurse the system back to health — all
without human intervention. Today we build the Rust patterns that make this possible.

This is the capstone theory day. Every pattern here will appear in the week 1 project.

---

## Prerequisites

- Day 1: Tokio async / tasks
- Day 2: Unix I/O and sockets
- Day 3: FFI (not directly used, but context for unsafe boundaries)
- Day 4: IPC, D-Bus, Unix sockets

---

## The Three Steps: Detect → Isolate → Recover

### 1. Fault Detection

You can't fix what you can't see. Detection mechanisms include:

- **Watchdogs**: A task must prove it is alive by periodically "kicking" a timer.
  If the timer expires, the watchdog fires. This catches deadlocks, infinite loops,
  and resource starvation — bugs that don't crash the program but render it useless.
- **Health monitoring**: Every component reports its state. A central health table
  accumulates this information and provides a system-level view.
- **Sanity checks**: Range validation, checksum verification, plausibility tests.
  A temperature sensor reading -500°C on an orbiting spacecraft is almost certainly
  a sensor fault, not a physics breakthrough.
- **Missed heartbeats**: Similar to watchdogs but at the communication level.
  If a subsystem stops sending telemetry, assume it has failed.

### 2. Fault Isolation

Once you detect a fault, you must prevent it from spreading:

- **Circuit breakers**: Stop hammering a failing component. If a sensor daemon
  returns errors on every call, stop calling it for a while and let it recover.
  Borrowed from distributed systems (Netflix Hystrix, etc.) but equally relevant
  in embedded daemons.
- **Sandboxing**: Run risky operations in separate processes or tasks so their
  failure doesn't corrupt global state.
- **Mode locking**: When something goes wrong, restrict what the system is allowed
  to do. You can't accidentally fire a thruster if you're in a mode that doesn't
  allow it. The typestate pattern enforces this at compile time.

### 3. Recovery

Recovery follows an **escalation chain**: try the cheapest option first; escalate
only if it fails.

```
Fault detected
    ↓
Try reset / retry (cheap)
    ↓ (still failing)
Switch to redundant component
    ↓ (no redundancy or redundancy also failed)
Enter safe mode (minimal-risk configuration)
    ↓ (cannot recover autonomously)
Wait for ground command / emergency mode
```

**Spacecraft example — star tracker failure:**
1. Star tracker returns invalid quaternion.
2. Mark component Degraded. Attempt soft reset (send reset command over I2C).
3. If still failing after 3 resets, switch to redundant star tracker.
4. If both star trackers failed, switch attitude control to magnetometers only
   (lower accuracy but functional). Enter Degraded mode.
5. If magnetometers also unreliable (e.g., near magnetic poles), enter Safe Mode:
   orient solar panels toward Sun, stop all non-essential operations, beacon
   telemetry on a slow schedule, wait for ground contact.
6. If even that fails: Emergency mode. Fire survival heaters, transmit emergency
   beacon, await ground intervention.

---

## Watchdog Patterns

### Hardware Watchdog

All modern microcontrollers (STM32, NXP LPC, etc.) have a hardware watchdog timer
(IWDG/WWDG). If not kicked within a configurable window, the MCU resets.
This is the last line of defence — a bare-metal guarantee.

In Linux-based flight computers (Raspberry Pi CM4, Jetson, custom SBCs), the kernel
exposes `/dev/watchdog`. Writing any byte to it kicks the hardware watchdog. If your
process dies without closing the fd (SO_KEEPALIVE is off), the hardware resets.

```bash
# Open /dev/watchdog and keep it alive — closing without writing resets the system
echo 1 > /dev/watchdog   # kick
echo V > /dev/watchdog   # magic close: disarm before clean shutdown
```

### Software Watchdog Daemon

A software watchdog monitors multiple tasks within a single process. Each task gets
a `WatchdogToken`; calling `.kick()` resets its timer. A background monitor task
checks all tokens. This catches per-task hangs without a full process reset.

This is the pattern in `examples/01_watchdog.rs`.

---

## Circuit Breaker

Originated in distributed systems (Fowler 2014). Works like a physical circuit
breaker: trips when too many failures occur, preventing further damage until the
fault clears.

```
         Failure count < threshold
         ┌─────────────────────────┐
         │                         │
    ┌────▼─────┐   threshold    ┌──┴──────┐
    │  CLOSED  │──────────────►│  OPEN   │
    │ (normal) │               │(failing)│
    └──────────┘               └────┬────┘
         ▲                          │
         │    probe succeeds        │ timeout elapsed
         │   ┌───────────┐          │
         └───│ HALF-OPEN │◄─────────┘
             │ (testing) │
             └───────────┘
                   │ probe fails
                   │────────────►back to OPEN
```

States:
- **Closed**: Normal operation. Failures increment counter. Counter ≥ threshold → Open.
- **Open**: All calls fail immediately (fast-fail). After recovery timeout → Half-Open.
- **Half-Open**: One probe call allowed through. Success → Closed. Failure → Open.

See `examples/02_circuit_breaker.rs`.

---

## Supervisor Trees (Erlang's Influence)

Erlang introduced the "let it crash" philosophy: instead of defensive coding for
every possible error, let processes crash and have a supervisor restart them.
The supervisor knows the restart policy (one-for-one, one-for-all, rest-for-one).

In Rust async, we adapt this: tasks are cheap, panics or `Err` returns indicate
fatal failure, supervisors restart with exponential backoff.

```
Supervisor
├── SensorTask     (restart: always, max: 5, backoff: 2^n * 100ms)
├── WatchdogTask   (restart: always, max: unlimited)
└── TelemetryTask  (restart: always, max: 3, backoff: 2^n * 500ms)
```

Key parameters:
- **max_restarts**: Prevent infinite restart loops (crash loop protection).
- **backoff**: Don't hammercycle a broken resource; give it time to recover.
- **restart policy**: one-for-one (restart just the failed task) is usually right
  for independent tasks; one-for-all is for tightly coupled task sets.

See `examples/04_supervisor.rs`.

---

## Typestate Pattern for Mode Management

In spacecraft software, not all operations are legal in all modes. You must not
attempt an orbit manoeuvre when in Safe Mode. You must not downlink science data
when the link is not established.

The **typestate pattern** encodes these constraints in Rust's type system. Instead
of a runtime `if mode == SafeMode { return Err(...) }` check, the compiler rejects
illegal operations at compile time. There is zero runtime cost.

```rust
struct OBC<Mode> { _mode: PhantomData<Mode>, ... }

// Only callable when Mode = Nominal
impl OBC<Nominal> {
    fn fire_thruster(&self) { ... }
}

// Only callable when Mode = SafeMode
impl OBC<SafeMode> {
    fn run_diagnostics(&self) { ... }
}

// A function receiving OBC<SafeMode> cannot call fire_thruster —
// it simply doesn't exist on that type. Compile error, not runtime error.
```

See `examples/05_safe_state.rs`.

---

## FDIR vs Error Handling

These are **different levels of abstraction** and are often confused:

| | Error Handling | FDIR |
|---|---|---|
| **Level** | Function/module | System |
| **Goal** | Propagate and handle errors gracefully | Keep the system alive and safe |
| **Mechanism** | `Result<T, E>`, `?`, `match` | Watchdogs, circuit breakers, supervisors, health tables |
| **Scope** | One operation | Entire subsystem or mission |
| **Time horizon** | Milliseconds | Seconds to hours |
| **Actor** | The calling code | An autonomous monitoring system |

Example: A sensor read returns `Err(ChecksumMismatch)`. Error handling says
"retry once, then return Err up the call stack." FDIR says "this sensor has
returned errors 10 times in the last 30 seconds; mark it Degraded, switch to
backup, and notify the health table."

---

## ECSS-E-ST-70-11C FDIR Requirements (Overview)

The European Cooperation for Space Standardization defines requirements for
onboard software. Key FDIR-relevant clauses:

- **FDIR-1**: The OBSW shall implement a hierarchical FDIR with at least three levels.
- **FDIR-2**: Each level shall have a defined escalation path.
- **FDIR-3**: Safe mode shall disable all non-critical functions and minimise power.
- **FDIR-4**: All fault detection events shall be logged with timestamp and context.
- **FDIR-5**: Recovery actions shall be idempotent (safe to repeat).
- **FDIR-6**: The system shall enter safe mode autonomously within a defined time
  after loss of ground contact.

These requirements directly motivate our patterns: the health table satisfies FDIR-4,
the typestate safe mode satisfies FDIR-3, the supervisor satisfies FDIR-2.

---

## The Safe State Concept

A **safe state** is a well-defined, minimal-risk system configuration. The system
retreats to it when it doesn't know what else to do safely.

Properties of a good safe state:
1. **Reachable from any other state**: You can always get there.
2. **Stable**: Once in safe state, you stay there until explicitly commanded out.
3. **Known power budget**: Solar panels deployed, heaters on, non-essentials off.
4. **Communication preserved**: You can still receive ground commands.
5. **No hazardous actions possible**: Thrusters disabled, pyros safed.
6. **Telemetry continues**: You can see what's wrong from the ground.

In our code, `OBC<SafeMode>` enforces properties 4 and 5 at compile time.

---

## Examples

| File | Concept | Run Command |
|------|---------|-------------|
| `01_watchdog.rs` | Software watchdog, per-task timers | `cargo run --example 01_watchdog` |
| `02_circuit_breaker.rs` | Circuit breaker state machine | `cargo run --example 02_circuit_breaker` |
| `03_health_table.rs` | Shared health table, concurrent access | `cargo run --example 03_health_table` |
| `04_supervisor.rs` | Task supervision with backoff | `cargo run --example 04_supervisor` |
| `05_safe_state.rs` | Typestate mode management | `cargo run --example 05_safe_state` |

## Exercises

| File | Task |
|------|------|
| `ex1_fdir_chain.rs` | Wire all patterns together around an ADC daemon |
| `ex1_fdir_chain_sol.rs` | Reference solution |
