# Day 8: Security for Embedded Linux Daemons

Spacecraft software runs on embedded Linux boards (e.g., a Raspberry Pi CM4 running
PetaLinux) connected to RF uplink hardware. The threat environment is different from
a corporate web server, but the principles are the same: **authenticate everything,
trust nothing, give each component only the privileges it needs**.

---

## Threat Model

### Uplink Spoofing (TC Injection)

A ground station transmits telecommands (TCs) over a radio link. Anyone within RF range
can transmit on the same frequency. Without authentication, an attacker can:

- Inject false telecommands (open a valve, disable a heater, switch off the OBC)
- Replay a previously-captured valid command at an inconvenient time
- Jam the link and substitute their own packets

**Mitigation**: HMAC-SHA256 authentication on every TC packet. The key is loaded
into the OBC before launch and never transmitted. Without the key, an attacker
cannot forge a valid MAC.

### Insider / Supply Chain Threats

The spacecraft OBC may run third-party libraries (CCSDS parsers, protocol stacks).
A malicious or buggy library should not be able to:

- Access actuator driver file descriptors it was never given
- Make arbitrary syscalls (e.g., `execve` to spawn a shell)
- Escalate privileges

**Mitigation**: Privilege separation (separate processes per function) + seccomp BPF
(allowlist only the syscalls each process legitimately needs).

### Memory Corruption

Even in Rust, `unsafe` code can introduce memory-safety bugs. A buffer overflow in
a C FFI call or an incorrect pointer cast can give an attacker control flow.

**Mitigation**: Minimize `unsafe`, write `SAFETY` comments for every unsafe block,
run Miri and AddressSanitizer in CI, fuzz parsers.

---

## Defense-in-Depth Layers

```
Uplink (RF)
    │
    ▼
[RF Receiver] ─── raw bytes ──► [tc_receiver daemon]
                                    │  authenticate (HMAC)
                                    │  replay check (seq window)
                                    │  seccomp: only net I/O syscalls
                                    │  runs as uid=2001, no capabilities
                                    ▼
                              Unix socket (security boundary)
                                    │
                                    ▼
                              [obc_router daemon]
                                    │  parse APID
                                    │  route to subsystem
                                    │  seccomp: only IPC syscalls
                                    ▼
                         [subsystem daemons] (actuators, sensors)
```

If `tc_receiver` is compromised, it can only send bytes over the Unix socket.
It cannot directly command actuators, read sensor data, or access the filesystem.

---

## Linux Capabilities

### Why `setuid root` Is Dangerous

The traditional Unix model is binary: root (uid=0) can do everything, everyone else
is restricted. A daemon that needs to open a raw socket must run as root — and if it
has a bug, the attacker gets a root shell.

### Capability-Based Security

Linux breaks root privileges into ~40 independent capabilities:

| Capability           | Allows                                          |
|----------------------|-------------------------------------------------|
| `CAP_NET_BIND_SERVICE`| Bind to ports < 1024                           |
| `CAP_SYS_RAWIO`      | Access raw I/O ports (`/dev/mem`, iopl)         |
| `CAP_NET_RAW`        | Open raw sockets (sniff/inject packets)         |
| `CAP_SYS_NICE`       | Set process priority / real-time scheduling     |
| `CAP_NET_ADMIN`      | Configure network interfaces                    |

**Principle of least privilege**: Start as root, acquire the few capabilities you
need for initialization, then **drop all others permanently**. Even if the process
is exploited, the attacker only gets the capabilities you kept.

### `PR_SET_NO_NEW_PRIVS`

After calling `prctl(PR_SET_NO_NEW_PRIVS, 1)`, the process and all its children
can never gain new privileges via `setuid` binaries or file capabilities. This is
a one-way door — you cannot undo it.

### Capability Drop Sequence

```
1. Start as root (needed to open raw socket / access /dev/spidev)
2. Open the privileged resources (raw socket, device file)
3. Drop all capabilities you don't need
4. setgid(daemon_gid)   ← must happen BEFORE setuid
5. setuid(daemon_uid)
6. prctl(PR_SET_NO_NEW_PRIVS, 1)
7. Load seccomp BPF filter
8. Enter main event loop
```

---

## Seccomp BPF

Seccomp (secure computing mode) restricts which syscalls a process may make.
With BPF (Berkeley Packet Filter) rules you can write an **allowlist**:

```
# For tc_receiver: we only need network I/O
ALLOW: read, write, recv, recvmsg, sendmsg, accept, close, epoll_wait, futex, exit
DENY ALL (SIGKILL)
```

If an attacker exploits a memory-corruption bug and tries to call `execve` or
`open("/etc/passwd")`, the kernel kills the process immediately.

The `seccomp` crate provides a safe Rust API. In this day's examples we show the
pattern with comments; a full seccomp implementation requires a separate crate
(`libseccomp` bindings) not in the workspace.

---

## HMAC-SHA256 for TC Authentication

### What It Provides

- **Integrity**: Any bit flip in the packet changes the MAC
- **Authentication**: Only someone with the key can produce a valid MAC
- **NOT confidentiality**: The packet contents are in plaintext; HMAC does not encrypt

