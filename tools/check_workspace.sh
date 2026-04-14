#!/usr/bin/env bash
# Run all CI checks locally before pushing.
# chmod +x tools/check_workspace.sh

set -euo pipefail
cd "$(dirname "$0")/.."

echo "==================================================="
echo "  Workspace check"
echo "==================================================="

echo ""
echo "--- cargo fmt --check ---"
cargo fmt --all -- --check

echo ""
echo "--- cargo clippy ---"
cargo clippy --workspace --all-targets --all-features -- -D warnings

echo ""
echo "--- cargo test ---"
cargo test --workspace --all-features

echo ""
echo "==================================================="
echo "  All checks passed!"
echo "==================================================="
