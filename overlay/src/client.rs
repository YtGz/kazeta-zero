use crate::ipc::{OverlayMessage, OverlayScreen, ToastStyle};
use anyhow::{Context, Result};
use std::io::Write;
use std::os::unix::net::UnixStream;

const SOCKET_PATH: &str = "/tmp/kazeta-overlay.sock";

/// Client for sending messages to the overlay daemon
pub struct OverlayClient {
    socket_path: String,
}

impl OverlayClient {
    /// Create a new overlay client
    pub fn new() -> Self {
        Self {
            socket_path: SOCKET_PATH.to_string(),
        }
    }

    /// Create a client with a custom socket path
    pub fn with_socket_path(socket_path: String) -> Self {
        Self { socket_path }
    }

    /// Check if the overlay daemon is running
    /// Actually tries to connect to verify the daemon is responsive
    pub fn is_available(&self) -> bool {
        use std::os::unix::net::UnixStream;
        use std::time::Duration;

        // First check if socket file exists
        if !std::path::Path::new(&self.socket_path).exists() {
            return false;
        }

        // Try to actually connect to verify daemon is running
        // Use a short timeout to avoid blocking
        match UnixStream::connect(&self.socket_path) {
            Ok(stream) => {
                // Set a short timeout and try to read (daemon should be responsive)
                let _ = stream.set_read_timeout(Some(Duration::from_millis(100)));
                true
            }
            Err(_) => {
                // Socket exists but can't connect - daemon not running
                // Clean up the stale socket
                let _ = std::fs::remove_file(&self.socket_path);
                false
            }
        }
    }

    /// Send a message to the overlay
    fn send_message(&self, message: &OverlayMessage) -> Result<()> {
        let mut stream = UnixStream::connect(&self.socket_path)
            .context("Failed to connect to overlay daemon")?;

        let json = serde_json::to_string(message)
            .context("Failed to serialize message")?;

        stream
            .write_all(json.as_bytes())
            .context("Failed to write message")?;
        stream
            .write_all(b"\n")
            .context("Failed to write newline")?;
        stream.flush().context("Failed to flush stream")?;

        Ok(())
    }

    /// Show a toast notification
    pub fn show_toast(
        &self,
        message: impl Into<String>,
        style: ToastStyle,
        duration_ms: u32,
    ) -> Result<()> {
        self.send_message(&OverlayMessage::ShowToast {
            message: message.into(),
            icon: None,
            duration_ms,
            style,
        })
    }

    /// Show a toast notification with an icon
    pub fn show_toast_with_icon(
        &self,
        message: impl Into<String>,
        icon: impl Into<String>,
        style: ToastStyle,
        duration_ms: u32,
    ) -> Result<()> {
        self.send_message(&OverlayMessage::ShowToast {
            message: message.into(),
            icon: Some(icon.into()),
            duration_ms,
            style,
        })
    }

    /// Show an info toast
    pub fn info(&self, message: impl Into<String>) -> Result<()> {
        self.show_toast(message, ToastStyle::Info, 3000)
    }

    /// Show a success toast
    pub fn success(&self, message: impl Into<String>) -> Result<()> {
        self.show_toast(message, ToastStyle::Success, 3000)
    }

    /// Show a warning toast
    pub fn warning(&self, message: impl Into<String>) -> Result<()> {
        self.show_toast(message, ToastStyle::Warning, 4000)
    }

    /// Show an error toast
    pub fn error(&self, message: impl Into<String>) -> Result<()> {
        self.show_toast(message, ToastStyle::Error, 5000)
    }

    /// Show the overlay menu
    pub fn show_overlay(&self, screen: OverlayScreen) -> Result<()> {
        self.send_message(&OverlayMessage::ShowOverlay { screen })
    }

    /// Hide the overlay menu
    pub fn hide_overlay(&self) -> Result<()> {
        self.send_message(&OverlayMessage::HideOverlay)
    }

    /// Unlock an achievement
    pub fn unlock_achievement(
        &self,
        cart_id: impl Into<String>,
        achievement_id: impl Into<String>,
    ) -> Result<()> {
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        self.send_message(&OverlayMessage::UnlockAchievement {
            cart_id: cart_id.into(),
            achievement_id: achievement_id.into(),
            timestamp,
        })
    }

    /// Request status from the overlay
    pub fn get_status(&self) -> Result<()> {
        self.send_message(&OverlayMessage::GetStatus)
    }

    /// Set the overlay theme colors
    pub fn set_theme(
        &self,
        font_color: impl Into<String>,
        cursor_color: impl Into<String>,
    ) -> Result<()> {
        self.send_message(&OverlayMessage::SetTheme {
            font_color: font_color.into(),
            cursor_color: cursor_color.into(),
        })
    }
}

impl Default for OverlayClient {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_client_creation() {
        let client = OverlayClient::new();
        assert_eq!(client.socket_path, SOCKET_PATH);
    }

    #[test]
    fn test_custom_socket_path() {
        let custom_path = "/tmp/custom-overlay.sock";
        let client = OverlayClient::with_socket_path(custom_path.to_string());
        assert_eq!(client.socket_path, custom_path);
    }
}