For spacecraft TCs, confidentiality is typically not required (the commands are
not secret; preventing unauthorized execution is what matters).

### Construction

```
MAC = HMAC-SHA256(key, packet_bytes_excluding_mac_field)
```

The MAC (32 bytes) is appended to the end of each packet. The receiver:
1. Strips the last 32 bytes (the claimed MAC)
2. Recomputes HMAC over the remaining bytes
3. Compares in constant time

### Key Management

- The key is a 256-bit random value generated on the ground
- Loaded into the OBC non-volatile storage (NVS) before integration
- Never transmitted over any link
- Rotated between missions (or on orbit if a secure key-update channel exists)

---

## Timing Attacks

A **timing attack** exploits the fact that `==` on byte arrays short-circuits:
it returns `false` the moment it finds a differing byte. By measuring how long
verification takes, an attacker can learn how many bytes of their guess are correct.

```rust
// WRONG: timing-sensitive comparison
if computed_mac == received_mac { ... }

// RIGHT: constant-time comparison (subtle crate)
use subtle::ConstantTimeEq;
if computed_mac.ct_eq(&received_mac).into() { ... }
```

`subtle::ConstantTimeEq` always touches every byte regardless of where the first
difference is, so timing reveals nothing about the key.

In practice, timing attacks on MACs over a network are difficult due to jitter,
but **you must always use constant-time comparison** — the cost is zero, the risk
of not doing it is non-zero.

---

## Replay Attacks

An attacker records a valid, authenticated TC (e.g., "open fuel valve") and retransmits
it later. The HMAC is still valid — the attacker didn't modify anything.

**Mitigation: Sliding window sequence number**

Each TC carries a monotonically increasing 16-bit sequence number. The receiver
maintains a window of the last N sequence numbers seen:

```
                   window (64 bits)
last_seq=100  ──►  bit 0 = seq 100 seen
                   bit 1 = seq 99 seen
                   ...
                   bit 63 = seq 37 seen

Accept:  seq 101 (one ahead — advance window)
Accept:  seq 95  (in window, not yet seen — set its bit)
Reject:  seq 100 (in window, bit 0 already set — replay!)
Reject:  seq 36  (behind window — too old, assume replay)
```

Combined with HMAC, this prevents both forgery and replay.

---

## Rust Memory Safety and `unsafe`

### What Safe Rust Guarantees

- No null pointer dereferences
- No buffer overflows (bounds are checked at runtime, or proven at compile time)
- No use-after-free (the borrow checker ensures references don't outlive their data)
- No data races (the type system enforces Send/Sync)
- No uninitialized memory reads (the compiler requires initialization)

### What `unsafe` Requires YOU to Guarantee

Inside an `unsafe` block, Rust turns off some of these checks. The programmer must
manually ensure:

1. Pointer arithmetic stays in-bounds
2. Pointed-to memory is properly initialized and aligned
3. No aliased mutable references exist
4. FFI data types match the C ABI exactly
5. Invariants documented in `SAFETY` comments hold

### The Unsafe Audit Process

For safety-critical code (DO-178C DAL-B and above), every `unsafe` block needs:

1. A `// SAFETY:` comment explaining why the unsafe operation is valid
2. Review by a second engineer
3. Miri clean run (no undefined behavior detected)
4. Fuzz test for parsers / deserializers
5. Test coverage ≥ MC/DC criteria (see Day 9)

The goal is not to eliminate `unsafe` (sometimes impossible with hardware access),
but to **contain it** in small, well-reviewed functions with documented invariants.

---

## Privilege Separation Architecture

The TC receiver is the most attack-exposed component (it faces the uplink). We give
it the **fewest possible privileges**:

```
[tc_receiver]                    [obc_router]
 - uid: 2001 (tc_rx user)         - uid: 2002 (router user)
 - No capabilities                - CAP_SYS_NICE (for RT scheduling)
 - seccomp: net I/O only          - seccomp: IPC + timer syscalls
 - Can read: raw socket           - Can read/write: Unix socket
 - Cannot: open files, fork,      - Cannot: net I/O, raw sockets
   execve, mmap exec              
```

The Unix socket between them is the **security boundary**. The router:
- Validates the packet structure (APID, length fields)
- Rate-limits per-APID
- Routes to the correct subsystem handler

Even if an attacker finds a remote code execution bug in `tc_receiver`, they are
sandboxed: they can only send bytes to the router over the Unix socket, and the
router's own validation limits what harm those bytes can cause.

---

## References

- [Linux Capabilities man page](https://man7.org/linux/man-pages/man7/capabilities.7.html)
- [Seccomp BPF kernel docs](https://www.kernel.org/doc/html/latest/userspace-api/seccomp_filter.html)
- [ECSS-E-ST-70-41C: Telecommand Protocols](https://ecss.nl/standard/ecss-e-st-70-41c-space-engineering-space-packet-protocol/)
- NIST SP 800-38B: HMAC recommendations
- [subtle crate](https://docs.rs/subtle): constant-time cryptographic operations
