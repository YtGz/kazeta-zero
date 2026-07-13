# Kazeta Zero

Offline, no-account RetroAchievements, running entirely on-device — a fork of [kazeta-plus-plus](https://github.com/goldsziggy/kazeta-plus-plus), hosted at [github.com/YtGz/kazeta-zero](https://github.com/YtGz/kazeta-zero).

Kazeta Zero replaces the server-dependent RetroAchievements flow with a fully local one: achievement definitions are fetched once and baked into each cartridge's SD card, then evaluated on-device by the rcheevos engine against the emulator's live memory every frame. The playing machine has no internet connection and no RetroAchievements account. Unlocks are stored in a local SQLite database and never transmitted anywhere.

GameCube is the first console supported — running through a standalone Dolphin runtime — because it is the hardest case (partition-aware disc hashing, compressed RVZ support, external memory access). The offline model itself is console-agnostic by design.

## What's New in Kazeta Zero

### 🎫 Offline, No-Account Achievements
A fully local RetroAchievements experience — GameCube is the first supported console, with the architecture designed to extend to others:
- **No internet required on the playing machine** — the console never contacts retroachievements.org
- **No RetroAchievements account needed** — no login, no token, no session, no tracking
- **On-device evaluation** — the rcheevos engine parses achievement conditions and reads Dolphin memory each frame, detecting unlocks locally
- **Local unlocks** — every unlock is recorded in an on-device SQLite database; nothing is synced upstream
- **Baked-in definitions** — achievement sets (conditions, titles, badges) live on the cartridge SD card as `achievements.json` + `badges/`

### 🎮 In-Game Overlay System
Real-time overlay UI accessible during gameplay via Guide button, F12, or Ctrl+O:
- **Achievement Tracking**: View unlocked achievements and progress
- **Performance Monitor**: Live CPU, RAM, temperature, and FPS stats (toggle with F3)
- **Controller Tester**: Interactive gamepad button testing and diagnostics
- **Playtime Tracking**: Automatic session time tracking per game
- **Multiple Themes**: Choose from Dark, Light, RetroGreen, PlayStation, or Xbox themes
- **Toast Notifications**: In-game achievement unlocks and system messages

### ⚡ Performance Optimizations
Critical performance improvements for better resource usage:
- **Streaming ROM Hashing**: 98% memory reduction for large ROMs (N64 64MB+)
- **Partition-Aware GameCube Hashing**: rcheevos `rc_hash_gamecube()` handles RVZ/ISO/GCM discs with DOL+header partition hashing (other consoles keep using the existing pure-Rust MD5 path)
- **Idle Overlay Optimization**: 66% CPU reduction when overlay is hidden
- **Event-Driven Input Detection**: Zero background CPU usage for device monitoring

### 🎯 Global Input Daemon (Linux)
Background service for system-wide hotkey detection:
- **Always-On Hotkeys**: Guide button works regardless of window focus
- **Hotplug Support**: Automatic detection of newly connected controllers
- **Multi-Device**: Supports 4+ controllers simultaneously
- **Event-Driven**: inotify-based device detection (zero polling overhead)

## Achievement Architecture

### Local Definitions Format
Each cartridge carries an `achievements.json` file containing the full achievement set — the same data the RetroAchievements API returns, stored locally:

```json
{
  "game_id": 7693,
  "game_title": "Mario Kart: Double Dash",
  "console_id": 16,
  "console_name": "Nintendo GameCube",
  "achievements": [
    {
      "id": 55001,
      "title": "First Grand Prix",
      "description": "Complete your first Grand Prix",
      "points": 10,
      "badge_name": "55001",
      "mem_addr": "0xH00801234=1.0.5.0=d0xH00801234",
      "type": "standard",
      "display_order": 1
    }
  ]
}
```

The `mem_addr` field is the rcheevos condition string — the full achievement logic (memory addresses, hit counts, pause conditions, delta comparisons) that the engine parses via `rc_parse_trigger()`.

### Evaluation Pipeline
1. **Game start**: `kazeta-ra game-start` detects `achievements.json` on the cartridge and loads definitions locally (no API call, no auth)
2. **Hashing**: GameCube discs are hashed via rcheevos' `rc_hash_gamecube()` — partition-aware, handles compressed RVZ files
3. **Memory access**: kazeta-ra reads Dolphin's emulator memory each frame
4. **Evaluation**: rcheevos' `rc_runtime_tick()` evaluates all achievement conditions against the memory buffer
5. **Unlock**: When a trigger fires, the unlock is recorded in the local SQLite `local_unlocks` table and a `RaAchievementUnlocked` IPC message is sent to the overlay
6. **Notification**: The overlay shows a toast and updates the achievement list

### What This Fork Does NOT Do
- Does not contact retroachievements.org from the playing machine
- Does not require an RA account on the playing machine
- Does not upload, sync, or report unlocks anywhere
- Does not use leaderboards (those are inherently online)

## Core Features

### Media & Storage
- [Multi-cart support](https://github.com/the-outcaster/kazeta-plus/wiki/Multi%E2%80%90Cart-Logic)
- [Optical disc drive support](https://github.com/the-outcaster/kazeta-plus/wiki/Creating-Optical-Disc-Media) (CDs, DVDs, etc)
  - Music CD player support
- Compressed `.kzp` EROFS image support for space-efficient game packaging
- Runtime downloads directly to hard drive (saves space on removable media)

### Display & Audio
- Multi-resolution and aspect ratio support, including 4:3
- Multi-audio sink support with adjustable volume controls
- Steam Deck volume and brightness control support

### Controller & Input
- Bluetooth controller support
- Native GameCube controller adapter support, overclocked to 1,000 Hz
- Global hotkey support (Guide button, F12, Ctrl+O)
- Interactive gamepad tester in overlay

### Customization
- Full BIOS customization: fonts, backgrounds, logos, and more
- Theme support with [community themes](https://github.com/the-outcaster/kazeta-plus-themes)
- [Theme creator](https://github.com/the-outcaster/kazeta-plus-theme-creator) for making custom themes
- Overlay themes: Dark, Light, RetroGreen, PlayStation, Xbox

### System Management
- OTA update support
- Battery monitoring and clock display
- Session log copying to SD card for troubleshooting
- Error screen with session log display on cart load failures

## Architecture

Kazeta Zero uses a modular multi-process architecture:
- **BIOS** (`kazeta-bios`): Main menu and system configuration
- **Overlay** (`kazeta-overlay`): In-game UI and achievement display
- **Input Daemon** (`kazeta-input`): Global hotkey monitoring (Linux only)
- **RA Library** (`kazeta-ra`): Offline achievement definitions, hashing, evaluation, and local unlocks
- **Export Tool** (`kazeta-ra-export`): One-time achievement fetcher for the prep machine

Communication via Unix domain sockets (`/tmp/kazeta-overlay.sock`) for efficient IPC.

## Components

### BIOS
Main system UI for game selection and configuration:
- Game library with metadata display
- RetroAchievements status (shows "Local mode (offline)" when definitions are baked in)
- Theme selection and downloads
- Audio/video configuration
- Save state management
- System updates

### Overlay Daemon
Transparent in-game overlay accessible via hotkey:
- Achievement list and unlock notifications
- Performance stats (CPU, RAM, temps, FPS)
- Controller connection status and tester
- Playtime tracking
- Settings and theme selection
- Toast notification system

### Input Daemon (Linux)
Background service for global input monitoring:
- Event-driven device detection using inotify
- Hotplug support for controllers
- Multi-device monitoring
- Guide button, F12, Ctrl+O, F3 hotkeys

### RetroAchievements Library
Standalone library and CLI for offline RA integration:
- GameCube/Wii disc hashing via rcheevos FFI (`rc_hash_gamecube()`)
- Local definitions loader (reads `achievements.json` from the cartridge)
- On-device achievement evaluation via rcheevos (`rc_runtime_tick()`)
- Local SQLite unlock storage (no server contact)
- Fallback online path for backward compat with other consoles

### Export Tool (Prep Machine Only)
One-time achievement fetcher for the prep machine:
- Fetches achievement sets from the RetroAchievements API
- Exports definitions to `achievements.json` in the local format
- Downloads badge images to a `badges/` directory
- Works for any console with RetroAchievements support (GameCube is just the first target)
- This is the **only** component that touches the internet or an RA account

## Development

### Building Components

```bash
# BIOS
cd bios && cargo build --release

# Overlay (daemon mode)
cd overlay && cargo build --release --features daemon

# Input daemon (Linux only)
cd input-daemon && cargo build --release

# RA library and CLI (includes rcheevos FFI)
cd ra && cargo build --release

# Export tool (prep machine)
cd ra && cargo build --release --bin kazeta-ra-export
```

### Testing Overlay

```bash
# Start overlay daemon
cd overlay && cargo run --features daemon

# Send test messages
echo '{"type":"show_toast","message":"Test","style":"info","duration_ms":2000}' | nc -U /tmp/kazeta-overlay.sock
echo '{"type":"show_overlay","screen":"achievements"}' | nc -U /tmp/kazeta-overlay.sock
```

### RetroAchievements CLI

```bash
# Hash a GameCube ROM (partition-aware, handles RVZ)
kazeta-ra hash-rom --path game.rvz --console gamecube

# Start a game with local definitions (no account needed)
kazeta-ra game-start --path game.rvz

# View status (reports enabled when achievements.json is found)
kazeta-ra status
```

### Export Tool (Prep Machine)

```bash
# Fetch achievement definitions and badges for a game
kazeta-ra-export --username USER --api-key KEY --game-id 7693 --output-dir ./mario-kart-double-dash/
```

## Documentation

- **[ARCHITECTURE_OVERLAY.md](ARCHITECTURE_OVERLAY.md)** — Comprehensive architecture documentation
- **[RA_IMPLEMENTATION_VERIFICATION.md](RA_IMPLEMENTATION_VERIFICATION.md)** — RetroAchievements implementation details
- **[claude_plan/PERFORMANCE_ISSUES.md](claude_plan/PERFORMANCE_ISSUES.md)** — Performance optimization reference
- **[overlay/TESTING.md](overlay/TESTING.md)** — Overlay testing guide

## System Requirements

- **OS**: Linux (primary), macOS/Windows (partial support)
- **Input Daemon**: Linux with evdev support (optional)
- **Playing machine**: No internet connection required, no RetroAchievements account needed
- **Controllers**: Any gamepad with Guide/Home button (or keyboard with F12)

## Screenshots

![Kazeta+ About page](https://i.imgur.com/kQiAVvc.png)

## Credits

**Kazeta Zero is a fork of [kazeta-plus-plus](https://github.com/goldsziggy/kazeta-plus-plus)** by [Linux Gaming Central](https://linuxgamingcentral.org/).

**Lineage:** [Kazeta](https://github.com/kazetaos/kazeta) (Alkazar) → [Kazeta+](https://github.com/the-outcaster/kazeta-plus) (the-outcaster) → [Kazeta++](https://github.com/goldsziggy/kazeta-plus-plus) → **Kazeta Zero**

**Major Contributors:**
- the-outcaster (Kazeta+ fork maintainer)
- goldsziggy (Kazeta++ fork maintainer)
- Community theme creators
- RetroAchievements integration and overlay system

## License

See the [original Kazeta repository](https://github.com/kazetaos/kazeta) for license information.

## Links

- **[This fork](https://github.com/YtGz/kazeta-zero)**
- **[Upstream: Kazeta++](https://github.com/goldsziggy/kazeta-plus-plus)**
- **[Kazeta+ Wiki](https://github.com/the-outcaster/kazeta-plus/wiki/Installation)**
- **[Community Themes](https://github.com/the-outcaster/kazeta-plus-themes)**
- **[Theme Creator](https://github.com/the-outcaster/kazeta-plus-theme-creator)**
- **[Linux Gaming Central](https://linuxgamingcentral.org/)**
- **[RetroAchievements](https://retroachievements.org/)**
- **[rcheevos (C library)](https://github.com/RetroAchievements/rcheevos)**
