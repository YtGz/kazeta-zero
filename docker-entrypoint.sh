#!/bin/bash
# Docker entrypoint for running Kazeta+ BIOS with virtual display

set -e

echo "🖥️  Starting virtual display (Xvfb)..."
Xvfb :99 -screen 0 1280x720x24 -ac +extension GLX +render -noreset &
XVFB_PID=$!
export DISPLAY=:99

# Give Xvfb time to start
sleep 2

echo "📺 Starting VNC server..."
x11vnc -display :99 -nopw -listen 0.0.0.0 -forever -shared &
VNC_PID=$!

echo "🎮 Starting Kazeta+ BIOS..."
echo "   VNC available at: localhost:5900"
echo "   Use VNC viewer to connect and see the display"
echo ""

# Run the BIOS
cd /workspace/bios
exec cargo run

# Cleanup on exit
trap "kill $XVFB_PID $VNC_PID 2>/dev/null || true" EXIT
