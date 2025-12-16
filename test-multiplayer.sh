#!/bin/bash
# Test multiplayer VBA-M setup
#
# VBA-M multiplayer uses Local IPC (shared memory) for linking.
# Multiple separate VBA-M processes can communicate through shared state.

set -e

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"

echo "🎮 Testing VBA-M Multiplayer Support"
echo "====================================="
echo ""
echo "ℹ️  How VBA-M Multiplayer Works:"
echo "   - Each player runs as a separate VBA-M process"
echo "   - Processes communicate via Local IPC (shared memory)"
echo "   - GBA/LinkType=1 enables link cable"
echo "   - GBA/LinkProto=1 enables local IPC mode"
echo "   - Player 1 uses --delete-shared-state to clear stale state"
echo ""

# Check for VBA-M
VBA_BIN=""
if [ "$(uname)" = "Darwin" ]; then
    # macOS
    if [ -f "/Applications/visualboyadvance-m.app/Contents/MacOS/visualboyadvance-m" ]; then
        VBA_BIN="/Applications/visualboyadvance-m.app/Contents/MacOS/visualboyadvance-m"
    elif [ -f "$HOME/Applications/visualboyadvance-m.app/Contents/MacOS/visualboyadvance-m" ]; then
        VBA_BIN="$HOME/Applications/visualboyadvance-m.app/Contents/MacOS/visualboyadvance-m"
    elif [ -f "/tmp/visualboyadvance-m.app/Contents/MacOS/visualboyadvance-m" ]; then
        VBA_BIN="/tmp/visualboyadvance-m.app/Contents/MacOS/visualboyadvance-m"
    fi
else
    # Linux
    if command -v visualboyadvance-m &> /dev/null; then
        VBA_BIN="visualboyadvance-m"
    fi
fi

if [ -z "$VBA_BIN" ]; then
    echo "❌ VBA-M not found!"
    echo ""
    echo "   To install VBA-M:"
    echo "   macOS: Download from https://github.com/visualboyadvance-m/visualboyadvance-m/releases"
    echo "          Extract to /Applications/visualboyadvance-m.app"
    echo "   Linux: pacman -S vbam-wx"
    echo ""
    echo "   Quick test install (macOS ARM64):"
    echo "   cd /tmp && curl -LO https://github.com/visualboyadvance-m/visualboyadvance-m/releases/download/v2.2.3/visualboyadvance-m-Mac-ARM64.zip && unzip -o visualboyadvance-m-Mac-ARM64.zip"
    exit 1
fi

echo "✅ Found VBA-M: $VBA_BIN"

# Check for test ROM
ROM_PATH="${1:-$SCRIPT_DIR/test_game/pokemon.gba}"
if [ ! -f "$ROM_PATH" ]; then
    echo "❌ Test ROM not found: $ROM_PATH"
    echo "   Provide a GBA ROM file as argument or place one at ./test_game/pokemon.gba"
    exit 1
fi

echo "✅ Using ROM: $ROM_PATH"

# Set up test environment
export VBA_MULTIPLAYER=true
export VBA_PLAYERS=2

echo ""
echo "🚀 Launching 2-player multiplayer session via wrapper script..."
echo ""

# Run the wrapper script
bash "$SCRIPT_DIR/runtimes/gba/vba-run-wrapper.sh" "$ROM_PATH" "test-multiplayer"

echo ""
echo "✅ Multiplayer session ended."
