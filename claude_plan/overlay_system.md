# Kazeta Overlay System - Implementation Plan

## Overview

An in-game overlay system for Kazeta-plus that provides Steam-like functionality including global settings, achievement tracking, and toast notifications. The overlay appears on top of running games and can be triggered via hotkey.

## Goals

- ⚙️ **Global Settings**: Controller configuration, audio/video settings accessible during gameplay
- 🏆 **Achievement System**: Track, display, and notify players of unlocked achievements
- 💬 **Toast Messages**: Non-intrusive notifications for system events, achievements, etc.
- 🎮 **Quick Actions**: Save states, game info, exit options
- 🔌 **Game Integration**: Games can trigger achievements and send notifications via IPC

## Architecture

### High-Level Components

```
┌─────────────────────────────────────────────────────┐
│                   Kazeta BIOS                       │
│  - Launches games                                   │
│  - Starts overlay daemon                            │
└─────────────────────────────────────────────────────┘
                      │
                      ├─ spawns ─────┐
                      │               │
         ┌────────────▼──────┐  ┌────▼────────────────┐
         │  Game Process     │  │ kazeta-overlay      │
         │  (VBA-M, mGBA)    │  │ - Monitors hotkey   │
         │                   │  │ - Renders overlay   │
         │  Can send IPC:    │  │ - Handles settings  │
         │  - Achievements   │  │ - Shows toasts      │
         │  - Toast msgs     │◄─┤ - IPC listener     │
         └───────────────────┘  └─────────────────────┘
                                          │
                                          │ reads/writes
                                          ▼
                               ┌──────────────────────┐
                               │ Storage              │
                               │ - Achievements DB    │
                               │ - Settings           │
                               │ - Controller config  │
                               └──────────────────────┘
```

### Technology Stack

- **Rendering**: macroquad (reuse BIOS rendering code)
- **IPC**: Unix domain sockets (`/tmp/kazeta-overlay.sock`)
- **Hotkey Detection**: evdev (Linux) / IOKit (macOS)
- **Window Management**: X11 override-redirect windows (Linux) / NSPanel (macOS)
- **Data Storage**: JSON files in `~/.local/share/kazeta/`

## Component Breakdown

### 1. Overlay Daemon (`kazeta-overlay`)

**Location**: `overlay/` (new crate in workspace)

**Responsibilities**:
- Background service that runs during gameplay
- Monitors for overlay hotkey (Guide button / F12)
- Renders overlay UI on top of game windows
- Listens for IPC messages from games and BIOS
- Manages achievement state and notifications
- Handles global settings persistence

**Key Modules**:

```rust
// overlay/src/main.rs
mod ipc;           // IPC message handling
mod input;         // Hotkey detection
mod rendering;     // Overlay UI rendering
mod achievements;  // Achievement tracking
mod toasts;        // Toast notification queue
mod settings;      // Settings management
mod screens;       // Overlay screen UI components
```

**Startup Flow**:
1. BIOS spawns `kazeta-overlay` as daemon when launching a game
2. Overlay initializes IPC listener on Unix socket
3. Creates transparent overlay window on top of game
4. Enters main loop: input monitoring + IPC handling + rendering

### 2. IPC Protocol

**Socket Path**: `/tmp/kazeta-overlay.sock`

**Message Format** (JSON over Unix socket):

```json
// Achievement Unlock
{
  "type": "unlock_achievement",
  "cart_id": "pokemon-emerald-multi",
  "achievement_id": "catch_first_pokemon",
  "timestamp": 1234567890
}

// Toast Message
{
  "type": "show_toast",
  "message": "Controller connected",
  "icon": "controller",
  "duration_ms": 3000,
  "style": "info" // info, success, warning, error
}

// Trigger Overlay
{
  "type": "show_overlay",
  "screen": "achievements" // main, settings, achievements
}

// Query Status
{
  "type": "get_status"
}

// Response
{
  "type": "status",
  "overlay_visible": false,
  "active_toasts": 1,
  "pending_achievements": 2
}
```

**Rust IPC Implementation**:

