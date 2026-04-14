# Tools

Helper scripts for working with the training workspace.

---

## `run_obc_stack.sh`

Starts the full Week 2 OBC stack in one command.

```bash
bash tools/run_obc_stack.sh
```

**What it does:**
1. Runs `cargo build --workspace -q` to ensure all binaries are up to date
2. Launches `tc-receiver`, `obc-router`, `hk-service`, and `sensor-daemon` in the background
3. Waits 1 second for daemons to bind their sockets
4. Runs `ground-sim` as the test harness
5. On exit (Ctrl+C or `ground-sim` completion), kills all background processes and removes socket files

**Prerequisites:**
- Must be run from the repo root or the `tools/` directory (the script `cd`s to the repo root)
- Workspace must compile cleanly (`cargo build --workspace`)

---

## `check_workspace.sh`

Runs all CI checks locally before pushing.

```bash
bash tools/check_workspace.sh
```

**What it checks (in order):**
1. `cargo fmt --all -- --check` — formatting (matches CI)
2. `cargo clippy --workspace --all-targets --all-features -- -D warnings` — lints
3. `cargo test --workspace --all-features` — all unit and integration tests

If any step fails the script exits immediately (`set -euo pipefail`).

**Tip:** run this before every `git push` to catch issues locally. The CI workflow
(`.github/workflows/ci.yml`) runs exactly the same checks.

---

## Make the scripts executable

If you get `Permission denied`:
```bash
chmod +x tools/run_obc_stack.sh tools/check_workspace.sh
```
