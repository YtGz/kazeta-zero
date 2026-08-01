// kazeta-ra library
// RetroAchievements integration for Kazeta Zero

pub mod api;
pub mod auth;
pub mod cache;
pub mod evaluation;
pub mod game_names;
pub mod hash;
pub mod local_definitions;
pub mod rcheevos_ffi;
pub mod types;

pub use api::{AsyncRAClient, RAClient};
pub use auth::{CredentialManager, Credentials};
pub use evaluation::EvaluationEngine;
pub use game_names::{GameNameEntry, GameNameMapping};
pub use hash::{detect_console, hash_rom};
pub use local_definitions::LocalDefinitions;
pub use rcheevos_ffi::{hash_rom_rcheevos, Runtime as RcRuntime};
pub use types::*;