```rust
// overlay/src/ipc.rs
use serde::{Deserialize, Serialize};
use std::os::unix::net::UnixListener;

#[derive(Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum OverlayMessage {
    UnlockAchievement {
        cart_id: String,
        achievement_id: String,
        timestamp: u64,
    },
    ShowToast {
        message: String,
        icon: Option<String>,
        duration_ms: u32,
        style: ToastStyle,
    },
    ShowOverlay {
        screen: OverlayScreen,
    },
    GetStatus,
}

#[derive(Serialize, Deserialize)]
pub enum ToastStyle {
    Info,
    Success,
    Warning,
    Error,
}

#[derive(Serialize, Deserialize)]
pub enum OverlayScreen {
    Main,
    Settings,
    Achievements,
}

pub struct IpcServer {
    listener: UnixListener,
}

impl IpcServer {
    pub fn new() -> std::io::Result<Self> {
        let socket_path = "/tmp/kazeta-overlay.sock";
        // Remove stale socket
        let _ = std::fs::remove_file(socket_path);

        let listener = UnixListener::bind(socket_path)?;
        listener.set_nonblocking(true)?;

        Ok(Self { listener })
    }

    pub fn poll_messages(&mut self) -> Vec<OverlayMessage> {
        let mut messages = Vec::new();

        // Accept all pending connections
        while let Ok((mut stream, _)) = self.listener.accept() {
            if let Ok(msg) = serde_json::from_reader(&mut stream) {
                messages.push(msg);
            }
        }

        messages
    }
}
```

### 3. Hotkey Detection

**Linux (evdev)**:
```rust
// overlay/src/input/linux.rs
use evdev::{Device, Key};

pub struct HotkeyMonitor {
    devices: Vec<Device>,
    hotkey: Key,
}

impl HotkeyMonitor {
    pub fn new() -> std::io::Result<Self> {
        let devices = evdev::enumerate()
            .filter(|(_, d)| d.supported_keys().map_or(false, |keys| {
                keys.contains(Key::BTN_MODE) // Guide button
            }))
            .map(|(_, d)| d)
            .collect();

        Ok(Self {
            devices,
            hotkey: Key::BTN_MODE,
        })
    }

    pub fn check_hotkey_pressed(&mut self) -> bool {
        for device in &mut self.devices {
            while let Ok(Some(event)) = device.fetch_events() {
                if event.kind() == evdev::InputEventKind::Key(self.hotkey)
                   && event.value() == 1 {
                    return true;
                }
            }
        }
        false
    }
}
```

**macOS (IOKit)**:
```rust
// overlay/src/input/macos.rs
// Use IOKit to monitor HID events
// Alternative: Use NSEvent global monitoring
```

**Fallback**:
- F12 key via window events
- Configurable hotkey in settings

### 4. Overlay Rendering

**Window Creation**:

```rust
// overlay/src/rendering/window.rs

#[cfg(target_os = "linux")]
pub fn create_overlay_window() -> Result<Window, Box<dyn Error>> {
    use x11rb::connection::Connection;
    use x11rb::protocol::xproto::*;

    let (conn, screen_num) = x11rb::connect(None)?;
    let screen = &conn.setup().roots[screen_num];

    let window_id = conn.generate_id()?;

    // Create transparent overlay window
    conn.create_window(
        x11rb::COPY_DEPTH_FROM_PARENT,
        window_id,
        screen.root,
        0, 0,
        screen.width_in_pixels,
        screen.height_in_pixels,
        0,
        WindowClass::INPUT_OUTPUT,
        screen.root_visual,
        &CreateWindowAux::new()
            .override_redirect(1) // No window decorations
            .event_mask(
                EventMask::EXPOSURE |
                EventMask::KEY_PRESS |
                EventMask::BUTTON_PRESS
            ),
    )?;

    // Make window stay on top
    let atoms = get_atoms(&conn)?;
    conn.change_property32(
        PropMode::REPLACE,
        window_id,
        atoms._NET_WM_STATE,
        AtomEnum::ATOM,
        &[atoms._NET_WM_STATE_ABOVE],
    )?;

    conn.map_window(window_id)?;
    conn.flush()?;

    Ok(Window { conn, window_id })
}

#[cfg(target_os = "macos")]
pub fn create_overlay_window() -> Result<Window, Box<dyn Error>> {
    // Use NSPanel with NSWindowLevelStatusPanel
    // Set window to be transparent and float above all windows
    todo!("Implement macOS overlay window")
}
```

