# PUS-C Service Catalog
**ECSS-E-ST-70-41C**

---

## Mandatory Services for Minimum Viable OBC

| Service | Name | Key TCs | Key TMs |
|---------|------|---------|---------|
| **1** | TC Verification | _(none — ground doesn't send these)_ | 1,1=Accepted; 1,2=Rejected; 1,7=Completed; 1,8=Failed |
| **3** | Housekeeping | 3,129=Generate one-shot report; 3,130=Enable periodic | 3,25=HK parameter report |
| **5** | Event Reporting | _(none — OBC generates events)_ | 5,1=Info; 5,2=Low severity; 5,3=Medium; 5,4=High |
| **17** | On-Board Operations | 17,1=Are You Alive ping | 17,2=I Am Alive pong |

---

## Recommended Services

| Service | Name | Key TCs | Key TMs |
|---------|------|---------|---------|
| **6** | Memory Management | 6,2=Load memory; 6,5=Dump memory | 6,6=Memory dump report |
| **9** | Time Management | 9,128=Set On-Board Time | 9,2=Current OBT report |
| **11** | Time-Based Scheduling | 11,4=Insert TC into schedule | 11,10=Summary report |
| **12** | On-Board Monitoring | 12,1=Enable parameter monitoring | 12,12=Out-of-limit report |
| **20** | Parameter Management | 20,128=Set parameter value; 20,129=Get parameter | 20,130=Parameter report |

---

## Service 1 — TC Verification Details

Every accepted TC that requests it should generate these TMs:

```
TC received   → TM(1,1) Acceptance Successful  OR  TM(1,2) Acceptance Failed
TC executed   → TM(1,7) Completion Successful  OR  TM(1,8) Completion Failed
```

The subservice distinguishes which stage failed, making debugging much easier.

---

## Service 3 — Housekeeping Details

**TC(3,129) — Generate One-Shot HK Report**
- App data: [Report ID: u8] identifying which parameter set to report
- Response: TM(3,25) with the parameter values

**TM(3,25) — HK Parameter Report format** (depends on report definition):
- App data: [Report ID: u8] [parameter values in order]

---

## Service 17 — On-Board Operations (Connectivity Test)

The simplest possible end-to-end test:

```
Ground: TC(17,1) → OBC
OBC:    TM(1,1) + TM(17,2) → Ground
```

If you receive TM(17,2), the uplink, OBC software, and downlink all work.
This is the first TC you implement and the last thing you test before launch.

---

## TC Source IDs (convention)

| ID | Source |
|----|--------|
| 0 | Primary ground station |
| 1 | Secondary ground station |
| 2 | Onboard autonomy (OBC commanding itself) |
| 255 | Simulation/test |

---

## Common Mistakes

1. **Forgetting TC Verification** — every accepted/executed TC should get a TM(1,x)
2. **APID vs Service** — route by APID, not by service number
3. **Sequence count per APID** — not one global counter
4. **CRC** — verify on receive, append on transmit, every time
5. **OBT in TM** — must be monotonic; wrong OBT confuses ground analysis tools
