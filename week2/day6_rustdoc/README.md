# Day 6: Rustdoc & API Documentation

## Why Documentation Matters in Aerospace

In consumer software, poor docs are inconvenient. In aerospace, they can be mission-critical:

- **Your API is a contract.** The attitude control team writing code against your packet codec has no
  idea what assumptions you made unless you write them down. A wrong assumption means a lost packet
  — or a lost spacecraft.
- **Docs ARE part of the product.** ESA, NASA, and JPL software standards (e.g., JPL C Coding
  Standard, ECSS-E-ST-40C) all require formal documentation of interfaces. Rust's rustdoc makes this
  executable and verifiable.
- **The reviewer is not you.** Flight software reviews involve multiple teams, safety boards, and
  sometimes regulators. They will read your docs, not your source code.
- **Code review latency is high.** On a spacecraft project, a question like "what happens if seq_count
  overflows?" can take days to answer through formal channels. Good docs prevent the question.

The rule: **if a public item has no doc comment, it does not exist as far as other teams are concerned.**

---

## Rustdoc Architecture

Rustdoc is the official Rust documentation tool, built into the Rust toolchain. It works by:

1. **Parsing doc comments** (`///` and `//!`) as Markdown.
2. **Generating HTML** from those Markdown comments, cross-linking types and items automatically.
3. **Extracting code blocks** (` ```rust ` blocks) from doc comments and compiling them as tests.

This means documentation and tests are unified. A doc comment that describes "here is an example"
becomes a test that verifies the example is correct.

```
source code → rustdoc parser → Markdown → HTML docs
                            ↘ code blocks → cargo test
```

---

## Outer vs Inner Doc Comments

```rust
//! This is an INNER doc comment — documents the item it is INSIDE.
//! Used at the top of a file to document the module/crate.
//! Applies to: lib.rs (crate), mod.rs (module), top of a file.

/// This is an OUTER doc comment — documents the item BELOW it.
/// Used immediately before a struct, fn, enum, trait, const, etc.
pub struct MyType { ... }
```

**Rule of thumb:**
- `//!` at the top of `lib.rs` → documents the crate
- `//!` at the top of `foo.rs` → documents the `foo` module
- `///` before any `pub` item → documents that item

---

## Standard Doc Comment Sections

A well-structured doc comment follows this pattern:

```rust
/// One-line summary. This is what appears in search results and hover tooltips.
///
/// Optional longer description. Explain WHY, not just WHAT. Describe invariants,
/// trade-offs, and decisions that aren't obvious from the type signature.
///
/// # Examples
///
/// At least one runnable example. These are compiled and run by `cargo test`.
///
/// ```rust
/// let x = MyType::new(42)?;
/// assert_eq!(x.value(), 42);
/// # Ok::<(), MyError>(())  // hidden line: makes the ? operator work in doc test
/// ```
///
/// # Panics
///
/// Document when this function panics. If it never panics, say so.
/// In safety-critical code, panics are usually forbidden — but the *promise*
/// of "never panics" must be documented and verified.
///
/// # Errors
///
/// Document every error variant that can be returned. Link to the error type.
/// This is the most important section for functions returning `Result`.
///
/// Returns [`MyError::InvalidInput`] if the value is out of range.
///
/// # Safety
///
/// REQUIRED for `unsafe fn`. Describe every precondition the caller must uphold.
/// Failure to meet these preconditions is undefined behavior.
pub fn documented_function(value: u32) -> Result<MyType, MyError> { ... }
```

---

## Doc Tests: Examples That Compile and Run

Every ` ```rust ` block in a doc comment is a test case:

```rust
/// Parses a big-endian u16 from a byte slice.
///
/// # Examples
/// ```
/// use my_crate::parse_u16_be;
/// let bytes = [0x01, 0x02];
/// assert_eq!(parse_u16_be(&bytes), 0x0102);
/// ```
pub fn parse_u16_be(bytes: &[u8]) -> u16 { ... }
```

Run them with: `cargo test --doc`

### Hidden Lines

Lines starting with `# ` are hidden in the HTML but included in the test:

```rust
/// ```
/// let result = fallible_fn()?;
/// assert_eq!(result, 42);
/// # Ok::<(), MyError>(())   // ← hidden; makes ? valid at the top level
/// ```
```

### Marked Non-Runnable

Use ` ```rust,no_run ` when the code requires external resources (hardware, network):

```rust
/// ```rust,no_run
/// // This would open a real serial port — can't run in CI
/// let port = UartDriver::open("/dev/ttyS0")?;
/// # Ok::<(), std::io::Error>(())
/// ```
```

Use ` ```rust,compile_fail ` to document that something is intentionally a compile error:

```rust
/// ```rust,compile_fail
/// // This must NOT compile — the APID is too large
/// let hdr = CcsdsPrimaryHeader::new_tc(0xFFFF, 0, 0);
/// ```
```

---

## Intra-Doc Links

Rustdoc auto-resolves links to other items in the same crate (or dependencies):

```rust
/// Returns a [`CcsdsFrame`] or [`CcsdsError::InvalidApid`].
///
/// See also: [`codec::encode`] and [`codec::decode`].
```

Use backtick-bracket syntax: `` [`TypeName`] ``, `` [`module::function`] ``.

These are checked at compile time — a broken link is a warning (or error with `RUSTDOCFLAGS`).

---

## `#[doc(hidden)]`

Marks an item so it does NOT appear in the generated documentation:

```rust
#[doc(hidden)]
pub fn internal_helper() { ... }  // public for macro use, but not part of the API
```

Use sparingly. If something is truly internal, make it `pub(crate)` instead.

---

## `#![deny(missing_docs)]`

The most important doc lint for library crates. Add it at the top of `lib.rs`:

```rust
#![deny(missing_docs)]
```

This makes the compiler **refuse to build** if any public item lacks a doc comment.

In CI, this means undocumented APIs are a build failure, not a code review note.

---

## Building and Viewing Docs

```bash
# Build docs for this crate only (no dependency docs — faster)
cargo doc --no-deps

# Build and open in browser
cargo doc --no-deps --open

# Build for a specific crate in a workspace
cargo doc -p day6-rustdoc --no-deps --open

# Run doc tests only
cargo test --doc

# Run all tests (unit + integration + doc tests)
cargo test
```

---

## CI: Treat Doc Warnings as Errors

In a real project's CI pipeline, set:

```bash
RUSTDOCFLAGS="-D warnings" cargo doc --no-deps
```

This makes doc warnings (broken intra-doc links, malformed Markdown) into build failures.

Combined with `#![deny(missing_docs)]`, this gives you:
- Every public item documented: enforced by compiler
- All doc tests passing: enforced by `cargo test --doc`
- No broken links: enforced by `RUSTDOCFLAGS="-D warnings"`

---

## How Good Docs Connect to the Job

Aerospace APIs are consumed by teams who:
- Are in a different building, city, or country
- Cannot interrupt you with questions during integration phases
- Will use your code years after you've moved to another project
- Must justify every interface decision to a safety review board

Your doc comment is the closest they will get to asking you a question and getting an answer.
Write it as if you are explaining to a competent engineer who has never seen this codebase
and cannot contact you.

**Checklist for every public `fn`:**
- [ ] One-line summary (imperative mood: "Creates a...", "Returns the...", "Parses a...")
- [ ] At least one `# Examples` block that compiles and runs
- [ ] `# Errors` section listing every `Err` variant
- [ ] `# Panics` section (or explicit statement that it never panics)
- [ ] `# Safety` section if the function is `unsafe`
- [ ] Intra-doc links for all types mentioned in prose