**Rendering with macroquad**:

```rust
// overlay/src/rendering/mod.rs
use macroquad::prelude::*;

pub struct OverlayRenderer {
    visible: bool,
    current_screen: OverlayScreen,
    font: Font,
    scale_factor: f32,
}

impl OverlayRenderer {
    pub async fn new() -> Self {
        let font = load_ttf_font("assets/fonts/NotoSans-Regular.ttf")
            .await
            .unwrap();

        Self {
            visible: false,
            current_screen: OverlayScreen::Main,
            font,
            scale_factor: 1.0,
        }
    }

    pub fn render(&mut self, state: &OverlayState) {
        if !self.visible {
            return;
        }

        // Semi-transparent background
        draw_rectangle(
            0.0,
            0.0,
            screen_width(),
            screen_height(),
            Color::new(0.0, 0.0, 0.0, 0.7),
        );

        match self.current_screen {
            OverlayScreen::Main => self.render_main_menu(state),
            OverlayScreen::Settings => self.render_settings(state),
            OverlayScreen::Achievements => self.render_achievements(state),
        }
    }

    fn render_main_menu(&self, state: &OverlayState) {
        let menu_x = screen_width() / 2.0 - 300.0;
        let menu_y = screen_height() / 2.0 - 200.0;

        // Menu box
        draw_rectangle(menu_x, menu_y, 600.0, 400.0, DARKGRAY);
        draw_rectangle_lines(menu_x, menu_y, 600.0, 400.0, 2.0, WHITE);

        // Title
        draw_text_ex(
            "KAZETA OVERLAY",
            menu_x + 20.0,
            menu_y + 40.0,
            TextParams {
                font: self.font,
                font_size: 32,
                color: WHITE,
                ..Default::default()
            },
        );

        // Menu options
        let options = ["Settings", "Achievements", "Quick Save", "Resume Game"];
        for (i, option) in options.iter().enumerate() {
            let y = menu_y + 100.0 + (i as f32 * 50.0);
            draw_text_ex(
                option,
                menu_x + 40.0,
                y,
                TextParams {
                    font: self.font,
                    font_size: 24,
                    color: if i == state.selected_option {
                        YELLOW
                    } else {
                        WHITE
                    },
                    ..Default::default()
                },
            );
        }
    }
}
```

### 5. Achievement System

**Data Structures**:

