// Re-export the client and IPC types for external use
pub mod client;
pub mod ipc;

pub use client::OverlayClient;
pub use ipc::{OverlayMessage, OverlayScreen, ToastStyle, AchievementInfo};
