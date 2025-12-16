#!/bin/bash
# Development run script for Kazeta+
# Starts the overlay daemon, input daemon, and BIOS together

set -e

cd "$(dirname "$0")"

echo "=== Kazeta+ Dev Runner ==="

# Clean up any previous instances
echo "[1/6] Cleaning up previous instances..."
pkill -f kazeta-overlay 2>/dev/null || true
pkill -f kazeta-input 2>/dev/null || true
pkill -f kazeta-bios 2>/dev/null || true
rm -f /tmp/kazeta-overlay.sock /tmp/kazeta-overlay.log
sleep 0.5

# Build overlay if needed
echo "[2/6] Building overlay..."
cd overlay
cargo build --bin kazeta-overlay --features daemon 2>&1 | grep -E "(Compiling|Finished|error)" || true
cd ..

# Build RetroAchievements CLI if needed
echo "[3/6] Building RetroAchievements CLI (kazeta-ra)..."
cd ra
cargo build 2>&1 | grep -E "(Compiling|Finished|error)" || true
cd ..

# Build input daemon if needed
echo "[4/6] Building input daemon (kazeta-input)..."
cd input-daemon
cargo build 2>&1 | grep -E "(Compiling|Finished|error)" || true
cd ..

# Build BIOS if needed  
echo "[5/6] Building BIOS..."
cd bios
cargo build --features dev 2>&1 | grep -E "(Compiling|Finished|error)" || true
cd ..

# Start overlay in background
echo "[6/6] Starting overlay daemon..."
./overlay/target/debug/kazeta-overlay > /tmp/kazeta-overlay.log 2>&1 &
OVERLAY_PID=$!
sleep 1

# Verify overlay started
if [ -S /tmp/kazeta-overlay.sock ]; then
    echo "✓ Overlay daemon running (PID: $OVERLAY_PID)"
else
    echo "✗ Overlay failed to start. Check /tmp/kazeta-overlay.log"
    cat /tmp/kazeta-overlay.log
    exit 1
fi

# Start input daemon in background (Linux only, will fail gracefully on macOS)
echo "Starting input daemon..."
./input-daemon/target/debug/kazeta-input > /tmp/kazeta-input.log 2>&1 &
INPUT_PID=$!
sleep 0.5

# Check if input daemon started (it may fail on macOS, which is OK)
if kill -0 $INPUT_PID 2>/dev/null; then
    echo "✓ Input daemon running (PID: $INPUT_PID)"
else
    echo "⚠ Input daemon not running (this is OK on macOS - overlay has built-in input handling)"
fi

echo ""
echo "=== Starting BIOS ==="
echo "Controls:"
echo "  • F12 or Ctrl+O: Toggle overlay"
echo "  • Guide button: Toggle overlay (controller)"
echo ""

# Run BIOS in foreground with PATH updated to include kazeta-ra
cd bios
export PATH="$(cd .. && pwd)/ra/target/debug:$PATH"
cargo run --features dev

# Cleanup on exit
echo ""
echo "Shutting down services..."
kill $OVERLAY_PID 2>/dev/null || true
kill $INPUT_PID 2>/dev/null || true
rm -f /tmp/kazeta-overlay.sock
echo "Done."