```rust
// overlay/src/achievements.rs
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Serialize, Deserialize, Clone)]
pub struct Achievement {
    pub id: String,
    pub name: String,
    pub description: String,
    pub icon: String,
    pub hidden: bool,
    pub points: u32,
}

#[derive(Serialize, Deserialize)]
pub struct AchievementProgress {
    pub cart_id: String,
    pub unlocked: HashMap<String, UnlockedAchievement>,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct UnlockedAchievement {
    pub achievement_id: String,
    pub unlock_time: u64,
    pub shown: bool, // Has the notification been shown?
}

pub struct AchievementManager {
    definitions: HashMap<String, Vec<Achievement>>,
    progress: HashMap<String, AchievementProgress>,
    save_path: PathBuf,
}

impl AchievementManager {
    pub fn new() -> Result<Self, Box<dyn Error>> {
        let save_path = dirs::data_local_dir()
            .ok_or("Could not get data directory")?
            .join("kazeta")
            .join("achievements");

        fs::create_dir_all(&save_path)?;

        Ok(Self {
            definitions: HashMap::new(),
            progress: HashMap::new(),
            save_path,
        })
    }

    pub fn load_definitions(&mut self, cart_id: &str, path: &Path) -> Result<(), Box<dyn Error>> {
        // Load achievements from cartridge.toml
        let content = fs::read_to_string(path)?;
        let toml: toml::Value = toml::from_str(&content)?;

        if let Some(achievements) = toml.get("achievements").and_then(|a| a.as_array()) {
            let mut defs = Vec::new();
            for ach in achievements {
                let achievement = Achievement {
                    id: ach.get("id").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                    name: ach.get("name").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                    description: ach.get("description").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                    icon: ach.get("icon").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                    hidden: ach.get("hidden").and_then(|v| v.as_bool()).unwrap_or(false),
                    points: ach.get("points").and_then(|v| v.as_integer()).unwrap_or(10) as u32,
                };
                defs.push(achievement);
            }
            self.definitions.insert(cart_id.to_string(), defs);
        }

        Ok(())
    }

    pub fn load_progress(&mut self, cart_id: &str) -> Result<(), Box<dyn Error>> {
        let path = self.save_path.join(format!("{}.json", cart_id));

        if path.exists() {
            let content = fs::read_to_string(&path)?;
            let progress: AchievementProgress = serde_json::from_str(&content)?;
            self.progress.insert(cart_id.to_string(), progress);
        } else {
            // Create new progress
            let progress = AchievementProgress {
                cart_id: cart_id.to_string(),
                unlocked: HashMap::new(),
            };
            self.progress.insert(cart_id.to_string(), progress);
        }

        Ok(())
    }

    pub fn unlock_achievement(&mut self, cart_id: &str, achievement_id: &str) -> Result<bool, Box<dyn Error>> {
        let progress = self.progress.get_mut(cart_id)
            .ok_or("Cart ID not loaded")?;

        // Check if already unlocked
        if progress.unlocked.contains_key(achievement_id) {
            return Ok(false);
        }

        // Unlock it
        let unlocked = UnlockedAchievement {
            achievement_id: achievement_id.to_string(),
            unlock_time: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)?
                .as_secs(),
            shown: false,
        };

        progress.unlocked.insert(achievement_id.to_string(), unlocked);

        // Save progress
        self.save_progress(cart_id)?;

        Ok(true)
    }

    pub fn save_progress(&self, cart_id: &str) -> Result<(), Box<dyn Error>> {
        let progress = self.progress.get(cart_id)
            .ok_or("Cart ID not loaded")?;

        let path = self.save_path.join(format!("{}.json", cart_id));
        let content = serde_json::to_string_pretty(progress)?;
        fs::write(&path, content)?;

        Ok(())
    }

    pub fn get_unshown_achievements(&mut self, cart_id: &str) -> Vec<Achievement> {
        let mut result = Vec::new();

        if let Some(progress) = self.progress.get_mut(cart_id) {
            if let Some(definitions) = self.definitions.get(cart_id) {
                for unlocked in progress.unlocked.values_mut() {
                    if !unlocked.shown {
                        if let Some(def) = definitions.iter().find(|a| a.id == unlocked.achievement_id) {
                            result.push(def.clone());
                            unlocked.shown = true;
                        }
                    }
                }
            }
        }

        result
    }
}
```

**Achievement Definition in cartridge.toml**:

```toml
# test_game/cartridge.toml
name = "Pokemon Emerald"
id = "pokemon-emerald-multi"
version = "1.0"

exec = "pokemon.gba"
runtime = "vba-m"

[[achievements]]
id = "catch_first_pokemon"
name = "Gotta Start Somewhere"
description = "Caught your first Pokemon"
icon = "achievements/first_catch.png"
points = 10

[[achievements]]
id = "beat_first_gym"
name = "Stone Badge"
description = "Defeated Roxanne at Rustboro Gym"
icon = "achievements/stone_badge.png"
points = 25

[[achievements]]
id = "complete_pokedex"
name = "Pokemon Master"
description = "Completed the Hoenn Pokedex"
icon = "achievements/pokedex.png"
points = 100
hidden = true
```

### 6. Toast Notification System

**Toast Queue**:

