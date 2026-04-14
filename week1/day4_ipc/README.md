# Day 4 — Inter-Process Communication (IPC)

**Theme:** Moving data between processes reliably — the plumbing of a real OBC software stack.

A flight software stack is not a monolith. TC receiver, housekeeping service, FDIR monitor,
and payload manager are separate processes. This day teaches the four IPC mechanisms you will
actually use, and when to choose each.

---

## Learning Goals

- Frame length-delimited messages over Unix Domain Sockets with Tokio + `tokio-util`
- Use named pipes (FIFOs) for unidirectional, sequential data
- Leverage POSIX message queue priorities to ensure high-priority commands jump the queue
- Call D-Bus services with `zbus` for structured, typed IPC

---

## Examples

| File | What it demonstrates |
|------|----------------------|
| `01_uds_server.rs` | `UnixListener`, `Framed<LengthDelimitedCodec>`, bincode Request/Response |
| `02_uds_client.rs` | `UnixStream::connect`, matching codec, async send/recv loop |
| `03_named_pipe.rs` | `nix::unistd::mkfifo`, two tasks sharing a FIFO path |
| `04_posix_mq.rs` | `nix::mqueue`, priority send, demonstrate PRIO_HIGH arrives first |
| `05_dbus_intro.rs` | `zbus::interface`, `zbus::proxy`, session bus health service |

Run the UDS pair (two terminals):
```
# Terminal 1
cargo run -p day4-ipc --example 01_uds_server

# Terminal 2
cargo run -p day4-ipc --example 02_uds_client
```

---

## Exercises

### Exercise 1 — TM Bus (`ex1_tm_bus.rs`)

Implement `TmBus`, a publish/subscribe router for telemetry frames:

```rust
pub struct TmBus { /* ... */ }

impl TmBus {
    pub fn new() -> Self;
    pub fn subscribe(&mut self, apid: u16) -> Receiver<TmFrame>;
    pub fn publish(&self, frame: TmFrame) -> Result<(), BusError>;
}
```

- Each APID gets its own channel
- `publish` fans out to all subscribers for that APID
- `subscribe` on an already-registered APID returns a second receiver (broadcast)

Solution: `ex1_tm_bus_sol.rs`

---

## IPC Decision Matrix

| Mechanism | Latency | Throughput | Ordering | Persistence | Best for |
|-----------|---------|------------|----------|-------------|----------|
| UDS stream | ~1 µs | High | FIFO | None | Bidirectional command/response |
| UDS datagram | ~1 µs | High | None | None | Fire-and-forget events |
| Named pipe | ~5 µs | Medium | FIFO | None | Unidirectional byte streams |
| POSIX MQ | ~5 µs | Medium | Priority | Kernel | Priority-ordered commands |
| D-Bus | ~50 µs | Low | Per-method | None | Typed service calls, introspection |

---

## Key Concepts

### Length-delimited framing

Raw TCP and UDS streams are byte streams — there are no message boundaries. `LengthDelimitedCodec`
prepends a 4-byte big-endian length to each message, so the receiver knows exactly how many bytes
to read before calling the deserializer.

```
[len: u32 BE][payload bytes...]
```

Pair with `bincode` for compact binary serialization, or `serde_json` when human-readability matters.

### POSIX MQ priorities

`mq_send` accepts a priority (0–31 on Linux). `mq_receive` always returns the oldest message
at the *highest* priority, regardless of send order. This is exactly what you want for a TC
uplink queue: EMERGENCY_STOP packets preempt routine parameter uploads.

### Why not shared memory?

Shared memory is fastest (zero copy) but requires explicit synchronization — mutexes, semaphores,
or lock-free structures. It has no inherent message boundary, no blocking receive, and
easy-to-miss memory-ordering bugs. Reserve it for truly latency-critical, high-bandwidth paths
(e.g., streaming sensor data at 10 MHz). For command/response and telemetry distribution,
UDS or POSIX MQ are safer and simpler.
