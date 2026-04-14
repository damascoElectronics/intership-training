# Week 2 Project — OBC Software Stack

**Capstone for Week 2.** A realistic on-board computer (OBC) software stack modelling the
architecture of a flight software system: five communicating processes, CCSDS/PUS-C packet
handling, HMAC authentication, replay protection, housekeeping service, FDIR, and a ground
simulator test harness.

---

## Architecture

```
                    ┌─────────────┐
                    │  ground-sim │  (test harness — sends TCs, reads TMs)
                    └──────┬──────┘
                           │ UDS /tmp/obc_tc_uplink.sock
                           │ HMAC-signed CCSDS TC packets
                           ▼
                    ┌─────────────┐
                    │ tc-receiver │  verifies HMAC, checks replay window, forwards to router
                    └──────┬──────┘
                           │ UDS /tmp/obc_router.sock
                           │ raw verified TC bytes (length-prefixed)
                           ▼
                    ┌─────────────┐
                    │  obc-router │  routes by PUS service number
                    └──────┬──────┘
                 ┌─────────┴──────────┐
                 │ svc=17 (ping)      │ svc=3 (HK)        svc=* (sensors)
                 │ responds inline    ▼                    ▼
                 │          ┌──────────────┐    ┌───────────────────┐
                 │          │  hk-service  │    │  sensor-daemon    │
                 │          │  /proc stats │    │  FDIR state mach. │
                 │          └──────────────┘    └───────────────────┘
                 │
                 └── TM responses flow back over separate UDS sockets
```

---

## Running the Stack

**Option A — convenience script (recommended):**
```bash
bash tools/run_obc_stack.sh
```

The script builds the workspace, starts all daemons in the background, runs `ground-sim`,
and cleans up on Ctrl+C.

**Option B — manual (four terminals):**
```bash
# Terminal 1
cargo run -p tc-receiver

# Terminal 2
cargo run -p obc-router

# Terminal 3
cargo run -p hk-service

# Terminal 4
cargo run -p sensor-daemon

# Terminal 5 (test harness)
cargo run -p ground-sim
```

---

## Expected ground-sim Output

```
[TEST 1] TC(17,1) ping...
  → Sent 20 bytes
  ← Received TM(17,2) pong    PASS

[TEST 2] TC(3,129) HK request...
  → Sent 20 bytes
  ← Received TM(3,25) HK report (N bytes)    PASS

[TEST 3] Bad HMAC rejection...
  → Sent corrupted TC
  ← Connection closed / no response    PASS

[TEST 4] Replay attack rejection...
  → Replayed TC(17,1) seq=1
  ← Rejected (duplicate sequence)    PASS

==========================================
Tests passed: 4 / 4
```

---

## Crate Map

| Crate | Role |
|-------|------|
| `obc_core` | Shared types: `SpacePacket`, `PusService`, `HealthState`, `IpcMessage`, `OBCError` |
| `tc_receiver` | HMAC-SHA256 verification, sliding-window replay protection, TC forwarding |
| `obc_router` | APID/service routing, TC(17,1) inline pong |
| `hk_service` | Reads `/proc/uptime` + `/proc/self/status`, produces TM(3,25) |
| `sensor_daemon` | Synthetic sensor, FDIR (3 consecutive faults → Degraded) |
| `ground_sim` | Test harness: builds signed TCs, validates TM responses, reports PASS/FAIL |

---

## Security Properties Exercised

| Property | Where implemented |
|----------|------------------|
| HMAC-SHA256 packet authentication | `tc_receiver` |
| Constant-time MAC comparison | `tc_receiver` (`subtle::ConstantTimeEq`) |
| Sliding-window replay protection | `tc_receiver` (`ReplayWindow` with `u64` bitmask) |
| No secret bytes in logs | All crates (only packet length + APID logged, never payload) |

---

## Concepts Integrated

This project exercises every topic from Week 2:
- **Day 6** — structured rustdoc on `obc_core` public API
- **Day 7** — CCSDS header parsing, PUS-C TC/TM construction, CRC verification
- **Day 8** — HMAC, replay window, privilege minimisation design
- **Day 9** — the `crc.rs` test vector property, round-trip invariant in `ground_sim`

And carries forward Week 1 skills:
- Async tasks with `tokio::select!` and `CancellationToken`
- Unix socket IPC with length-prefix framing
- FDIR state machine in `sensor_daemon`
- `/proc` parsing in `hk_service`
