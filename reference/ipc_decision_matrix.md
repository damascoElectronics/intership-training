# IPC Decision Matrix

| Mechanism | Latency | Throughput | Reliability | Directionality | Complexity | Best For |
|-----------|---------|------------|-------------|----------------|------------|----------|
| **Unix socket (SOCK_STREAM)** | ~1µs | High | Reliable (ordered, no loss) | Bidirectional | Low | General daemon IPC; request/response |
| **Unix socket (SOCK_DGRAM)** | ~0.5µs | High | May drop if buffer full | Unidirectional | Low | Log messages; best-effort telemetry |
| **Named pipe (FIFO)** | ~1µs | Medium | Reliable | Unidirectional | Very low | Simple data streams; log pipelines |
| **POSIX Message Queue** | ~2µs | Medium | Reliable, **priority-ordered** | Unidirectional | Medium | TC priority lanes; command queuing |
| **Shared memory** | ~100ns | Very high | No delivery guarantee; requires sync | Both | High | High-rate sensor data; large buffers |
| **D-Bus** | ~100µs | Low | Reliable | Both | High | Service discovery; health queries |

---

## Spacecraft-specific guidance

### Use Unix domain sockets when:
- Two daemons need bidirectional request/response
- You need framing (use `LengthDelimitedCodec`)
- You want access control via filesystem permissions
- **Examples**: tc_receiver → router, router → hk_service

### Use POSIX MQ when:
- You need PRIORITY ordering (TCs have different urgencies)
- One producer, one consumer
- Bounded buffer is acceptable (mq drops when full)
- **Examples**: TC priority lanes, fault event queuing

### Use shared memory when:
- You need to share large data buffers (e.g., image data, raw sensor streams)
- Ultra-low latency matters (microseconds vs milliseconds)
- You can handle synchronization carefully (mutex, semaphore, RCU)
- **Examples**: image processor feeding compressor; telemetry ring buffer

### Use D-Bus when:
- You need service discovery ("is the comms subsystem available?")
- You want introspection (what methods does this service expose?)
- Latency doesn't matter (health queries are not time-critical)
- **Examples**: OBC components registering their health status

### Avoid for spacecraft:
- **TCP/IP** over loopback: works but slower than UDS, no obvious advantage
- **SysV IPC** (msgget/shmget): older API, use POSIX equivalents instead

---

## Framing reminder

Unix sockets and FIFOs are **byte streams** — they have no message boundaries.
You MUST add framing. Common approaches:

| Method | Library | Use when |
|--------|---------|----------|
| Length prefix (4-byte big-endian length + payload) | `tokio_util::codec::LengthDelimitedCodec` | Binary protocols (most OBC IPC) |
| Newline delimiter | `tokio::io::BufReader::lines()` | ASCII debug/log channels |
| COBS + 0x00 delimiter | Manual (see day9 examples) | Embedded serial framing |
