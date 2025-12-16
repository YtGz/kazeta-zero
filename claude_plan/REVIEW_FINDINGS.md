# Kazeta++ Code Review: Overlay / Input-Daemon / RetroAchievements

## Executive Summary

This document provides a comprehensive review of three key components in the Kazeta++ codebase:
1. **Overlay** - In-game overlay daemon with achievement display, controller management, and performance HUD
2. **Input-Daemon** - Global hotkey capture daemon using Linux evdev
3. **RetroAchievements (RA)** - Integration with RetroAchievements.org for achievement tracking

---

## 1. Overlay Component

### Architecture Overview

Location: `overlay/src/`

The overlay is a macroquad-based application that renders on top of games, providing:
- Toast notifications for achievements and system events
- In-game menu (Controllers, Settings, Achievements, Quick Save, Quit)
- Performance HUD (FPS, CPU, Memory)
- Controller management (Bluetooth pairing, player assignment, gamepad tester)

### Key Files

| File | Purpose |
|------|---------|
| `main.rs` | Application loop, 60 FPS target, window setup |
| `rendering.rs` | All UI drawing code (~900 lines) |
| `state.rs` | Overlay state management, message handling |
| `ipc.rs` | Unix socket IPC for receiving messages |
| `input.rs` | Hotkey monitoring via gilrs + macroquad |
| `controllers.rs` | Controller state, Bluetooth devices, gamepad tester |
| `performance.rs` | FPS/CPU/Memory tracking |

### Performance Observations

#### Current Implementation
- Fixed 60 FPS target with `std::thread::sleep` for frame pacing (main.rs:145-147)
- Frame history tracks last 120 frames (2 seconds at 60fps)
- System stats update every 500ms to reduce sysinfo overhead (performance.rs:24)
- Non-blocking IPC polling (ipc.rs:133-135)

#### Performance Recommendations

**P1 - High Priority:**

1. **Frame Timing Precision** (`main.rs:145-147`)
   ```rust
   // Current: thread::sleep can have ~15ms variance on some systems
   if elapsed < FRAME_TIME {
       std::thread::sleep(FRAME_TIME - elapsed);
   }
   ```
   **Recommendation:** Use spin-loop for final microseconds or consider vsync:
   ```rust
   // Hybrid approach: sleep for bulk, spin for precision
   if elapsed < FRAME_TIME {
       let sleep_time = FRAME_TIME - elapsed;
       if sleep_time > Duration::from_millis(2) {
           std::thread::sleep(sleep_time - Duration::from_millis(1));
       }
       while frame_start.elapsed() < FRAME_TIME {
           std::hint::spin_loop();
       }
   }
   ```

2. **Conditional Rendering Optimization** (`main.rs:136-141`)
   - Currently calls `next_frame()` even when not visible
   - When overlay is hidden and no toasts, consider reducing loop frequency
   ```rust
   if !overlay_state.should_render() {
       // Sleep longer when hidden to save CPU
       std::thread::sleep(Duration::from_millis(50));
       next_frame().await;
       continue;
   }
   ```

3. **Achievement List Memory** (`state.rs:14`)
   - `Vec<AchievementInfo>` is stored inline in state
   - For games with 100+ achievements, consider lazy loading or pagination

**P2 - Medium Priority:**

4. **Toast Queue Unbounded** (`state.rs:692-713`)
   - `VecDeque<Toast>` has no size limit
   - Add max capacity to prevent memory growth:
   ```rust
   const MAX_TOASTS: usize = 10;
   if self.queue.len() >= MAX_TOASTS {
       self.queue.pop_front();
   }
   ```

5. **System Stats Refresh** (`performance.rs:86-100`)
   - `refresh_cpu_all()` and `refresh_memory()` called together
   - Consider staggering these operations across frames

6. **String Allocations in Rendering** (`rendering.rs`)
   - Multiple `format!()` calls per frame for display strings
   - Pre-allocate reusable strings or use `write!` to reduce allocations

### Feature Recommendations

1. **Achievement Badge Images**
   - Currently text-only achievement display
   - Add async image loading for achievement badges from RA CDN

2. **Controller Vibration Feedback**
   - Add haptic feedback on menu navigation and achievement unlocks
   - gilrs supports force feedback on some controllers

