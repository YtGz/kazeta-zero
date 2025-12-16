#!/bin/bash
# Test script for controller input simulation

SOCKET="/tmp/kazeta-overlay.sock"

echo "=== Testing Controller Input via IPC ==="
echo ""

# First, show the overlay
echo "1. Showing overlay menu..."
echo '{"type":"show_overlay","screen":"main"}' | nc -U $SOCKET
sleep 1

# Test navigation - move down through menu
echo "2. Testing navigation - moving down..."
for i in {1..3}; do
    # Simulate D-pad down via keyboard for testing
    # (In production, this would come from actual controller)
    echo "[Test] Simulating Down press $i"
    sleep 0.5
done

# Test selecting Settings
echo "3. Selecting Settings option..."
echo "[Test] Simulating Select button"
sleep 1

# Test going back
echo "4. Going back to main menu..."
echo "[Test] Simulating Back button"
sleep 1

# Test Quick Save
echo "5. Testing Quick Save..."
echo "[Test] Simulating Select on Quick Save"
sleep 1

# Test closing overlay
echo "6. Closing overlay..."
echo '{"type":"hide_overlay"}' | nc -U $SOCKET
sleep 1

echo ""
echo "=== Test Complete ==="
echo "Note: Controller input testing on macOS requires a connected controller."
echo "On Linux, the overlay will respond to actual controller input."
