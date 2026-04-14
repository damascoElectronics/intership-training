#!/usr/bin/env bash
# Starts all OBC daemons in the background, then runs ground-sim.
# Press Ctrl+C to stop everything.
# chmod +x tools/run_obc_stack.sh

set -euo pipefail
cd "$(dirname "$0")/.."

cleanup() {
    echo ""
    echo "Stopping OBC stack..."
    kill $(jobs -p) 2>/dev/null || true
    rm -f /tmp/obc_*.sock /tmp/obc_tc_uplink.sock
}
trap cleanup EXIT INT TERM

echo "Building workspace..."
cargo build --workspace -q 2>&1 | grep -v "^$" || true

echo ""
echo "Starting OBC stack..."

./target/debug/tc-receiver &
echo "  [started] tc-receiver (pid $!)"
sleep 0.3

./target/debug/obc-router &
echo "  [started] obc-router (pid $!)"
sleep 0.3

./target/debug/hk-service &
echo "  [started] hk-service (pid $!)"

./target/debug/sensor-daemon &
echo "  [started] sensor-daemon (pid $!)"

sleep 1.0
echo ""
echo "Running ground-sim test harness..."
echo "==========================================="
./target/debug/ground-sim
echo "==========================================="
