# CCSDS Space Packet Protocol — Primer
**CCSDS 133.0-B-2**

---

## Primary Header Bit Layout (6 octets, always present)

```
Byte 0          Byte 1          Byte 2          Byte 3          Byte 4          Byte 5
 7 6 5 4 3 2 1 0  7 6 5 4 3 2 1 0  7 6 5 4 3 2 1 0  7 6 5 4 3 2 1 0  7 6 5 4 3 2 1 0  7 6 5 4 3 2 1 0
├───────┬─┬─┬───────────────────┤├──┬─────────────────────────────────┤├──────────────────────────────────┤
│ 0 0 0 │T│S│  APID [10:8]     ││FF│    SEQUENCE COUNT [13:0]        ││   DATA LENGTH FIELD [15:0]        │
│ ver=0 │C│H│  (3 bits)        ││  │    (14 bits)                    ││   (value = data_octets - 1)       │
└───────┴─┴─┴───────────────────┘└──┴─────────────────────────────────┘└──────────────────────────────────┘
                                 ←────── APID [7:0] ──────────────────→

Full APID = Byte0[2:0] concatenated with Byte1[7:0] = 11 bits total
```

### Field Summary

| Field | Bits | Values | Notes |
|-------|------|--------|-------|
| Version | 3 | Always `0b000` | CCSDS version 1 |
| Packet Type | 1 | `0`=TM, `1`=TC | TM=downlink, TC=uplink |
| Sec Hdr Flag | 1 | `1` if PUS | PUS always sets this |
| APID | 11 | `0x000`–`0x7FE` | `0x7FF` reserved for idle |
| Seq Flags | 2 | `11`=standalone, `01`=first, `10`=last, `00`=continuation | Most packets are standalone |
| Seq Count | 14 | `0x0000`–`0x3FFF` | Per-APID, wraps at 0x3FFF |
| Data Length | 16 | `data_octets - 1` | Minimum 0 (1 byte of data) |

---

## APID Allocation (example for a small OBC)

```
APID 0x000 — undefined / not used
APID 0x001 — On-Board Operations service (PUS 17)
APID 0x002 — Sensor subsystem
APID 0x003 — Housekeeping service (PUS 3)
APID 0x004 — Communications subsystem
APID 0x100 — Attitude Control System
APID 0x200 — Power Management
APID 0x300 — Thermal Control
...
APID 0x7FF — IDLE (fill) packet — always discard
```

---

## Sequence Flags

| Bits | Meaning |
|------|---------|
| `11` | Standalone — single-packet message (most common) |
| `01` | First segment of a multi-packet message |
| `10` | Last segment |
| `00` | Continuation segment |

---

## PUS-C Packet Layout (on top of primary header)

```
┌──────────────────────────────────────────────────────────────────────────────────┐
│ CCSDS Primary Header (6 B)                                                        │
├──────────────────────────────────────────────────────────────────────────────────┤
│ PUS Secondary Header (variable)                                                   │
│   TC (5 B): [ver+spare (1B)] [service (1B)] [subservice (1B)] [source_id (2B)]   │
│   TM (10B): [ver+spare (1B)] [service (1B)] [subservice (1B)] [dest_id (2B)]     │
│             [OBT coarse (4B)] [OBT fine (2B)]                                    │
├──────────────────────────────────────────────────────────────────────────────────┤
│ Application Data (variable)                                                       │
├──────────────────────────────────────────────────────────────────────────────────┤
│ Packet Error Control — CRC-CCITT (2 B, polynomial 0x1021, init 0xFFFF)           │
└──────────────────────────────────────────────────────────────────────────────────┘
```

---

## On-Board Time (OBT) — CUC Format

| Field | Bytes | Content |
|-------|-------|---------|
| Coarse | 4 | Seconds since mission epoch (usually J2000 or UNIX epoch) |
| Fine | 2 | Sub-second ticks (65536 ticks/second) |

---

## Key Rules
1. **APID** is the "address" — route packets by APID, not by service/subservice
2. **Sequence count** increments per-APID; gaps indicate lost packets
3. **CRC** covers everything from primary header through application data (not the CRC itself)
4. **Idle packets** (APID 0x7FF) are fill and must be silently discarded
5. **Standalone** is correct for 99% of packets; segmentation is rare

---

## References
- CCSDS 133.0-B-2 (free download from public.ccsds.org)
- ECSS-E-ST-70-41C (PUS-C services, requires ECSS membership or purchase)
