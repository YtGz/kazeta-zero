#!/bin/bash
# Detect available display servers (X11 and/or Wayland)

echo "=== Display Server Detection ==="

# Check session type
if [ -n "$XDG_SESSION_TYPE" ]; then
    echo "Session Type: $XDG_SESSION_TYPE"
else
    echo "Session Type: unknown"
fi

# Check for Wayland
WAYLAND_AVAILABLE=false
if [ -n "$WAYLAND_DISPLAY" ]; then
    echo "✓ Wayland available (WAYLAND_DISPLAY=$WAYLAND_DISPLAY)"
    WAYLAND_AVAILABLE=true
elif [ -S "$XDG_RUNTIME_DIR/wayland-0" ]; then
    echo "✓ Wayland socket exists at $XDG_RUNTIME_DIR/wayland-0"
    WAYLAND_AVAILABLE=true
else
    echo "✗ Wayland not available"
fi

# Check for X11
X11_AVAILABLE=false
if [ -n "$DISPLAY" ]; then
    echo "✓ X11 available (DISPLAY=$DISPLAY)"
    X11_AVAILABLE=true
    # Try to verify X11 is actually working
    if command -v xdpyinfo &> /dev/null; then
        if xdpyinfo &> /dev/null; then
            echo "  ✓ X11 server responding"
        else
            echo "  ✗ X11 server not responding"
            X11_AVAILABLE=false
        fi
    fi
else
    echo "✗ X11 not available (DISPLAY not set)"
fi

# Check for XWayland (X11 compatibility on Wayland)
if [ "$WAYLAND_AVAILABLE" = true ] && [ "$X11_AVAILABLE" = true ]; then
    echo "✓ XWayland detected (X11 apps supported on Wayland)"
fi

# Recommendation
echo ""
echo "=== Recommendation ==="
if [ "$X11_AVAILABLE" = true ]; then
    echo "Use X11 backend"
elif [ "$WAYLAND_AVAILABLE" = true ]; then
    echo "Use Wayland backend"
else
    echo "No display server detected - running headless?"
fi

# Exit codes:
# 0 = X11 available
# 1 = Wayland only
# 2 = Neither available
if [ "$X11_AVAILABLE" = true ]; then
    exit 0
elif [ "$WAYLAND_AVAILABLE" = true ]; then
    exit 1
else
    exit 2
fi
