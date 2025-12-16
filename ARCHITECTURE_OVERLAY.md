# Overlay Architecture & BIOS Integration

## Current Architecture

### Process Flow

```
1. System Boot
   └─> kazeta-session starts
       └─> BIOS launches (kazeta-bios)
           └─> User selects game
               └─> BIOS exits (creates .RESTART_SESSION_SENTINEL)
                   └─> kazeta-session detects BIOS exit
                       └─> Launches game via /usr/bin/kazeta
                           └─> Overlay daemon starts (kazeta-overlay)
                               └─> Game runs with overlay available
                                   └─> Game exits
                                       └─> Overlay daemon stops
                                           └─> kazeta-session restarts
                                               └─> BIOS launches again
```

### Key Points

1. **BIOS and Overlay Never Run Simultaneously**
   - BIOS exits completely before game launches
   - Overlay starts when game launches
   - No window/focus conflicts in production

2. **Separate Processes**
   - BIOS: `kazeta-bios` (macroquad application)
   - Overlay: `kazeta-overlay` (separate macroquad application)
   - Game: Runs in gamescope

3. **Communication**
   - IPC via Unix socket: `/tmp/kazeta-overlay.sock`
   - BIOS can send messages to overlay (toasts, achievements)
   - Overlay handles its own input (Guide button, controller)

## Why Keep Overlay Separate?

### ✅ Advantages

1. **Modularity**: Overlay can be updated independently
2. **Survivability**: Overlay survives game crashes
3. **Resource Isolation**: Game crashes don't kill overlay
4. **Clean Separation**: BIOS doesn't need overlay code
5. **Window Management**: Overlay can be always-on-top, transparent
6. **Input Handling**: Overlay uses evdev directly (not through BIOS)

### ❌ Disadvantages

1. **Two Macroquad Processes**: More memory usage
2. **IPC Overhead**: Socket communication (minimal)
3. **Deployment**: Need to install both binaries

## Potential Issues & Solutions

### Issue 1: Window Layering
**Problem**: Overlay must appear above game window

**Solution**: 
- Overlay uses macroquad with transparent background
- Window manager should set overlay as always-on-top
- On Linux: Use X11/Wayland window properties
- Overlay renders with `clear_background(BLANK)` for transparency

### Issue 2: Input Capture
**Problem**: Overlay must capture Guide button even when game has focus

**Solution**:
- Overlay uses `evdev` directly (not through macroquad input)
- Bypasses window focus requirements
- Guide button (BTN_MODE) is captured at kernel level

### Issue 3: Dev Mode Conflicts
**Problem**: In dev mode, BIOS might keep running while testing overlay

**Solution**:
- ✅ **FIXED**: Removed overlay startup from BIOS initialization
- Overlay only starts when games launch
- In dev mode, launch a game to test overlay

## Should Overlay Be Built Into BIOS?

### Recommendation: **NO** (Keep Separate)

**Reasons:**

1. **Architecture Mismatch**
   - BIOS exits before games launch
   - Overlay needs to run during gameplay
   - Building into BIOS would require BIOS to stay alive (defeats session restart)

2. **Window Management**
   - Overlay needs to be always-on-top of game
   - Separate process = better window control
   - Can use window manager properties independently

3. **Input Handling**
   - Overlay uses evdev for Guide button (bypasses focus)
   - BIOS uses gilrs/macroquad input
   - Different input requirements

4. **Resource Management**
   - Game crashes shouldn't kill overlay
   - Separate process = better isolation
   - Overlay can survive and show error messages

5. **Development**
   - Easier to test overlay independently
   - Can run overlay standalone for debugging
   - Modular codebase

### Alternative: Hybrid Approach (Not Recommended)

If you wanted overlay in BIOS:
- BIOS would need to stay running during gameplay
- Would need to render overlay UI on top of game window
- Complex window compositing
- Defeats the clean session restart architecture
- More complex code

## Production Deployment

### Installation

Both binaries need to be installed:
```bash
# BIOS
/usr/bin/kazeta-bios

# Overlay daemon
/usr/bin/kazeta-overlay
```

### Startup Sequence

1. Game launches via `/usr/bin/kazeta`
2. Script starts overlay daemon:
   ```bash
   kazeta-overlay > /dev/null 2>&1 &
   ```
3. Game runs in gamescope
4. Overlay renders on top (transparent window)
5. When game exits, overlay is killed via trap

### Window Management

For proper overlay behavior, ensure:
- Overlay window is always-on-top
- Overlay window is transparent
- Overlay window doesn't steal focus from game
- Window manager respects overlay's z-order

## Testing

### Dev Mode

1. Build overlay:
   ```bash
   cd overlay && cargo build --bin kazeta-overlay --features daemon
   ```

2. Launch a game (overlay starts automatically)

3. Press Guide button or F12 to test overlay

### Standalone Testing

```bash
# Terminal 1: Start overlay
cargo run --bin kazeta-overlay --features daemon

# Terminal 2: Send test message
echo '{"type":"show_overlay","screen":"main"}' | nc -U /tmp/kazeta-overlay.sock
```

## Summary

**Current architecture is correct:**
- ✅ No conflicts (BIOS exits before overlay starts)
- ✅ Clean separation of concerns
- ✅ Overlay survives game crashes
- ✅ Modular and maintainable

**No need to build overlay into BIOS:**
- Would complicate architecture
- Defeats session restart design
- Window management harder
- Less modular

**Ensure proper window management:**
- Overlay must be always-on-top
- Transparent background
- Direct input capture (evdev)