```rust
// overlay/src/toasts.rs
use std::collections::VecDeque;
use std::time::{Duration, Instant};

pub struct Toast {
    pub message: String,
    pub icon: Option<String>,
    pub style: ToastStyle,
    pub created_at: Instant,
    pub duration: Duration,
}

pub struct ToastManager {
    queue: VecDeque<Toast>,
    max_visible: usize,
}

impl ToastManager {
    pub fn new() -> Self {
        Self {
            queue: VecDeque::new(),
            max_visible: 3,
        }
    }

    pub fn add_toast(&mut self, message: String, icon: Option<String>, style: ToastStyle, duration_ms: u32) {
        let toast = Toast {
            message,
            icon,
            style,
            created_at: Instant::now(),
            duration: Duration::from_millis(duration_ms as u64),
        };
        self.queue.push_back(toast);
    }

    pub fn update(&mut self) {
        // Remove expired toasts
        let now = Instant::now();
        self.queue.retain(|toast| {
            now.duration_since(toast.created_at) < toast.duration
        });
    }

    pub fn get_visible_toasts(&self) -> Vec<&Toast> {
        self.queue.iter().take(self.max_visible).collect()
    }

    pub fn render(&self, font: Font) {
        let toasts = self.get_visible_toasts();
        let base_y = 100.0;
        let toast_height = 60.0;
        let toast_width = 400.0;
        let x = screen_width() - toast_width - 20.0;

        for (i, toast) in toasts.iter().enumerate() {
            let y = base_y + (i as f32 * (toast_height + 10.0));

            // Calculate fade based on remaining time
            let elapsed = Instant::now().duration_since(toast.created_at);
            let remaining = toast.duration.saturating_sub(elapsed);
            let alpha = if remaining < Duration::from_millis(500) {
                remaining.as_millis() as f32 / 500.0
            } else {
                1.0
            };

            // Background color based on style
            let bg_color = match toast.style {
                ToastStyle::Info => Color::new(0.2, 0.4, 0.8, 0.9 * alpha),
                ToastStyle::Success => Color::new(0.2, 0.8, 0.4, 0.9 * alpha),
                ToastStyle::Warning => Color::new(0.9, 0.7, 0.2, 0.9 * alpha),
                ToastStyle::Error => Color::new(0.9, 0.2, 0.2, 0.9 * alpha),
            };

            // Draw toast background
            draw_rectangle(x, y, toast_width, toast_height, bg_color);
            draw_rectangle_lines(x, y, toast_width, toast_height, 2.0,
                Color::new(1.0, 1.0, 1.0, alpha));

            // Draw icon if present
            let text_x = if toast.icon.is_some() {
                x + 60.0
            } else {
                x + 15.0
            };

            // Draw message
            draw_text_ex(
                &toast.message,
                text_x,
                y + 35.0,
                TextParams {
                    font,
                    font_size: 20,
                    color: Color::new(1.0, 1.0, 1.0, alpha),
                    ..Default::default()
                },
            );
        }
    }
}
```

### 7. Settings Management

**Settings Structure**:

```rust
// overlay/src/settings.rs
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone)]
pub struct OverlaySettings {
    pub hotkey: String, // "guide", "f12", etc.
    pub show_fps: bool,
    pub show_notifications: bool,
    pub achievement_sound: bool,
    pub toast_duration_ms: u32,
    pub controller_mappings: Vec<ControllerMapping>,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct ControllerMapping {
    pub name: String,
    pub device_id: String,
    pub mappings: HashMap<String, String>,
}

impl Default for OverlaySettings {
    fn default() -> Self {
        Self {
            hotkey: "guide".to_string(),
            show_fps: true,
            show_notifications: true,
            achievement_sound: true,
            toast_duration_ms: 3000,
            controller_mappings: Vec::new(),
        }
    }
}

impl OverlaySettings {
    pub fn load() -> Result<Self, Box<dyn Error>> {
        let path = dirs::config_dir()
            .ok_or("Could not get config directory")?
            .join("kazeta")
            .join("overlay.json");

        if path.exists() {
            let content = fs::read_to_string(&path)?;
            Ok(serde_json::from_str(&content)?)
        } else {
            Ok(Self::default())
        }
    }

    pub fn save(&self) -> Result<(), Box<dyn Error>> {
        let path = dirs::config_dir()
            .ok_or("Could not get config directory")?
            .join("kazeta")
            .join("overlay.json");

        fs::create_dir_all(path.parent().unwrap())?;
        let content = serde_json::to_string_pretty(self)?;
        fs::write(&path, content)?;

        Ok(())
    }
}
```

