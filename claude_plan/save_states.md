# Quick Resume & Save States Implementation Plan

## Overview

Add quick resume functionality to Kazeta-plus that allows users to save game state when swapping between games and resume from where they left off. Uses **runtime-specific handlers**: RetroArch save state API for emulators, filesystem snapshots for native/Wine games.

## User Requirements

- **Runtime-specific save states**: Use RetroArch's native save state API for emulator runtimes, filesystem snapshots for native Linux/Wine games
- **Auto-save triggers**: On game swap (when exiting to switch games) AND periodic snapshots every N minutes during gameplay
- **Multiple slots**: Keep 3-5 save state slots per game
- **Full termination**: Exit game process and save to disk (not process suspension)

## Architecture

**Current Game Launch Flow:**
1. User selects game from BIOS → writes to `/var/kazeta/state/.LAUNCH_CMD`
2. System executes `/rootfs/usr/bin/kazeta` bash script
3. Script mounts game via OverlayFS: lower (game files) + upper (save data at `~/.local/share/kazeta/saves/default/{cart_id}/`)
4. Runtime (.kzr file) mounted into overlay
5. Game executes, writes go to upper layer
6. On exit, trap handler unmounts and cleans up

**Key Integration Points:**
- Save data: `~/.local/share/kazeta/saves/default/{cart_id}/`
- Launcher: `/rootfs/usr/bin/kazeta` (lines 217-227: EXIT trap)
- BIOS: `/bios/src/main.rs`, `/bios/src/ui/main_menu.rs`

---

## Storage Structure

### Directory Layout

```
~/.local/share/kazeta/states/{cart_id}/
├── metadata.json          # Index of all save states for this game
├── slot_001/
│   ├── state.json        # Metadata for this specific slot
│   ├── screenshot.png    # Optional 8-50KB preview image
│   └── data/
│       ├── retroarch.state      # For RetroArch-based emulator runtimes
│       └── filesystem.tar.gz    # For native/Wine games
├── slot_002/
├── slot_003/
├── slot_004/
└── slot_005/
```

### Metadata Schemas

**`metadata.json`** (game-level index):
```json
{
  "cart_id": "super-mario-64",
  "cart_name": "Super Mario 64",
  "runtime": "nintendo64-1.0.kzr",
  "last_updated": "2025-12-14T10:30:00Z",
  "slots": [
    {
      "slot_id": "001",
      "timestamp": "2025-12-14T10:30:00Z",
      "type": "auto",
      "size_bytes": 15728640
    }
  ]
}
```

**`state.json`** (per-slot metadata):
```json
{
  "slot_id": "001",
  "cart_id": "super-mario-64",
  "runtime": "nintendo64-1.0.kzr",
  "runtime_type": "retroarch",
  "timestamp": "2025-12-14T10:30:00Z",
  "save_type": "auto",
  "size_bytes": 15728640,
  "has_screenshot": true
}
```

---

## Runtime Detection

### Runtime Type Classification

**RetroArch-based** (use RetroArch save state API via network commands):
- Pattern match: `dolphin`, `snes`, `nes`, `n64`, `ps1`, `psx`, `saturn`, `megadrive`, `pcengine`, `segacd`, `retroarch`
- Detection: `if runtime_name contains any pattern → RetroArch`

**Filesystem-based** (tar.gz snapshots of OverlayFS upper layer):
- `none.kzr` - Native Linux binaries
- `linux-*.kzr` - Steam/Proton runtime
- `windows-*.kzr` - Wine-based games
- Default fallback for any runtime not matching RetroArch patterns

### Handler Implementation

**RetroArch Handler:**
- Use RetroArch network command interface (UDP localhost:55355)
- Send commands: `SAVE_STATE_SLOT N`, then `SAVE_STATE`
- Load: `LOAD_STATE_SLOT N`, then `LOAD_STATE`
- State file managed by RetroArch, copy to our slots directory

**Filesystem Handler:**
- Create tar.gz of `~/.local/share/kazeta/saves/default/{cart_id}/`
- **Exclude** (from existing EXCLUDED_DIRS in save.rs):
  - `.cache/`
  - `.config/pulse/cookie`
  - `.kazeta/share/` (runtime files)
  - `.kazeta/var/prefix/dosdevices` (Wine symlinks)
  - `.kazeta/var/prefix/drive_c/windows` (Wine system)
