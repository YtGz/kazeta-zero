# RetroAchievements Integration Plan for Kazeta+

## Overview

This document outlines the plan to integrate [RetroAchievements](https://retroachievements.org/) with the Kazeta+ system, providing achievement tracking for retro games across all supported runtimes.

## Architecture

```
┌─────────────────────────────────────────────────────────────────────────┐
│                           Kazeta+ System                                 │
├─────────────────────────────────────────────────────────────────────────┤
│                                                                          │
│  ┌─────────────┐     ┌──────────────────┐     ┌────────────────────┐   │
│  │    BIOS     │────▶│  RA Credentials  │◀────│  RetroAchievements │   │
│  │  (Login UI) │     │    Storage       │     │       API          │   │
│  └─────────────┘     └──────────────────┘     └────────────────────┘   │
│         │                    │                          ▲              │
│         ▼                    ▼                          │              │
│  ┌─────────────┐     ┌──────────────────┐              │              │
│  │   Overlay   │◀────│  kazeta-ra CLI   │──────────────┘              │
│  │ (Toasts UI) │     │  (RA Service)    │                             │
│  └─────────────┘     └──────────────────┘                             │
│         ▲                    ▲                                         │
│         │                    │                                         │
│  ┌──────┴────────────────────┴──────────────────────────────┐        │
│  │                    Runtime Wrappers                       │        │
│  │  ┌─────────┐  ┌─────────┐  ┌─────────┐  ┌─────────┐     │        │
│  │  │   GBA   │  │   NES   │  │  SNES   │  │   PSX   │ ... │        │
│  │  │ (VBA-M) │  │(Mesen2) │  │ (bsnes) │  │(DuckSt) │     │        │
│  │  └─────────┘  └─────────┘  └─────────┘  └─────────┘     │        │
│  └──────────────────────────────────────────────────────────┘        │
│                                                                          │
└─────────────────────────────────────────────────────────────────────────┘
```

## Components

### 1. RetroAchievements Service (`kazeta-ra`)

A new Rust CLI tool that handles all RA API interactions.

**Location:** `ra/` (new directory)

**Responsibilities:**

- Store and manage RA credentials securely
- Fetch game achievement lists from RA API
- Submit achievement unlocks to RA API
- Cache achievement data locally
- Provide CLI interface for runtimes to call

**IPC Protocol:**

```json
// Request achievement list for a game
{"command": "get_achievements", "game_hash": "abc123", "console_id": 5}

// Submit achievement unlock
{"command": "unlock", "achievement_id": 12345, "hardcore": false}

// Get user profile
{"command": "get_profile"}
```

### 2. BIOS Integration

**New UI Screens:**

- **RetroAchievements Login** - Username/API key entry
- **RA Profile View** - Show user stats, recent achievements
- **RA Settings** - Enable/disable RA, hardcore mode toggle

**Config Changes (`config.toml`):**

```toml
[retroachievements]
enabled = true
hardcore_mode = false
show_notifications = true
notification_duration = 5000
```

**Credential Storage:**

- Store in `~/.local/share/kazeta-plus/ra_credentials.json`
- Encrypted with user password or system keyring
- Fields: `username`, `api_key`, `web_api_key`

### 3. Overlay Integration

**New IPC Messages:**

```rust
enum OverlayMessage {
    // Existing...

    // New RA messages
    RAGameStart {
        game_title: String,
        game_icon: Option<String>,
        total_achievements: u32,
        earned_achievements: u32,
    },
    RAAchievementUnlocked {
        achievement_id: u32,
        title: String,
        description: String,
        points: u32,
        icon_url: Option<String>,
        is_hardcore: bool,
    },
    RALeaderboardSubmit {
        leaderboard_name: String,
        score: String,
        rank: Option<u32>,
    },
    RAProgressUpdate {
        earned: u32,
        total: u32,
    },
}
```

**New Overlay Screens:**

- **Achievements Browser** - View all achievements for current game
- **RA Profile** - Quick view of user stats during gameplay

### 4. Runtime Integration

Each runtime wrapper needs to integrate with RA-capable emulators.

#### Option A: Emulator-Native RA Support (Preferred)

Many emulators have built-in RetroAchievements support via `rcheevos`:

| Emulator    | RA Support  | Console             |
| ----------- | ----------- | ------------------- |
| **VBA-M**   | ✅ Built-in | **GBA** (preferred) |
| mGBA        | ✅ Built-in | GBA                 |
| RetroArch   | ✅ Built-in | Multi               |
| DuckStation | ✅ Built-in | PSX                 |
| PCSX2       | ✅ Built-in | PS2                 |
| Dolphin     | ✅ Built-in | GC/Wii              |
| PPSSPP      | ✅ Built-in | PSP                 |

**Note:** For Kazeta+, VBA-M is the preferred GBA emulator due to its superior local multiplayer support (separate processes with shared memory linking).

**Integration approach:**

1. Pass RA credentials to emulator via config/env vars
2. Configure emulator to send achievement notifications via IPC
3. Parse emulator logs for achievement events (fallback)

#### Option B: External RA Client (Fallback)

For emulators without native RA support:

1. Use `rcheevos` library to monitor game memory
2. Run alongside emulator as separate process
3. Inject achievement logic based on game hash

### 5. Achievement Data Flow

```
Game Launch
    │
    ▼
Runtime Wrapper starts
    │
    ├── Compute ROM hash (MD5/SHA1)
    │
    ├── Call: kazeta-ra get_achievements --hash <hash>
    │       │
    │       ▼
    │   RA API: GET /API_GetGameInfoAndUserProgress.php
    │       │
    │       ▼
    │   Cache response locally
    │
    ├── Notify overlay: RAGameStart { game_title, achievements... }
    │
    ▼
Game Running
    │
    ├── Emulator monitors memory for achievement triggers
    │
    ├── Achievement triggered!
    │       │
    │       ▼
    │   Call: kazeta-ra unlock --id <id> [--hardcore]
    │       │
    │       ▼
    │   RA API: POST /API_AwardAchievement.php
    │       │
    │       ▼
    │   Notify overlay: RAAchievementUnlocked { ... }
    │
    ▼
Game Exit
    │
    ├── Sync any pending achievements
    │
    └── Update local cache
```

## Implementation Phases

### Phase 1: Foundation (Week 1-2)

**Tasks:**

1. Create `ra/` directory structure
2. Implement `kazeta-ra` CLI tool
   - RA API client (using `reqwest`)
   - Credential management
   - Local caching (SQLite)
3. Add RA settings to BIOS config
4. Add RA login screen to BIOS UI

**Files to create:**

```
ra/
├── Cargo.toml
├── src/
│   ├── main.rs           # CLI entry point
│   ├── lib.rs            # Library exports
│   ├── api.rs            # RA API client
│   ├── auth.rs           # Credential management
│   ├── cache.rs          # Local achievement cache
│   ├── hash.rs           # ROM hashing utilities
│   └── types.rs          # Data structures
```

**Files to modify:**

```
bios/src/config.rs        # Add RA settings
bios/src/ui/mod.rs        # Add RA UI screens
bios/Cargo.toml           # Add kazeta-ra dependency
```

### Phase 2: Overlay Integration (Week 3)

**Tasks:**

1. Add new RA-related IPC messages
2. Implement RA toast notifications
3. Create achievement browser screen
4. Add achievement progress indicator

**Files to modify:**

```
overlay/src/ipc.rs        # New message types
overlay/src/state.rs      # Handle RA messages
overlay/src/rendering.rs  # Achievement UI rendering
```

### Phase 3: GBA Runtime - VBA-M (Week 4)

**Tasks:**

1. Update VBA-M wrapper for RA integration
2. Configure VBA-M RA settings via config file
3. Hook VBA-M achievement notifications to overlay
4. Test with known RA-supported GBA games

**Files to modify:**

```
runtimes/gba/vba-run-wrapper.sh
```

**VBA-M RA Integration Notes:**

- VBA-M has built-in RetroAchievements support
- Credentials passed via config file or command-line
- Achievement events can be captured via log parsing or IPC

### Phase 4: Additional Runtimes (Week 5-6)

**Tasks:**

1. Add RA support to other runtimes as needed
2. Document runtime-specific configurations
3. Create runtime RA integration guide

### Phase 5: Polish & Testing (Week 7)

**Tasks:**

1. End-to-end testing
2. Error handling improvements
3. Offline mode support
4. Documentation

## Technical Details

### RA API Endpoints

**Authentication:**

- User provides: `username` + `api_key` (from RA website)
- API key: Settings → Keys → Web API Key

**Key Endpoints:**

```
GET /API_GetUserSummary.php?u={user}&y={api_key}
GET /API_GetGameInfoAndUserProgress.php?g={game_id}&u={user}&y={api_key}
POST /API_AwardAchievement.php?u={user}&t={token}&a={achievement_id}&h={hardcore}
GET /API_GetGameList.php?c={console_id}&y={api_key}
```

### ROM Hashing

RA uses specific hash algorithms per console:

- Most consoles: MD5 of ROM
- Some consoles: Custom hashing (header stripped, etc.)

Use `rcheevos` hash functions or replicate the logic.

### Console IDs

| ID  | Console            |
| --- | ------------------ |
| 1   | Mega Drive/Genesis |
| 2   | Nintendo 64        |
| 3   | SNES               |
| 4   | Game Boy           |
| 5   | Game Boy Advance   |
| 6   | Game Boy Color     |
| 7   | NES                |
| 11  | Master System      |
| 12  | PlayStation        |
| 18  | Nintendo DS        |
| 21  | PlayStation 2      |
| 25  | Atari 2600         |
| 28  | Virtual Boy        |

### Credentials Storage Format

```json
{
  "username": "player123",
  "api_key": "xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx",
  "token": "session_token_from_login",
  "last_sync": "2025-12-15T10:30:00Z"
}
```

### Achievement Cache Schema

```sql
CREATE TABLE games (
    hash TEXT PRIMARY KEY,
    game_id INTEGER,
    title TEXT,
    icon_url TEXT,
    console_id INTEGER,
    last_updated TEXT
);

CREATE TABLE achievements (
    id INTEGER PRIMARY KEY,
    game_hash TEXT,
    title TEXT,
    description TEXT,
    points INTEGER,
    badge_url TEXT,
    type INTEGER,  -- 0=core, 1=unofficial
    FOREIGN KEY (game_hash) REFERENCES games(hash)
);

CREATE TABLE user_progress (
    achievement_id INTEGER PRIMARY KEY,
    earned INTEGER,
    earned_date TEXT,
    hardcore INTEGER,
    FOREIGN KEY (achievement_id) REFERENCES achievements(id)
);
```

## VBA-M RA Configuration (Preferred GBA Emulator)

VBA-M has built-in RetroAchievements support via the rcheevos library.

### VBA-M Configuration File

Location: `~/.config/visualboyadvance-m/vbam.ini` or passed via `--config`

```ini
[RetroAchievements]
Username=<username>
Token=<token>
Enabled=1
Hardcore=0
TestMode=0
Notifications=1
NotificationDuration=5000
```

### VBA-M Command Line Options

```bash
# Enable RA with credentials
visualboyadvance-m \
    --ra-user=<username> \
    --ra-token=<token> \
    --ra-hardcore=0 \
    game.gba
```

### VBA-M RA Login Flow

1. User enters credentials in BIOS RA settings
2. `kazeta-ra login` obtains session token from RA API
3. Token written to VBA-M config before game launch
4. VBA-M connects to RA servers and tracks achievements

### VBA-M Achievement Notifications

VBA-M can display achievement notifications natively, but for consistent UX:

1. Disable VBA-M's built-in notifications
2. Configure VBA-M to log achievement events
3. Parse log output and send to Kazeta overlay via IPC

```ini
[RetroAchievements]
Notifications=0  # Disable built-in, use Kazeta overlay instead
LogEvents=1      # Enable event logging
```

### Integration with vba-run-wrapper.sh

The wrapper script will:

1. Read RA credentials from `kazeta-ra get-credentials`
2. Generate VBA-M config with RA settings
3. Launch VBA-M with the config
4. Optionally monitor stdout for achievement events
5. Forward achievement events to overlay via `kazeta-ra notify`

## Environment Variables

```bash
# For emulators that support env-based config
export RETROACHIEVEMENTS_USERNAME="player123"
export RETROACHIEVEMENTS_TOKEN="xxx"
export RETROACHIEVEMENTS_HARDCORE="0"
```

## Error Handling

| Scenario            | Handling                                       |
| ------------------- | ---------------------------------------------- |
| No internet         | Queue achievements locally, sync later         |
| Invalid credentials | Show error in BIOS, prompt re-login            |
| API rate limit      | Exponential backoff, cache aggressively        |
| Unknown game        | Show "No achievements available"               |
| Emulator crash      | Achievements submitted instantly, nothing lost |

## Privacy Considerations

1. RA username/stats are public by default on RA website
2. Credentials stored locally only
3. No telemetry sent to Kazeta servers
4. Users can disable RA entirely in settings

## Future Enhancements

1. **Leaderboard support** - Submit scores to RA leaderboards
2. **Rich presence** - Show "Currently playing X" on RA profile
3. **Achievement challenges** - Daily/weekly challenges
4. **Social features** - Friend activity feed
5. **Achievement sounds** - Custom sound effects for unlocks
6. **Mastery tracking** - Track 100% completion per game

## References

- [RetroAchievements API Documentation](https://api-docs.retroachievements.org/)
- [rcheevos Library](https://github.com/RetroAchievements/rcheevos)
- [RA Console IDs](https://api-docs.retroachievements.org/systems.html)
- [mGBA Documentation](https://mgba.io/docs/scripting.html)
- [DuckStation RA Integration](https://github.com/stenzek/duckstation)

## Appendix: Sample Code

### ROM Hash Calculation (Rust)

```rust
use md5::{Md5, Digest};
use std::fs::File;
use std::io::Read;

pub fn hash_rom(path: &str, console_id: u32) -> Result<String, std::io::Error> {
    let mut file = File::open(path)?;
    let mut buffer = Vec::new();
    file.read_to_end(&mut buffer)?;

    // Some consoles need header stripped
    let data = match console_id {
        7 => strip_nes_header(&buffer),  // NES
        _ => &buffer,
    };

    let hash = Md5::digest(data);
    Ok(format!("{:x}", hash))
}

fn strip_nes_header(data: &[u8]) -> &[u8] {
    if data.len() > 16 && &data[0..4] == b"NES\x1a" {
        &data[16..]
    } else {
        data
    }
}
```

### Overlay Achievement Toast

```rust
fn render_achievement_toast(achievement: &Achievement, progress: f32) {
    let toast_width = 350.0;
    let toast_height = 80.0;
    let x = screen_width() - toast_width - 20.0;
    let y = 20.0;

    // Background
    draw_rectangle(x, y, toast_width, toast_height, Color::new(0.1, 0.1, 0.1, 0.95));

    // Gold border for achievement
    draw_rectangle_lines(x, y, toast_width, toast_height, 2.0, GOLD);

    // Trophy icon
    draw_text("🏆", x + 10.0, y + 45.0, 40.0, GOLD);

    // Achievement title
    draw_text(&achievement.title, x + 60.0, y + 30.0, 20.0, WHITE);

    // Points
    draw_text(
        &format!("{} points", achievement.points),
        x + 60.0,
        y + 55.0,
        16.0,
        Color::new(0.7, 0.7, 0.7, 1.0),
    );

    // Progress bar (slide in animation)
    draw_rectangle(x, y + toast_height - 4.0, toast_width * progress, 4.0, GOLD);
}
```

### VBA-M Runtime Wrapper RA Integration

```bash
#!/bin/bash
# VBA-M RA integration in vba-run-wrapper.sh

# ===================================
# RETROACHIEVEMENTS SETUP
# ===================================

setup_retroachievements() {
    # Check if RA is enabled and credentials exist
    RA_CREDS=$(kazeta-ra get-credentials 2>/dev/null)
    if [ $? -ne 0 ]; then
        echo "RetroAchievements: Not configured (run BIOS → Settings → RetroAchievements)"
        return 1
    fi

    RA_USERNAME=$(echo "$RA_CREDS" | jq -r '.username')
    RA_TOKEN=$(echo "$RA_CREDS" | jq -r '.token')
    RA_HARDCORE=$(echo "$RA_CREDS" | jq -r '.hardcore // 0')

    if [ -z "$RA_USERNAME" ] || [ -z "$RA_TOKEN" ]; then
        echo "RetroAchievements: Invalid credentials"
        return 1
    fi

    # Compute ROM hash for game identification
    ROM_HASH=$(kazeta-ra hash-rom "$ROM_PATH" --console gba)
    echo "RetroAchievements: Game hash = $ROM_HASH"

    # Fetch game info and notify overlay
    kazeta-ra game-start --hash "$ROM_HASH"  &

    return 0
}

# Call during wrapper initialization
RA_ENABLED=false
if setup_retroachievements; then
    RA_ENABLED=true
fi

# ===================================
# VBA-M CONFIG WITH RA
# ===================================

create_vbam_config_with_ra() {
    local config_file="$1"

    cat > "$config_file" <<EOF
[General]
recentThreshold=10

[GBA]
BiosFile=$([ -n "$BIOS_PATH" ] && echo "$BIOS_PATH" || echo "")

[video]
fullscreen=1

[RetroAchievements]
Enabled=$([ "$RA_ENABLED" = true ] && echo "1" || echo "0")
Username=$RA_USERNAME
Token=$RA_TOKEN
Hardcore=$RA_HARDCORE
Notifications=0
LogEvents=1
EOF

    echo "$config_file"
}

# ===================================
# LAUNCH WITH RA MONITORING
# ===================================

launch_with_ra_monitoring() {
    local rom="$1"
    local config_file="$2"

    # Create a named pipe for achievement events
    RA_PIPE="/tmp/kazeta-ra-events-$$"
    mkfifo "$RA_PIPE" 2>/dev/null || true

    # Background process to monitor VBA-M output for achievements
    if [ "$RA_ENABLED" = true ]; then
        (
            while read -r line; do
                # Parse VBA-M achievement log lines
                if echo "$line" | grep -q "Achievement Unlocked:"; then
                    ACHIEVEMENT_ID=$(echo "$line" | grep -oP 'ID:\K[0-9]+')
                    kazeta-ra notify-achievement --id "$ACHIEVEMENT_ID"
                fi
            done < "$RA_PIPE"
        ) &
        MONITOR_PID=$!
    fi

    # Launch VBA-M, tee output to pipe
    "$VBA_BIN" --config="$config_file" -f "$rom" 2>&1 | tee "$RA_PIPE"
    VBA_EXIT=$?

    # Cleanup
    [ -n "$MONITOR_PID" ] && kill "$MONITOR_PID" 2>/dev/null
    rm -f "$RA_PIPE"

    return $VBA_EXIT
}
```
