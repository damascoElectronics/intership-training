# Day 9 — Verification & Testing

**Theme:** Property-based testing, state machine invariants, and memory safety with Miri.

Unit tests check specific cases. Property-based tests check *all* cases — the framework
generates thousands of random inputs and searches for counterexamples. For safety-critical
embedded software this is the difference between "it worked in our tests" and "it works".

---

## Learning Goals

- Write `proptest!` properties for CCSDS packet round-trips
- Model a state machine as a property (any valid sequence of events must obey the invariants)
- Test a codec (COBS) with encode/decode round-trip and boundary properties
- Understand what Miri checks and how to structure unsafe code to survive it

---

## Examples

| File | What it demonstrates |
|------|----------------------|
| `01_proptest_packet.rs` | Round-trip APID, sequence count, full packet; no-panic on arbitrary bytes |
| `02_proptest_state_machine.rs` | Health event strategy, `failed_requires_manual_reset` invariant |
| `03_property_tests.rs` | COBS encode/decode: `encoded_has_no_zeros`, `roundtrip`, `length_bound` |
| `04_miri_safety.rs` | `MaybeUninit`, pointer provenance; commented UB examples with explanations |

Run all tests (includes proptest):
```
cargo test -p day9-verification
```

Run Miri on the safety example (requires nightly + `cargo +nightly miri`):
```
cargo +nightly miri test -p day9-verification
```

---

## Exercises

### Exercise 1 — Find the COBS Bug (`ex1_proptest_codec.rs`)

The file contains a deliberately buggy COBS encoder. Your task:

1. Write a `roundtrip` proptest property: `decode(encode(input)) == input`
2. Write a `no_zeros` property: encoded output never contains `0x00`
3. Run `cargo test` — proptest will find a failing case
4. Fix the encoder
5. Add a regression test for the specific input that failed

**Hint:** the bug manifests only when a run of non-zero bytes is exactly 254 bytes long.

Solution: `ex1_proptest_codec_sol.rs`

---

## Key Concepts

### What proptest does

```rust
proptest! {
    #[test]
    fn roundtrip(input in any::<Vec<u8>>()) {
        let encoded = encode(&input);
        let decoded = decode(&encoded).unwrap();
        prop_assert_eq!(decoded, input);
    }
}
```

The macro generates 256 random `Vec<u8>` values (configurable). When it finds a failure it
*shrinks* — finds the smallest failing case — and reports that. "Input length 254, all bytes
= 0xFF" is more useful than "some 10 000-byte buffer".

### COBS (Consistent Overhead Byte Stuffing)

COBS is a serial framing codec that eliminates `0x00` from encoded data. This allows `0x00`
to be used as a packet delimiter on a byte stream, giving unambiguous framing without escaping.

Encoding rules:
- Replace each `0x00` with a forward pointer to the next `0x00` (or end-of-frame)
- Maximum overhead: 1 byte per 254 payload bytes (one overhead byte at start, one per 254 block)

### Miri

Miri is an interpreter for Rust MIR that detects:
- Use of uninitialised memory
- Out-of-bounds memory accesses
- Pointer aliasing violations (stacked borrows)
- Invalid values in typed positions

Run it with `cargo +nightly miri test`. It is slow (10–100× real execution) but finds bugs
that sanitizers miss. Ideal for running on the `unsafe` modules of your codebase.

### When proptest finds a bug

1. Look at the **shrunk input** — it is the minimal counterexample
2. Add it as a `#[test]` regression case immediately (before fixing the bug)
3. Fix the bug
4. Confirm both the regression test and proptest pass

This workflow is called *test-driven debugging*.