3. **Quick Resume State**
   - Implement save state preview images
   - Show last save timestamp in Quick Save menu

4. **Overlay Positioning**
   - Add configurable overlay position (corners, center)
   - Allow user to resize overlay for different screen resolutions

---

## 2. Input-Daemon Component

### Architecture Overview

Location: `input-daemon/src/main.rs`

Linux-only daemon that monitors `/dev/input/event*` devices using evdev for global hotkey capture. Supports:
- Guide/Home button on controllers (BTN_MODE)
- F12 key on keyboard
- Ctrl+O combo

### Key Features
- Multi-device support (4+ players)
- Hotplug detection (scans every 2 seconds)
- Global debounce (300ms) to prevent duplicate triggers
- Per-device modifier tracking for Ctrl+O

### Performance Observations

#### Current Implementation
- Spawns one thread per input device
- Device scan every 2000ms (DEVICE_SCAN_INTERVAL_MS)
- Non-blocking device reads with 10ms sleep on WouldBlock

#### Performance Recommendations

**P1 - High Priority:**

1. **Thread Per Device Scaling** (`main.rs:190-273`)
   - Current: One thread per device
   - With many input devices (keyboard, mouse, touchpad, controllers), can spawn 8+ threads
   **Recommendation:** Use async I/O or epoll-based event loop:
   ```rust
   // Consider using tokio or mio for event-driven architecture
   // Single thread can monitor multiple devices with epoll
   ```

2. **Device Scan Efficiency** (`main.rs:146-186`)
   - Currently rescans all `/dev/input/event*` every 2 seconds
   - **Recommendation:** Use inotify to watch for device additions:
   ```rust
   // Watch /dev/input for IN_CREATE events instead of polling
   use inotify::{Inotify, WatchMask};
   let mut inotify = Inotify::init()?;
   inotify.watches().add("/dev/input", WatchMask::CREATE)?;
   ```

**P2 - Medium Priority:**

3. **Mutex Contention** (`main.rs:98-103`)
   - GlobalState mutex locked on every hotkey trigger
   - Low contention in practice, but consider `parking_lot::Mutex` for lower overhead

4. **Device Permission Handling** (`main.rs:176-179`)
   - Silent skip on PermissionDenied is correct behavior
   - Add startup check with helpful user guidance

### Feature Recommendations

1. **Configurable Hotkeys**
   - Allow users to remap overlay toggle key
   - Support additional combos (e.g., Start+Select on controller)

2. **Per-Controller Guide Button Behavior**
   - Option to disable guide button on specific controllers
   - Useful when guide button has other game functions

3. **Long-Press Actions**
   - Differentiate short press (toggle overlay) from long press (screenshot/quick save)

4. **macOS/Windows Support**
   - Current implementation is Linux-only (evdev)
   - Consider cross-platform alternatives:
     - macOS: IOKit HID APIs
     - Windows: Raw Input API or XInput

---

## 3. RetroAchievements Component

### Architecture Overview

Location: `ra/src/`

CLI tool and library for RetroAchievements.org integration:
- API client for RA endpoints
- Local SQLite cache for offline viewing
- ROM hashing (console-specific preprocessing)
- Credential management with secure file permissions

### Key Files

| File | Purpose |
|------|---------|
| `api.rs` | HTTP client for RA API (blocking reqwest) |
| `auth.rs` | Credential storage with 0600 permissions |
| `cache.rs` | SQLite cache for games/achievements |
| `hash.rs` | Console-specific ROM hashing (NES/SNES/N64) |
| `types.rs` | API response types and ConsoleId enum |
| `main.rs` | CLI interface (login, status, game-info, etc.) |

### Performance Observations

#### Current Implementation
- Uses `reqwest::blocking::Client` (synchronous HTTP)
- 30-second timeout on all requests
- SQLite cache reduces repeat API calls
- ROM hashing reads entire file into memory

#### Performance Recommendations

**P1 - High Priority:**

1. **Blocking HTTP Client** (`api.rs:9`)
   ```rust
   client: reqwest::blocking::Client,
   ```
   - CLI use is fine, but library use blocks calling thread
   **Recommendation:** Add async API variant for integration with async runtimes:
   ```rust
   pub struct AsyncRAClient {
       client: reqwest::Client, // async version
   }
   ```