## Implementation Phases

### Phase 1: Core Infrastructure (Week 1)
**Goal**: Basic overlay that can be toggled with hotkey

**Tasks**:
1. ✅ Create `overlay/` workspace crate
2. ✅ Implement hotkey detection (evdev on Linux)
3. ✅ Create transparent overlay window
4. ✅ Basic macroquad rendering setup
5. ✅ Simple menu UI (Main, Settings, Resume)
6. ✅ IPC server setup (Unix socket)

**Deliverable**: Overlay that can be triggered with Guide button, shows a simple menu, and can be dismissed.

**Testing**:
```bash
# Terminal 1: Start overlay daemon
cargo run --bin kazeta-overlay

# Terminal 2: Test IPC
echo '{"type":"show_overlay","screen":"main"}' | nc -U /tmp/kazeta-overlay.sock

# Terminal 3: Launch game (should see overlay when pressing Guide button)
cargo run --bin kazeta-bios
```

### Phase 2: Toast Notifications (Week 2)
**Goal**: Display non-intrusive notifications

**Tasks**:
1. ✅ Implement toast queue and rendering
2. ✅ Add fade-in/fade-out animations
3. ✅ Support different toast styles (info, success, warning, error)
4. ✅ IPC endpoint for triggering toasts
5. ✅ Test from game processes

**Deliverable**: Toast notifications that appear in corner of screen, stack vertically, and auto-dismiss.

**Testing**:
```bash
# Send test toast
echo '{"type":"show_toast","message":"Controller connected","style":"success","duration_ms":3000}' | nc -U /tmp/kazeta-overlay.sock
```

### Phase 3: Achievement System (Week 3-4)
**Goal**: Full achievement tracking and notifications

**Tasks**:
1. ✅ Achievement data structures and storage
2. ✅ Load achievement definitions from cartridge.toml
3. ✅ Track unlocked achievements per cart
4. ✅ Achievement unlock notifications (animated popup)
5. ✅ Achievement review screen in overlay
6. ✅ Progress tracking (X/Y achievements unlocked)

**Deliverable**: Games can define achievements in cartridge.toml, trigger unlocks via IPC, and players can view progress in overlay.

**Testing**:
```toml
# test_game/cartridge.toml
[[achievements]]
id = "test_achievement"
name = "Test Achievement"
description = "This is a test"
icon = "test.png"
points = 10
```

```bash
# Unlock achievement
echo '{"type":"unlock_achievement","cart_id":"test-game","achievement_id":"test_achievement","timestamp":1234567890}' | nc -U /tmp/kazeta-overlay.sock
```

### Phase 4: Settings UI (Week 5-6)
**Goal**: Controller configuration and global settings

**Tasks**:
1. ✅ Settings screen UI
2. ✅ Controller detection and enumeration
3. ✅ Button mapping interface
4. ✅ Save/load controller mappings
5. ✅ Apply mappings globally (via evdev remapping or SDL_GAMECONTROLLERCONFIG)
6. ✅ Other settings: hotkey config, notification preferences

**Deliverable**: Users can configure controllers in overlay, mappings persist and apply to all games.

**Testing**:
- Press Guide button during game
- Navigate to Settings → Controller Configuration
- Connect controller and map buttons
- Test in game

### Phase 5: Integration & Polish (Week 7)
**Goal**: Integration with BIOS and games

**Tasks**:
1. ✅ BIOS automatically spawns overlay daemon when launching games
2. ✅ BIOS passes cart metadata to overlay
3. ✅ Games can easily send IPC messages (helper library)
4. ✅ Sound effects for achievements
5. ✅ Smooth animations and transitions
6. ✅ Error handling and recovery

