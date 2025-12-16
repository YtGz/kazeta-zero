# Kazeta+
The "overly complex" fork of the original [Kazeta](https://github.com/kazetaos/kazeta) project that features several enhancements, QoL improvements, and bug fixes.

## New in Kazeta+

### 🎮 In-Game Overlay System
Real-time overlay UI accessible during gameplay via Guide button, F12, or Ctrl+O:
- **Achievement Tracking**: View unlocked achievements and progress
- **Performance Monitor**: Live CPU, RAM, temperature, and FPS stats (toggle with F3)
- **Controller Tester**: Interactive gamepad button testing and diagnostics
- **Playtime Tracking**: Automatic session time tracking per game
- **Multiple Themes**: Choose from Dark, Light, RetroGreen, PlayStation, or Xbox themes
- **Toast Notifications**: In-game achievement unlocks and system messages

### 🏆 RetroAchievements Integration
Full RetroAchievements support for tracking achievements across classic games:
- **Automatic Game Detection**: ROM hashing and game identification
- **Achievement Notifications**: Real-time unlock notifications during gameplay
- **Hardcore Mode**: Optional hardcore mode for serious achievement hunters
- **Progress Tracking**: View achievement lists and completion progress
- **Local Caching**: Offline support with SQLite caching
- **CLI Tool**: Standalone `kazeta-ra` command-line tool for RA operations

Supported Consoles:
- Nintendo: NES, SNES, N64, Game Boy, GBC, GBA, Nintendo DS, Virtual Boy
- Sega: Genesis/Mega Drive, Master System
- Sony: PlayStation, PlayStation 2
- Atari 2600, and more...

### ⚡ Performance Optimizations (Dec 2025)
Critical performance improvements for better resource usage:
- **Streaming ROM Hashing**: 98% memory reduction for large ROMs (N64 64MB+)
- **Idle Overlay Optimization**: 66% CPU reduction when overlay is hidden
- **Event-Driven Input Detection**: Zero background CPU usage for device monitoring
- **Async HTTP Client**: Non-blocking RetroAchievements API calls

### 🎯 Global Input Daemon (Linux)
Background service for system-wide hotkey detection:
- **Always-On Hotkeys**: Guide button works regardless of window focus
- **Hotplug Support**: Automatic detection of newly connected controllers
- **Multi-Device**: Supports 4+ controllers simultaneously
- **Event-Driven**: inotify-based device detection (zero polling overhead)

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

## Improvements Over Original Kazeta

### Runtime Management
- Updated [runtimes](https://github.com/the-outcaster/kazeta-plus/wiki/Runtimes) for Linux, Windows, and emulator systems
- Runtime downloads to hard drive instead of removable media
- Faster load times and space savings

### Quality of Life
- Easy troubleshooting with one-click log copying
- Detailed error screens for failed cart loads
- Real-time performance monitoring
- Achievement tracking and progress display
- In-game overlay accessible without exiting

### Bug Fixes
- Font rendering fixes for applications (resolves black screen issues)
- [D-pad reversal fix](https://github.com/the-outcaster/kazeta-plus/wiki/Applying-D%E2%80%90Pad-Reversal-Fix) for native Linux games

## Architecture

Kazeta+ uses a modular multi-process architecture:
- **BIOS** (`kazeta-bios`): Main menu and system configuration
- **Overlay** (`kazeta-overlay`): In-game UI and achievement display
- **Input Daemon** (`kazeta-input`): Global hotkey monitoring (Linux only)
- **RA Library** (`kazeta-ra`): RetroAchievements CLI tool

Communication via Unix domain sockets (`/tmp/kazeta-overlay.sock`) for efficient IPC.

## Components

### BIOS
Main system UI for game selection and configuration:
- Game library with metadata display
- RetroAchievements login and settings
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
Standalone library and CLI for RA integration:
- ROM hashing with console-specific preprocessing
- Game identification and achievement fetching
- User authentication and session management
- Local caching for offline support
- Async and blocking HTTP clients

## Development

### Building Components

```bash
# BIOS
cd bios && cargo build --release

# Overlay (daemon mode)
cd overlay && cargo build --release --features daemon

# Input daemon (Linux only)
cd input-daemon && cargo build --release

# RA library and CLI
cd ra && cargo build --release
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
# Login to RetroAchievements
kazeta-ra login --username USER --api-key KEY

# Hash a ROM
kazeta-ra hash-rom --path rom.gba --console gba

# Get game info
kazeta-ra game-info --path rom.gba

# View status
kazeta-ra status
```

## Documentation

- **[Wiki](https://github.com/the-outcaster/kazeta-plus/wiki/Installation)** - Installation and setup guide
- **[ARCHITECTURE_OVERLAY.md](ARCHITECTURE_OVERLAY.md)** - Comprehensive architecture documentation
- **[RA_IMPLEMENTATION_VERIFICATION.md](RA_IMPLEMENTATION_VERIFICATION.md)** - RetroAchievements implementation details
- **[claude_plan/PERFORMANCE_ISSUES.md](claude_plan/PERFORMANCE_ISSUES.md)** - Performance optimization reference
- **[overlay/TESTING.md](overlay/TESTING.md)** - Overlay testing guide

## System Requirements

- **OS**: Linux (primary), macOS/Windows (partial support)
- **Input Daemon**: Linux with evdev support (optional)
- **RetroAchievements**: Network connectivity for achievement tracking
- **Controllers**: Any gamepad with Guide/Home button (or keyboard with F12)

## Screenshots

![Kazeta+ About page](https://i.imgur.com/kQiAVvc.png)

## Credits

**Kazeta+ brought to you by [Linux Gaming Central](https://linuxgamingcentral.org/).**

**Original concept:** Alkazar

**Major Contributors:**
- the-outcaster (Kazeta+ fork maintainer)
- Community theme creators
- RetroAchievements integration and overlay system

## License

See the [original Kazeta repository](https://github.com/kazetaos/kazeta) for license information.

## Links

- **[Official Wiki](https://github.com/the-outcaster/kazeta-plus/wiki/Installation)**
- **[Community Themes](https://github.com/the-outcaster/kazeta-plus-themes)**
- **[Theme Creator](https://github.com/the-outcaster/kazeta-plus-theme-creator)**
- **[Linux Gaming Central](https://linuxgamingcentral.org/)**
- **[RetroAchievements](https://retroachievements.org/)**
