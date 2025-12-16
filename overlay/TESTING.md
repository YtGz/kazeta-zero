# Testing Overlay Locally

## Problem
When running BIOS and overlay locally, they appear as two separate windows, making it hard to see how they'll work in production where the overlay appears on top of the game.

## Solution

### macOS
The overlay window is automatically configured to:
- Match BIOS window size (640x360)
- Be always-on-top (floating window level)
- Have transparent background
- Appear above the BIOS window

### Linux
For Linux, you may need to manually set the overlay window to always-on-top using your window manager:

```bash
# Using wmctrl (if available)
wmctrl -r "Kazeta Overlay" -b add,above

# Or use your window manager's "Always on Top" feature
```

## Testing Steps

1. **Build the overlay:**
   ```bash
   cd overlay
   cargo build --bin kazeta-overlay --features daemon
   ```

2. **Start the BIOS:**
   ```bash
   cd bios
   cargo run --features dev
   ```

3. **In another terminal, start the overlay:**
   ```bash
   cd overlay
   cargo run --bin kazeta-overlay --features daemon
   ```

4. **Position windows:**
   - On macOS: Overlay should automatically appear above BIOS
   - On Linux: Manually set overlay to always-on-top using your window manager

5. **Test the overlay:**
   - Press F12 or Guide button to toggle overlay visibility
   - The overlay should appear on top of the BIOS window
   - When hidden, you should see the BIOS window underneath

## Visual Result

When working correctly:
- Overlay window appears **on top** of BIOS window
- Overlay has transparent background (you can see BIOS through it when overlay menu is hidden)
- Overlay menu appears with semi-transparent dark background
- Both windows are the same size (640x360) and aligned

## Troubleshooting

### Overlay doesn't appear on top
- **macOS**: Check console for window property setting messages
- **Linux**: Use `wmctrl` or window manager settings to set always-on-top

### Can't see BIOS through overlay
- Overlay should use `clear_background(BLANK)` for transparency
- Check that `framebuffer_alpha: true` is set in window config

### Windows are different sizes
- Both should be 640x360 (BIOS default size)
- Overlay window config matches BIOS window size

