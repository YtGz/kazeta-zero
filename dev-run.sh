#!/bin/bash
# Development run script for Kazeta+
# Starts the overlay daemon and BIOS together

set -e

cd "$(dirname "$0")"

echo "=== Kazeta+ Dev Runner ==="

# Clean up any previous instances
echo "[1/4] Cleaning up previous instances..."
pkill -f kazeta-overlay 2>/dev/null || true
pkill -f kazeta-bios 2>/dev/null || true
rm -f /tmp/kazeta-overlay.sock /tmp/kazeta-overlay.log
sleep 0.5

# Build overlay if needed
echo "[2/4] Building overlay..."
cd overlay
cargo build --bin kazeta-overlay --features daemon 2>&1 | grep -E "(Compiling|Finished|error)" || true
cd ..

# Build BIOS if needed  
echo "[3/4] Building BIOS..."
cd bios
cargo build --features dev 2>&1 | grep -E "(Compiling|Finished|error)" || true
cd ..

# Start overlay in background
echo "[4/4] Starting overlay daemon..."
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

echo ""
echo "=== Starting BIOS ==="
echo "Controls:"
echo "  • F12 or Ctrl+O: Toggle overlay"
echo "  • Guide button: Toggle overlay (controller)"
echo ""

# Run BIOS in foreground
cd bios
cargo run --features dev

# Cleanup on exit
echo ""
echo "Shutting down overlay..."
kill $OVERLAY_PID 2>/dev/null || true
rm -f /tmp/kazeta-overlay.sock
echo "Done."