- Compression: `tar -czf` (gzip) or consider `zstd` for better compression

---

## Auto-Save Mechanisms

### A. Game Swap Auto-Save

**Modify `/rootfs/usr/bin/kazeta` EXIT trap (currently lines 217-227):**

Add call to save state creation before unmounting:

```bash
trap "\
    if [ -n \"\$PLAYTIME_PID\" ]; then kill \"\$PLAYTIME_PID\" 2>/dev/null; fi; \
    if [ -n \"\$PERIODIC_SAVE_PID\" ]; then kill \"\$PERIODIC_SAVE_PID\" 2>/dev/null; fi; \
    echo 'DEBUG: Creating auto-save state...'; \
    /usr/bin/kazeta-save-state --auto \"${cart_id}\" \"${runtime}\"; \
    popd; \
    ${post_exec_cmd} \
    sudo kazeta-mount --unmount "${target}" "${runtimedir}"; \
    sudo kazeta-mount kzp --unmount "${cart_path}"; \
    sudo rm -rf "${work}"; \
    sudo rm -rf "${upper}/.kazeta/share"; \
" EXIT
```

### B. Periodic Snapshots

Create `/usr/bin/kazeta-periodic-save` script:

```bash
#!/bin/bash
cart_id="$1"
runtime="$2"
interval="${3:-5}"  # Default: 5 minutes

while true; do
    sleep $((interval * 60))

    # Check if game still running
    if ! pgrep -f "kazeta-cart-exec" > /dev/null; then
        exit 0
    fi

    /usr/bin/kazeta-save-state --periodic "$cart_id" "$runtime"
done
```

Launch in kazeta script (after line 260, after playtime capture starts):

```bash
/usr/bin/kazeta-periodic-save "$cart_id" "$runtime" 5 &
PERIODIC_SAVE_PID=$!
```

### C. Slot Rotation Logic

Maintain 3-5 slots, oldest auto/periodic state gets replaced:

1. Read `metadata.json` to get list of existing slots
2. Filter slots by type (auto/periodic vs manual)
3. Sort by timestamp, find oldest auto/periodic slot
4. Overwrite that slot with new state
5. Manual saves protected from auto-rotation (must be deleted explicitly)

---

## BIOS UI Changes

### Main Menu Enhancement

**Modify `/bios/src/ui/main_menu.rs`** (line 17):

```rust
pub const MAIN_MENU_OPTIONS: &[&str] = &[
    "DATA",
    "PLAY",
    "RESUME",           // NEW - launch with latest save state
    "SAVE STATES",      // NEW - manage save states
    "COPY SESSION LOGS",
    "SETTINGS",
    "EXTRAS",
    "ABOUT"
];
```

Add handlers:
- **RESUME**: Check if save states exist for current cart, load latest, launch game with `--resume` flag
- **SAVE STATES**: Navigate to new SaveStates screen

Option visibility:
- RESUME: Only enabled when `cart_connected && has_save_states(cart_id)`
- SAVE STATES: Only enabled when `cart_connected && has_save_states(cart_id)`

### New Screen: Save State Management

Create `/bios/src/ui/save_states.rs`:

**Features:**
- Grid view showing 5 slots (or 3, configurable)
- Each slot shows:
  - Screenshot thumbnail (if available)
  - Slot number (001-005)
  - Save type badge (AUTO/MANUAL/PERIODIC)
  - Relative timestamp ("2 hours ago", "Yesterday")
  - File size (MB)
- Actions:
  - **A button**: Load selected save state and launch game
  - **X button**: Delete selected save state (with confirmation)
  - **B button**: Return to main menu
- Empty slots shown as grayed out placeholders

**Add to `/bios/src/types.rs` Screen enum** (around line 61):

```rust
pub enum Screen {
    MainMenu,
    SaveData,
    SaveStates,  // NEW
    // ... rest
}
```

---

## Data Structures