**Deliverable**: Fully integrated overlay system that "just works" when games launch.

## File Structure

```
kazeta-plus/
├── bios/                    # Existing BIOS
│   └── src/
│       └── save.rs          # Modified to spawn overlay
├── overlay/                 # New overlay crate
│   ├── Cargo.toml
│   ├── src/
│   │   ├── main.rs         # Main entry point
│   │   ├── ipc.rs          # IPC server
│   │   ├── input.rs        # Hotkey detection
│   │   ├── rendering/
│   │   │   ├── mod.rs
│   │   │   ├── window.rs   # Platform-specific window creation
│   │   │   └── ui.rs       # UI rendering
│   │   ├── achievements.rs # Achievement tracking
│   │   ├── toasts.rs       # Toast notifications
│   │   ├── settings.rs     # Settings management
│   │   └── screens/        # Overlay screen UIs
│   │       ├── main_menu.rs
│   │       ├── settings.rs
│   │       └── achievements.rs
│   └── assets/             # Overlay assets
│       ├── fonts/
│       └── icons/
├── overlay-client/         # IPC client library for games
│   ├── Cargo.toml
│   └── src/
│       └── lib.rs          # Helper functions for sending IPC
└── rootfs/
    └── usr/
        └── bin/
            └── kazeta-overlay  # Installed overlay binary
```

## API Examples

### For Game Developers

**Unlock Achievement**:
```rust
// In game code (e.g., VBA-M lua script)
use kazeta_overlay_client::OverlayClient;

let client = OverlayClient::new()?;
client.unlock_achievement("pokemon-emerald", "catch_first_pokemon")?;
```

**Show Toast**:
```rust
client.show_toast(
    "New item obtained: Rare Candy",
    Some("item_icon.png"),
    ToastStyle::Info,
    3000
)?;
```

**Bash/Script Integration**:
```bash
# From wrapper scripts
echo '{"type":"show_toast","message":"Game saved","style":"success","duration_ms":2000}' | nc -U /tmp/kazeta-overlay.sock
```

## Testing Strategy

