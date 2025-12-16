# Proposed New Features for Kazeta++

## Overlay Features

### 1. Screenshot Capture
**Description:** Allow users to capture screenshots via overlay hotkey
**Implementation:**
- Add F11 or Guide+A as screenshot hotkey
- Use macroquad's `get_screen_data()` to capture framebuffer
- Save to `~/.local/share/kazeta-plus/screenshots/` with timestamp
- Show toast confirmation with thumbnail preview

**Files to modify:**
- `overlay/src/input.rs` - Add screenshot hotkey detection
- `overlay/src/state.rs` - Add screenshot capture logic
- `overlay/src/rendering.rs` - Add thumbnail toast rendering

### 2. Achievement Progress Bar in Performance HUD
**Description:** Show mini achievement progress while playing
**Implementation:**
- Add compact progress bar (e.g., "🏆 5/20") to performance HUD
- Update on RaProgressUpdate messages
- Toggle visibility independently from full overlay

**Files to modify:**
- `overlay/src/performance.rs` - Add achievement tracking
- `overlay/src/rendering.rs` - Add `render_achievement_mini_hud()`

### 3. Controller Battery Level Display
**Description:** Show wireless controller battery in overlay
**Implementation:**
- gilrs doesn't expose battery, need upower/dbus integration
- Poll battery levels periodically (every 30s)
- Show warning toast when battery < 20%
- Display in Controllers menu

**Files to modify:**
- `overlay/src/controllers.rs` - Add battery monitoring
- Add new dependency: `dbus` or `upower_dbus`

### 4. Quick Menu Customization
**Description:** Let users reorder/hide main menu items
**Implementation:**
- Add config file for menu preferences
- Support hiding unused options (e.g., Achievements if not logged in)
- Drag-and-drop reordering in Settings

### 5. Overlay Themes
**Description:** Multiple color themes for overlay UI
**Implementation:**
- Define theme struct with colors (background, accent, text)
- Ship with presets (Dark, Light, RetroGreen, PS-style, Xbox-style)
- Theme selection in Settings menu
- Already have `SetTheme` IPC message, extend it

---

## Input-Daemon Features

### 1. Configurable Hotkey Mapping
**Description:** Allow users to customize overlay toggle key
**Implementation:**
- Read hotkey config from `~/.config/kazeta-plus/input.toml`
- Support multiple hotkey definitions
- Example config:
  ```toml
  [[hotkeys]]
  action = "toggle_overlay"
  keys = ["BTN_MODE"]  # Guide button

  [[hotkeys]]
  action = "toggle_overlay"
  keys = ["KEY_LEFTCTRL", "KEY_O"]  # Ctrl+O

  [[hotkeys]]
  action = "screenshot"
  keys = ["KEY_F11"]
  ```

**Files to modify:**
- `input-daemon/src/main.rs` - Add config loading
- Add new file: `input-daemon/src/config.rs`
- Add dependency: `toml`

### 2. Long-Press Actions
**Description:** Different actions for short vs long press
**Implementation:**
- Track press duration for configurable buttons
- Short press (<500ms): Toggle overlay
- Long press (>500ms): Quick save or screenshot
- Vibration feedback on long-press activation

### 3. Controller Rumble on Hotkey
**Description:** Brief vibration when overlay activated
**Implementation:**
- Use gilrs force feedback API
- Short pulse (100ms) on overlay toggle
- Configurable intensity or disable option

### 4. Input Recording/Playback (TAS-lite)
**Description:** Record and replay input sequences
**Implementation:**
- Record button presses with timestamps
- Save/load input recordings
- Playback with frame-accurate timing
- Useful for speedrun practice

---

## RetroAchievements Features

### 1. Leaderboard Support
**Description:** Display and submit to RA leaderboards
**Implementation:**
- Add `API_GetLeaderboard.php` endpoint support
- Show leaderboards in overlay achievements screen
- Submit scores when emulator reports them

**API Endpoints:**
- `API_GetGameLeaderboards.php` - List leaderboards for game
- `API_GetLeaderboardEntries.php` - Get scores

**Files to modify:**
- `ra/src/api.rs` - Add leaderboard methods
- `ra/src/types.rs` - Add Leaderboard types
- `overlay/src/state.rs` - Add leaderboard display state

### 2. Rich Presence Integration
**Description:** Show what user is doing in-game
**Implementation:**
- RA Rich Presence is game-specific scripts
- Parse rich presence definition from API
- Emulator must report memory values
- Display in overlay and optionally Discord

### 3. Achievement Progress Notifications
**Description:** Toast when making progress on achievements
**Implementation:**
- Some achievements have progress (e.g., "Collect 50 coins: 35/50")
- Show toast when crossing milestones (25%, 50%, 75%)
- Requires emulator memory integration

### 4. Offline Achievement Queue
**Description:** Queue unlocks when offline, submit later
**Implementation:**
- Save pending unlocks to SQLite
- Retry submission on network availability
- Show pending count in overlay

**Files to modify:**
- `ra/src/cache.rs` - Add pending_unlocks table
- `ra/src/api.rs` - Add queue/retry logic

### 5. Badge Image Caching
**Description:** Download and cache achievement badge images
**Implementation:**
- Download badges on game start
- Store in `~/.cache/kazeta-plus/badges/`
- Display in overlay achievement list
- Use placeholder for missing images

**Files to modify:**
- `ra/src/cache.rs` - Add badge file caching
- `overlay/src/state.rs` - Add Texture2D loading
- `overlay/src/rendering.rs` - Render badge images

### 6. Auto-Detect Console Type
**Description:** Determine console from ROM file
**Implementation:**
- Check file extension (.gba, .nes, .sfc, etc.)
- Verify with magic bytes
- Remove need for `--console` parameter

**Files to modify:**
- `ra/src/hash.rs` - Add `detect_console()` function

---

## Cross-Component Features

### 1. Discord Rich Presence
**Description:** Show game info in Discord status
**Implementation:**
- Use discord-rpc crate
- Show: Game title, achievement progress, play time
- Update on game start/achievement unlock
- Configurable enable/disable

### 2. Steam Deck Integration
**Description:** Better support for handheld PC gaming
**Implementation:**
- Detect Steam Deck via `/sys/devices/virtual/dmi/id/`
- Default overlay position optimized for 800p
- Quick access game mode mappings
- Performance profiles for battery/plugged

### 3. Save State Thumbnails
**Description:** Preview images for save states
**Implementation:**
- Capture screenshot when saving
- Scale down to thumbnail (160x120)
- Display in Quick Save menu
- Store alongside save files

### 4. Cloud Save Sync
**Description:** Sync saves across devices
**Implementation:**
- Support multiple backends (Dropbox, Google Drive, custom)
- Background sync with conflict resolution
- Manual sync trigger in overlay
- Show sync status in menu

---

## Implementation Priority

### Phase 1 (Quick Wins)
- [ ] Screenshot capture
- [ ] Toast queue limit
- [ ] Auto-detect console type
- [ ] Configurable hotkeys

### Phase 2 (Core Enhancements)
- [ ] Achievement badge images
- [ ] Offline achievement queue
- [ ] Leaderboard support
- [ ] Battery level display

### Phase 3 (Polish)
- [ ] Overlay themes
- [ ] Discord Rich Presence
- [ ] Save state thumbnails
- [ ] Long-press actions

### Phase 4 (Advanced)
- [ ] Rich Presence integration
- [ ] Cloud save sync
- [ ] Input recording
- [ ] Steam Deck optimization