2. **ROM Hashing Memory Usage** (`hash.rs:14-16`)
   ```rust
   let mut buffer = Vec::new();
   file.read_to_end(&mut buffer)?;
   ```
   - Reads entire ROM into memory (N64 ROMs can be 64MB+)
   **Recommendation:** Stream-based hashing for large files:
   ```rust
   use md5::{Md5, Digest};
   use std::io::{BufReader, Read};

   let mut hasher = Md5::new();
   let mut reader = BufReader::with_capacity(1024 * 1024, file); // 1MB buffer
   let mut chunk = [0u8; 8192];
   loop {
       let bytes_read = reader.read(&mut chunk)?;
       if bytes_read == 0 { break; }
       hasher.update(&chunk[..bytes_read]);
   }
   ```

**P2 - Medium Priority:**

3. **Cache Database Connection** (`cache.rs:9`)
   - Single connection per RACache instance
   - For concurrent access, consider connection pooling (r2d2-sqlite)

4. **API Response Parsing** (`api.rs:70`)
   - Parses JSON twice (once as text to check for empty, once as struct)
   ```rust
   let text = response.text()?;
   if text == "{}" || text.is_empty() || text.contains("\"ID\":0") {
       return Ok(None);
   }
   let lookup: GameInfoAndProgress = serde_json::from_str(&text)?;
   ```
   **Recommendation:** Parse once with option handling:
   ```rust
   let lookup: Option<GameInfoAndProgress> = response.json().ok();
   match lookup {
       Some(g) if g.id != 0 => Ok(Some(g.id)),
       _ => Ok(None),
   }
   ```

5. **Credential File Watching** (`auth.rs`)
   - Currently reads file on every load
   - Cache credentials in memory with file modification check

### Feature Recommendations

1. **Background Achievement Sync**
   - Periodically sync achievements without blocking game
   - Queue achievement unlocks and retry on network failure

2. **Leaderboard Support**
   - RA API supports leaderboards
   - Display high scores in overlay

3. **Rich Presence**
   - RA Rich Presence shows what you're doing in-game
   - Could display in overlay or Discord integration

4. **Batch Achievement Loading**
   - When game starts, prefetch all badge images
   - Store in cache for instant display on unlock

5. **Console ID Auto-Detection**
   - Currently requires console type parameter
   - Auto-detect from file extension or magic bytes

---

## Cross-Component Recommendations

### IPC Protocol Improvements

1. **Bidirectional Communication**
   - Current: One-way (sender → overlay)
   - Add response channel for status queries

2. **Message Batching**
   - Send multiple achievement unlocks in one message
   - Reduces socket overhead during rapid unlocks

3. **Binary Protocol Option**
   - JSON is human-readable but verbose
   - Consider MessagePack for high-frequency messages

### Testing Infrastructure

1. **Mock RA API Server**
   - Create test fixtures for offline development
   - Faster CI without network dependencies

2. **Input Simulation**
   - Add `evdev-uinput` based testing for input-daemon
   - Automated hotkey testing

3. **Visual Regression Tests**
   - Screenshot-based testing for overlay rendering
   - Catch UI regressions

### Documentation Needs

1. **Architecture Diagram**
   - Show data flow: BIOS → RA CLI → Overlay ← Input-Daemon

2. **IPC Message Reference**
   - Document all `OverlayMessage` variants with examples

3. **Runtime Integration Guide**
   - How emulator wrappers should integrate with RA/Overlay

---

## Summary Priority Matrix

| Priority | Component | Recommendation |
|----------|-----------|----------------|
| P1 | Overlay | Frame timing precision (spin-loop) |
| P1 | Overlay | Reduce CPU when hidden |
| P1 | Input | Use inotify instead of polling |
| P1 | Input | Consider event-driven architecture |
| P1 | RA | Async HTTP client variant |
| P1 | RA | Streaming ROM hashing |
| P2 | Overlay | Toast queue size limit |
| P2 | Overlay | String allocation reduction |
| P2 | Input | parking_lot::Mutex |
| P2 | RA | Single JSON parse |
| P3 | All | Cross-platform input support |
| P3 | All | Bidirectional IPC |
| P3 | RA | Leaderboard support |

---

*Review completed: December 2024*
*Reviewer: Claude Code Analysis*