### New Types in `/bios/src/types.rs`

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SaveStateSlot {
    pub slot_id: String,          // "001", "002", etc.
    pub cart_id: String,
    pub runtime: String,
    pub runtime_type: RuntimeType,
    pub timestamp: DateTime<Utc>,
    pub save_type: SaveType,
    pub size_bytes: u64,
    pub has_screenshot: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RuntimeType {
    RetroArch,
    Filesystem,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SaveType {
    Auto,      // Created on game exit
    Periodic,  // Created every N minutes
    Manual,    // User-created (future: hotkey support)
}
```

### Extend Existing Types

**In `/bios/src/save.rs` CartInfo struct** (currently lines 40-47):

```rust
pub struct CartInfo {
    pub name: Option<String>,
    pub id: String,
    pub exec: String,
    pub icon: String,
    pub runtime: Option<String>,
    pub has_save_states: bool,  // NEW
    pub latest_state_timestamp: Option<DateTime<Utc>>, // NEW
}
```

**Add to `/bios/src/save.rs` SaveError enum** (currently lines 62-67):

```rust
pub enum SaveError {
    Io(io::Error),
    Message(String),
    Walkdir(walkdir::Error),
    StripPrefix(std::path::StripPrefixError),
    StateNotFound(String),      // NEW
    CorruptedState(String),     // NEW
    RuntimeNotSupported(String), // NEW
}
```

---

## Implementation Files

### New Files to Create

1. **`/bios/src/state.rs`** - Core state management module
   - `list_save_states(cart_id) -> Result<Vec<SaveStateSlot>>`
   - `has_save_states(cart_id) -> bool`
   - `get_latest_state(cart_id) -> Result<SaveStateSlot>`
   - `delete_save_state(cart_id, slot_id) -> Result<()>`
   - `detect_runtime_type(runtime_name) -> RuntimeType`
   - Metadata JSON read/write helpers

2. **`/rootfs/usr/bin/kazeta-save-state`** - State creation script
   - Parse arguments: `--auto|--periodic|--manual cart_id runtime`
   - Detect runtime type (RetroArch vs filesystem)
   - Find next available slot (rotation logic)
   - Call appropriate handler (RetroArch network commands or tar)
   - Update metadata.json
   - Optional: Capture screenshot

3. **`/rootfs/usr/bin/kazeta-load-state`** - State restoration script
   - Parse arguments: `cart_id [slot_id]` (default: latest)
   - Read metadata.json to get slot info
   - Detect runtime type
   - Call appropriate handler (RetroArch commands or tar extract)

4. **`/rootfs/usr/bin/kazeta-periodic-save`** - Periodic saver daemon
   - Loop: sleep N minutes, check if game still running, trigger save

5. **`/bios/src/ui/save_states.rs`** - Save states management UI screen
   - Grid rendering
   - Input handling (navigate, load, delete)
   - Screenshot thumbnails
   - Relative timestamps

### Files to Modify

1. **`/rootfs/usr/bin/kazeta`** - Main launcher script
   - Add `--resume` flag parsing (after line 8)
   - Call `kazeta-load-state` before PreExec if resume mode (before line 238)
   - Enhance EXIT trap to call `kazeta-save-state --auto` (line 217-227)
   - Start `kazeta-periodic-save` background job (after line 260)
   - Kill periodic save PID in trap

2. **`/bios/src/types.rs`** - Core data structures
   - Add `SaveStateSlot`, `RuntimeType`, `SaveType` structs/enums
   - Add `Screen::SaveStates` to Screen enum

3. **`/bios/src/save.rs`** - Save management
   - Extend `CartInfo` with `has_save_states` and `latest_state_timestamp`
   - Add `StateNotFound`, `CorruptedState`, `RuntimeNotSupported` to SaveError
   - Import and re-export state module functions

4. **`/bios/src/ui/main_menu.rs`** - Main menu
   - Update MAIN_MENU_OPTIONS array to include "RESUME" and "SAVE STATES"
   - Add resume_option_enabled and save_states_option_enabled variables
   - Add handlers for RESUME (index 2) and SAVE STATES (index 3)
   - Shift existing indices (COPY SESSION LOGS → 4, SETTINGS → 5, etc.)

5. **`/bios/src/main.rs`** - Main BIOS loop
   - Import `ui::save_states` module
   - Add `Screen::SaveStates` case to screen rendering match statement
   - Handle transitions to/from SaveStates screen

6. **`/bios/src/ui/mod.rs`** - UI module exports
   - Add `pub mod save_states;`

7. **`/bios/src/config.rs`** - Configuration (optional, for settings)
   - Add `auto_save_on_exit: bool` (default: true)
   - Add `periodic_save_enabled: bool` (default: false)
   - Add `periodic_save_interval: u32` (default: 5 minutes)
   - Add `max_save_state_slots: usize` (default: 5)
   - Expose in GENERAL SETTINGS screen

---

## Implementation Sequence

### Phase 1: Core Infrastructure (Foundation)

**Priority 1 - Shell Scripts:**
- [ ] Create `/usr/bin/kazeta-save-state` (filesystem handler first)
- [ ] Create `/usr/bin/kazeta-load-state` (filesystem handler first)
- [ ] Modify `/rootfs/usr/bin/kazeta` EXIT trap to call auto-save
- [ ] Test: Launch game → exit → verify tar.gz created in states directory

**Priority 2 - Rust State Module:**
- [ ] Create `/bios/src/state.rs` with basic functions
- [ ] Add data structures to `/bios/src/types.rs` (SaveStateSlot, enums)
- [ ] Extend `/bios/src/save.rs` CartInfo and SaveError
- [ ] Test: BIOS can detect and list save states

### Phase 2: BIOS UI Integration

- [ ] Modify `/bios/src/ui/main_menu.rs` - add RESUME option
- [ ] Update `/bios/src/main.rs` to handle resume triggering
- [ ] Test: RESUME option appears when save states exist, launches with `--resume`
- [ ] Modify `/rootfs/usr/bin/kazeta` to accept `--resume` flag and call `kazeta-load-state`
- [ ] Test: Full cycle: play → exit (auto-save) → resume → verify game state restored

### Phase 3: Save State Management UI

- [ ] Create `/bios/src/ui/save_states.rs` screen
- [ ] Add "SAVE STATES" option to main menu
- [ ] Implement grid view, slot selection, delete functionality
- [ ] Test: Navigate to save states screen, view slots, delete slot

### Phase 4: RetroArch Integration

- [ ] Add RetroArch network command support to `kazeta-save-state`
- [ ] Add RetroArch restoration to `kazeta-load-state`
- [ ] Update `detect_runtime_type()` in state.rs
- [ ] Test: Play RetroArch game (e.g., SNES), exit, resume, verify state loaded

### Phase 5: Periodic Snapshots

- [ ] Create `/usr/bin/kazeta-periodic-save` script
- [ ] Modify kazeta launcher to start periodic save daemon
- [ ] Add configuration options to config.rs (interval, enable/disable)
- [ ] Test: Play game for 10+ minutes, verify periodic states created

### Phase 6: Polish & Testing

- [ ] Add screenshot capture (optional feature)
- [ ] Implement slot rotation logic (ensure oldest auto/periodic replaced)
- [ ] Error handling improvements (corrupted states, missing files)
- [ ] UI animations and visual polish
- [ ] Integration testing across all runtime types
- [ ] Documentation

---

## Critical Files Summary

**Must modify:**
1. `/rootfs/usr/bin/kazeta` - Launcher script (auto-save, resume, periodic)
2. `/bios/src/save.rs` - Extend CartInfo and SaveError
3. `/bios/src/types.rs` - Add SaveStateSlot, RuntimeType, SaveType, Screen::SaveStates
4. `/bios/src/ui/main_menu.rs` - Add RESUME and SAVE STATES options
5. `/bios/src/main.rs` - Handle SaveStates screen and resume triggers

**Must create:**
1. `/bios/src/state.rs` - Core state management logic
2. `/usr/bin/kazeta-save-state` - State creation script
3. `/usr/bin/kazeta-load-state` - State restoration script
4. `/usr/bin/kazeta-periodic-save` - Periodic saver daemon
5. `/bios/src/ui/save_states.rs` - UI screen for state management

---

## Testing Strategy

### Unit Tests (Manual)
- Metadata JSON serialization/deserialization
- Slot rotation logic (oldest replaced)
- Runtime type detection (pattern matching)

### Integration Tests
1. **Native Linux game:**
   - Launch → play briefly → exit
   - Verify auto-save tar.gz created
   - RESUME from menu → verify files restored

2. **Wine game:**
   - Same as above
   - Verify Wine prefix preserved correctly

3. **RetroArch emulator:**
   - Launch SNES/N64 game
   - Exit → verify RetroArch .state file captured
   - Resume → verify correct game state restored

4. **Periodic save:**
   - Launch game → wait 5+ minutes
   - Verify periodic snapshot created
   - Verify multiple snapshots rotate correctly (keep most recent 5)

5. **Slot management:**
   - Create 5 auto-saves
   - Verify 6th save overwrites oldest
   - Manually delete slot from UI
   - Verify metadata updated correctly

---

## Known Challenges & Solutions

### Challenge: RetroArch Save State Directory

**Problem:** RetroArch needs to know where to save states.

**Solution:**
- Modify runtime .kzr to include custom retroarch.cfg
- Set `savestate_directory = ~/.local/share/kazeta/states/{cart_id}/current/`
- Use symlink: `current/` → `slot_001/data/`
- OR: Let RetroArch save to default location, then copy .state file to our slots

### Challenge: Filesystem Snapshot Size

**Problem:** Full game save directories can be 50-200MB.

**Solution:**
- Use gzip -9 or zstd compression (70-80% reduction)
- EXCLUDED_DIRS already filters out large unnecessary directories
- Limit to 5 slots maximum
- Consider incremental snapshots (future enhancement)

### Challenge: Screenshot Capture

**Problem:** Capturing game framebuffer is complex.

**Solution:**
- RetroArch: Use `SCREENSHOT` network command (if supported)
- Native: Use `import -window root screenshot.png` (ImageMagick) if installed
- Gracefully degrade: Screenshot is optional, display placeholder if missing

### Challenge: Periodic Save Safety

**Problem:** Saving while game is running could corrupt state.

**Solution:**
- RetroArch: Network commands are safe (built-in feature)
- Native games:
  - Option 1: Only use auto-save on exit (safest, recommended for Phase 5)
  - Option 2: SIGSTOP game → snapshot → SIGCONT (riskier, test thoroughly)
- Make periodic saves opt-in via config

---

## Configuration Options (Optional Enhancement)

Add to `/bios/src/config.rs` and GENERAL SETTINGS screen:

```rust
pub struct Config {
    // ... existing fields ...

    pub auto_save_on_exit: bool,        // Default: true
    pub periodic_save_enabled: bool,    // Default: false
    pub periodic_save_interval: u32,    // Minutes, default: 5
    pub max_save_state_slots: usize,    // Default: 5
}
```

Settings UI:
- **Auto-save on exit:** ON / OFF
- **Periodic snapshots:** ON / OFF
- **Snapshot interval:** 1 / 3 / 5 / 10 minutes
- **Max slots per game:** 3 / 5 / 10

---

## Estimated Effort

**Phase 1-2 (Core + Basic UI):** 3-5 days
- Filesystem handler working
- Auto-save on exit
- RESUME from main menu
- Basic state management

**Phase 3-4 (Advanced UI + RetroArch):** 3-4 days
- Save States screen with grid view
- RetroArch integration
- Delete functionality

**Phase 5-6 (Periodic + Polish):** 2-3 days
- Periodic snapshots
- Configuration options
- Testing and bug fixes

**Total:** 8-12 days (1 developer, full-time)

---

## Success Criteria

✅ User can exit a game and see "RESUME" option in main menu
✅ Resume launches game and restores state from last session
✅ Save states work for both RetroArch and native games
✅ User can view all save states in dedicated UI screen
✅ User can delete individual save states
✅ Periodic snapshots create states every N minutes (configurable)
✅ Slots rotate automatically (oldest replaced when full)
✅ File size is reasonable (<50MB per state with compression)
✅ No data corruption or loss during save/restore operations