### Unit Tests
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_achievement_unlock() {
        let mut manager = AchievementManager::new().unwrap();
        manager.load_definitions("test", Path::new("test.toml")).unwrap();

        assert!(manager.unlock_achievement("test", "test_ach").unwrap());
        assert!(!manager.unlock_achievement("test", "test_ach").unwrap()); // Already unlocked
    }

    #[test]
    fn test_toast_expiration() {
        let mut manager = ToastManager::new();
        manager.add_toast("Test".to_string(), None, ToastStyle::Info, 100);

        assert_eq!(manager.get_visible_toasts().len(), 1);

        std::thread::sleep(Duration::from_millis(150));
        manager.update();

        assert_eq!(manager.get_visible_toasts().len(), 0);
    }
}
```

### Integration Tests
1. **IPC Communication**: Verify messages can be sent and received
2. **Hotkey Detection**: Test with different input devices
3. **Achievement Persistence**: Verify achievements save/load correctly
4. **Multi-process**: Test overlay + BIOS + game running simultaneously

### Manual Testing Checklist
- [ ] Overlay appears when hotkey pressed
- [ ] Overlay dismisses when hotkey pressed again
- [ ] Toasts appear and auto-dismiss
- [ ] Multiple toasts stack correctly
- [ ] Achievements unlock and show notification
- [ ] Achievement screen shows all achievements
- [ ] Settings persist across sessions
- [ ] Controller mapping works in-game
- [ ] No performance impact on gameplay
- [ ] Works with multiple games/emulators

## Performance Considerations

1. **Rendering Overhead**:
   - Only render when overlay is visible or toasts active
   - Use VSync to avoid excessive CPU usage
   - Target: < 1ms render time when visible, ~0ms when hidden

2. **IPC Overhead**:
   - Non-blocking Unix socket polling
   - Batch message processing
   - Target: < 0.1ms per message

3. **Input Monitoring**:
   - Only monitor relevant devices (controllers with Guide button)
   - Use evdev non-blocking mode
   - Target: < 0.5ms per poll cycle

4. **Memory Usage**:
   - Keep achievement definitions cached
   - Limit toast queue size
   - Target: < 50MB RAM total

## Security Considerations

1. **Socket Permissions**:
   - Unix socket should be user-only (chmod 600)
   - Validate all incoming messages
   - Rate-limit IPC messages to prevent spam

2. **Input Injection**:
   - Overlay should not interfere with game input
   - Use separate input contexts

3. **File System**:
   - Validate all file paths
   - Sanitize cart IDs used in file names
   - Protect achievement data from tampering (checksums)

## Future Enhancements

### Phase 6+: Advanced Features

1. **Save State Management**:
   - Quick save/load via overlay
   - Save state thumbnails
   - Save state browser

2. **Performance Overlay**:
   - FPS counter
   - CPU/GPU usage
   - Frame time graph
   - Temperature monitoring

3. **Social Features**:
   - Achievement comparison with friends
   - Screenshot sharing
   - Leaderboards

4. **Game Library Integration**:
   - View game info from overlay
   - Launch different game
   - Recent games list

5. **Theme System**:
   - Customizable overlay themes
   - Per-game themes
   - Community theme sharing

6. **Voice Chat**:
   - Built-in voice chat for multiplayer
   - Push-to-talk via overlay

7. **Streaming Integration**:
   - Twitch/YouTube streaming controls
   - Chat overlay
   - Clip capture

## Platform-Specific Notes

### Linux (Primary Target)
- Use evdev for input
- X11 override-redirect windows for overlay
- SDL2 for audio (achievement sounds)
- Works with both X11 and Wayland

### macOS (Dev/Testing)
- IOKit for input monitoring
- NSPanel with NSWindowLevelStatusPanel for overlay
- CoreAudio for sounds
- May need code signing for input monitoring

### Windows (Future)
- Raw Input API for hotkeys
- DirectX overlay or layered windows
- XInput for controller support

## Dependencies

```toml
# overlay/Cargo.toml
[dependencies]
macroquad = "0.4"
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
toml = "0.8"
dirs = "5.0"

# Linux-specific
[target.'cfg(target_os = "linux")'.dependencies]
evdev = "0.12"
x11rb = "0.13"

# macOS-specific
[target.'cfg(target_os = "macos")'.dependencies]
# TBD: IOKit bindings
```

## Build & Installation

### Development Build
```bash
# Build overlay
cd overlay
cargo build --release

# Copy to bin
cp target/release/kazeta-overlay ~/.local/bin/

# Test
kazeta-overlay &
```

### Production Build (rootfs)
```bash
# Build for Arch Linux target
cargo build --release --target x86_64-unknown-linux-gnu

# Install to rootfs
cp target/x86_64-unknown-linux-gnu/release/kazeta-overlay rootfs/usr/bin/

# Build ISO
./build.sh
```

## Documentation

### User Documentation
- How to use the overlay (hotkey, navigation)
- Achievement system guide
- Controller configuration guide
- Troubleshooting common issues

### Developer Documentation
- How to define achievements in cartridge.toml
- IPC protocol specification
- Example code for sending notifications
- Best practices for achievement design

## Success Metrics

1. **Performance**: No noticeable impact on game performance
2. **Usability**: < 5 seconds to access any overlay feature
3. **Reliability**: No crashes in 100 hours of gameplay
4. **Adoption**: Achievement definitions in >50% of games
5. **User Satisfaction**: Positive feedback from beta testers

## Conclusion

This overlay system will significantly enhance the Kazeta gaming experience by providing Steam-like functionality with achievements, notifications, and global settings. The phased approach allows for incremental development and testing, with each phase delivering tangible value.

The architecture is designed to be:
- **Performant**: Minimal overhead on gameplay
- **Extensible**: Easy to add new features
- **Cross-platform**: Linux primary, macOS/Windows future
- **Developer-friendly**: Simple IPC protocol for game integration

Next steps:
1. Review and approve this plan
2. Begin Phase 1 implementation
3. Regular check-ins and iteration based on feedback
