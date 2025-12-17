use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::io::{BufRead, BufReader};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::Path;

const SOCKET_PATH: &str = "/tmp/kazeta-overlay.sock";

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum OverlayMessage {
    UnlockAchievement {
        cart_id: String,
        achievement_id: String,
        timestamp: u64,
    },
    ShowToast {
        message: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        icon: Option<String>,
        duration_ms: u32,
        style: ToastStyle,
    },
    ShowOverlay {
        screen: OverlayScreen,
    },
    HideOverlay,
    GetStatus,
    SetTheme {
        font_color: String,
        cursor_color: String,
    },
    // RetroAchievements messages
    RaGameStart {
        game_title: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        game_id: Option<u32>,
        #[serde(skip_serializing_if = "Option::is_none")]
        game_icon: Option<String>,
        total_achievements: u32,
        earned_achievements: u32,
    },
    RaAchievementUnlocked {
        achievement_id: u32,
        title: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        description: Option<String>,
        points: u32,
        #[serde(skip_serializing_if = "Option::is_none")]
        icon_url: Option<String>,
        #[serde(default)]
        is_hardcore: bool,
    },
    RaProgressUpdate {
        earned: u32,
        total: u32,
    },
    /// Full achievement list for the current game
    RaAchievementList {
        game_title: String,
        game_hash: String,
        achievements: Vec<AchievementInfo>,
    },
    /// Toggle overlay visibility (from input daemon)
    ToggleOverlay,
    /// Notify that a game has started
    GameStarted {
        cart_id: String,
        game_name: String,
        runtime: String,
    },
    /// Notify that a game has stopped
    GameStopped {
        cart_id: String,
    },
    /// Request to quit the current game and return to BIOS
    QuitGame,
    /// Response confirming game quit was initiated
    QuitGameAck,
}

/// Achievement information for display
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct AchievementInfo {
    pub id: u32,
    pub title: String,
    pub description: String,
    pub points: u32,
    #[serde(default)]
    pub earned: bool,
    #[serde(default)]
    pub earned_hardcore: bool,

    // Optional fields for enhanced features
    #[serde(default)]
    pub rarity_percent: Option<f32>,  // 0-100, percentage of players who earned it

    #[serde(default)]
    pub earned_at: Option<u64>,  // Unix timestamp when earned

    #[serde(default)]
    pub progress: Option<AchievementProgress>,  // For multi-step achievements
}

/// Progress tracking for multi-step achievements
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct AchievementProgress {
    pub current: u32,
    pub target: u32,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy)]
#[serde(rename_all = "snake_case")]
pub enum ToastStyle {
    Info,
    Success,
    Warning,
    Error,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum OverlayScreen {
    Main,
    Settings,
    Achievements,
    Performance,        // Performance monitoring
    Playtime,           // Playtime tracking
    // Controller menu screens
    Controllers,        // Main controller menu
    BluetoothPairing,   // Find and pair Bluetooth controllers
    ControllerAssign,   // Assign controllers to players
    GamepadTester,      // Test gamepad inputs
    HotkeySettings,     // Configure hotkey bindings
    // Menu customization
    MenuCustomization,  // Customize main menu items
    ThemeSelection,     // Select overlay theme
    // Quit confirmation
    QuitConfirm,        // Confirm quit to BIOS
}

pub struct IpcServer {
    listener: UnixListener,
}

impl IpcServer {
    pub fn new() -> Result<Self> {
        // Remove stale socket if it exists
        if Path::new(SOCKET_PATH).exists() {
            std::fs::remove_file(SOCKET_PATH)
                .context("Failed to remove stale socket")?;
        }

        let listener = UnixListener::bind(SOCKET_PATH)
            .context("Failed to bind Unix socket")?;

        // Set non-blocking mode
        listener
            .set_nonblocking(true)
            .context("Failed to set non-blocking mode")?;

        println!("[IPC] Server listening on {}", SOCKET_PATH);

        Ok(Self { listener })
    }

    pub fn poll_messages(&mut self) -> Vec<OverlayMessage> {
        let mut messages = Vec::new();

        // Accept all pending connections
        loop {
            match self.listener.accept() {
                Ok((stream, _)) => {
                    if let Some(msg) = Self::read_message(stream) {
                        messages.push(msg);
                    }
                }
                Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    // No more connections to accept
                    break;
                }
                Err(e) => {
                    eprintln!("[IPC] Error accepting connection: {}", e);
                    break;
                }
            }
        }

        messages
    }

    fn read_message(stream: UnixStream) -> Option<OverlayMessage> {
        let reader = BufReader::new(stream);

        for line in reader.lines() {
            match line {
                Ok(line) => {
                    match serde_json::from_str::<OverlayMessage>(&line) {
                        Ok(msg) => {
                            println!("[IPC] Received message: {:?}", msg);
                            return Some(msg);
                        }
                        Err(e) => {
                            eprintln!("[IPC] Failed to parse message: {} - Error: {}", line, e);
                        }
                    }
                }
                Err(e) => {
                    eprintln!("[IPC] Error reading line: {}", e);
                    break;
                }
            }
        }

        None
    }
}

impl Drop for IpcServer {
    fn drop(&mut self) {
        // Clean up socket on exit
        let _ = std::fs::remove_file(SOCKET_PATH);
        println!("[IPC] Cleaned up socket");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn test_ipc_message_serialization() {
        let msg = OverlayMessage::ShowToast {
            message: "Test message".to_string(),
            icon: None,
            duration_ms: 3000,
            style: ToastStyle::Info,
        };

        let json = serde_json::to_string(&msg).unwrap();
        let parsed: OverlayMessage = serde_json::from_str(&json).unwrap();

        match parsed {
            OverlayMessage::ShowToast { message, .. } => {
                assert_eq!(message, "Test message");
            }
            _ => panic!("Wrong message type"),
        }
    }
}
