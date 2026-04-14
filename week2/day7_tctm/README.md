# Day 7 — Telecommand & Telemetry (TC/TM)

**Theme:** CCSDS Space Packet Protocol and PUS-C — the language spacecraft speak.

Every byte that travels between a ground station and an on-board computer follows a defined
standard. This day builds a working implementation of the packet formats you will encounter
in the interview and on the job.

---

## Learning Goals

- Implement the 6-byte CCSDS primary header with correct bit manipulation
- Compute and verify CRC-CCITT (polynomial 0x1021, init 0xFFFF)
- Build PUS-C TC (telecommand) and TM (telemetry) packet structures
- Route packets by APID to per-service handlers
- Detect sequence counter gaps and 14-bit wrap-around

---

## Packet Structure (ASCII diagram)

```
CCSDS Primary Header (6 bytes, mandatory for all Space Packets):

 Bit: 0         1         2         3         4         5
      0123456789012345678901234567890123456789012345678901234567
      │ Version │T│SH│   APID (11 bits)   │SF│   Seq Count    │   Pkt Data Len  │
      │  (3b)   │C│  │                    │(2│   (14 bits)    │   (16 bits)     │
      └─────────┴─┴──┴────────────────────┴──┴────────────────┴─────────────────┘

T  = Packet Type: 0=TM, 1=TC
SH = Secondary Header Flag: 1=present
SF = Sequence Flags: 11=standalone, 01=first, 00=continuation, 10=last
Pkt Data Len = (total_packet_length - 7), i.e. bytes after primary header minus 1
```

---

## The `spacepacket` Library

All packet types are implemented in the `spacepacket` sub-crate:

```
spacepacket/src/
├── lib.rs              re-exports + module docs
├── primary_header.rs   CcsdsPrimaryHeader, PacketType, SeqFlags
├── crc.rs              crc_ccitt(), append_crc(), verify_and_strip_crc()
├── pus_tc.rs           PusTelecommand (5-byte secondary header)
├── pus_tm.rs           PusTelemetry  (10-byte secondary header + OBT)
├── apid_router.rs      ApidRouter, route_packet()
└── error.rs            PacketError enum
```

---

## Examples

| File | What it demonstrates |
|------|----------------------|
| `01_build_tc.rs` | Build TC(17,1) ping, print byte-by-byte field breakdown |
| `02_parse_tm.rs` | Build and parse TM(3,25) housekeeping with fake sensor data |
| `03_route_packets.rs` | Route 12 packets across 4 APIDs to per-service threads |
| `04_sequence_counter.rs` | Gap detection, per-APID counters, 14-bit wrap at 0x3FFF |

Run an example:
```
cargo run -p day7-tctm --example 01_build_tc
```

---

## Exercises

### Exercise 1 — HK Service (`ex1_hk_service.rs`)

Implement a `HkService` that:

1. Accepts `register_parameter(id: u16, name: &str)` calls at startup
2. Accepts `update_parameter(id: u16, value: f32)` calls from sensors
3. Produces a `PusTelemetry` (service 3, subservice 25) via `build_report(apid, seq)`

The report encodes all registered parameters as `[id: u16 LE][value: f32 LE]` pairs in the
application data field.

Solution: `ex1_hk_service_sol.rs`

---

## Key Concepts

### APID allocation

APIDs 0x000–0x7FF are user-defined per mission. A typical allocation:

| APID | Service |
|------|---------|
| 0x001 | TC Verification (service 1) |
| 0x002 | Housekeeping (service 3) |
| 0x003 | Event Reporting (service 5) |
| 0x100–0x1FF | Payload subsystems |
| 0x7FF | Broadcast (no specific destination) |

### CRC-CCITT

The standard PUS-C error-detection code. Every TC and TM packet ends with a 2-byte CRC.
The receiver recomputes the CRC over bytes 0..(N-2) and compares with bytes (N-2)..(N).

Test vector: `crc_ccitt(b"123456789")` == `0x29B1`

### Packet Data Length field

Confusingly named — it is the number of octets in the Packet Data Field *minus one*,
not the total packet length. So a 10-byte data field → PDL = 9. Total packet = 6 + PDL + 1.

### OBT (On-Board Time)

PUS-C TM secondary headers include a timestamp. The standard format is CCSDS CUC:
- 4 bytes coarse time (integer seconds since epoch)
- 2 bytes fine time (sub-second, units of 2⁻¹⁶ seconds ≈ 15 µs resolution)
