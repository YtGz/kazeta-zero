// kazeta-ra library
// RetroAchievements integration for Kazeta+

pub mod api;
pub mod auth;
pub mod cache;
pub mod game_names;
pub mod hash;
pub mod types;

pub use api::{RAClient, AsyncRAClient};
pub use auth::{Credentials, CredentialManager};
pub use game_names::{GameNameEntry, GameNameMapping};
pub use hash::{hash_rom, detect_console};
pub use types::*;

