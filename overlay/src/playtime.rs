use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

/// Entry for a single game's playtime data
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct PlaytimeEntry {
    pub cart_id: String,
    pub total_seconds: u64,
    pub last_played: Option<u64>,  // Unix timestamp
    pub play_count: u32,
}

/// Database of all game playtimes
#[derive(Serialize, Deserialize, Default, Clone)]
pub struct PlaytimeDatabase {
    pub entries: HashMap<String, PlaytimeEntry>,
}

/// Tracks current game session
pub struct SessionData {
    pub cart_id: String,
    pub start_time: Instant,
}

/// Main playtime tracker
pub struct PlaytimeTracker {
    pub current_session: Option<SessionData>,
    pub database: PlaytimeDatabase,
    db_path: PathBuf,
}

impl PlaytimeTracker {
    /// Create new tracker and load existing database
    pub fn new() -> Result<Self> {
        let db_path = get_playtime_db_path()?;
        let database = Self::load_database(&db_path)?;

        Ok(Self {
            current_session: None,
            database,
            db_path,
        })
    }

    /// Start tracking a new game session
    pub fn start_session(&mut self, cart_id: String) {
        // Save any pending session first
        self.end_session();

        println!("[Playtime] Starting session for: {}", cart_id);
        self.current_session = Some(SessionData {
            cart_id,
            start_time: Instant::now(),
        });
    }

    /// End current session and save playtime
    pub fn end_session(&mut self) {
        if let Some(session) = self.current_session.take() {
            let elapsed_secs = session.start_time.elapsed().as_secs();
            println!("[Playtime] Ending session for {}: {} seconds", session.cart_id, elapsed_secs);

            self.add_playtime(&session.cart_id, elapsed_secs);
            if let Err(e) = self.save_database() {
                eprintln!("[Playtime] Failed to save database: {}", e);
            }
        }
    }

    /// Get playtime for a specific game
    pub fn get_playtime(&self, cart_id: &str) -> Option<&PlaytimeEntry> {
        self.database.entries.get(cart_id)
    }

    /// Add playtime to a game
    fn add_playtime(&mut self, cart_id: &str, seconds: u64) {
        let entry = self.database.entries
            .entry(cart_id.to_string())
            .or_insert_with(|| PlaytimeEntry {
                cart_id: cart_id.to_string(),
                total_seconds: 0,
                last_played: None,
                play_count: 0,
            });

        entry.total_seconds += seconds;
        entry.last_played = Some(current_timestamp());
        entry.play_count += 1;

        println!("[Playtime] Updated {}: total={}s, plays={}", cart_id, entry.total_seconds, entry.play_count);
    }

    /// Load database from disk
    fn load_database(path: &PathBuf) -> Result<PlaytimeDatabase> {
        if !path.exists() {
            println!("[Playtime] No existing database at {:?}, creating new", path);
            return Ok(PlaytimeDatabase::default());
        }

        let json = fs::read_to_string(path)
            .context("Failed to read playtime database")?;

        let db: PlaytimeDatabase = serde_json::from_str(&json)
            .context("Failed to parse playtime database")?;

        println!("[Playtime] Loaded database with {} entries", db.entries.len());
        Ok(db)
    }

    /// Save database to disk
    fn save_database(&self) -> Result<()> {
        let json = serde_json::to_string_pretty(&self.database)
            .context("Failed to serialize playtime database")?;

        fs::write(&self.db_path, json)
            .context("Failed to write playtime database")?;

        println!("[Playtime] Saved database to {:?}", self.db_path);
        Ok(())
    }
}

impl Drop for PlaytimeTracker {
    fn drop(&mut self) {
        // Save any pending session when tracker is dropped
        self.end_session();
    }
}

/// Get path to playtime database
fn get_playtime_db_path() -> Result<PathBuf> {
    let overlay_dir = dirs::home_dir()
        .context("No home directory found")?
        .join(".local/share/kazeta-plus/overlay");

    fs::create_dir_all(&overlay_dir)
        .context("Failed to create overlay directory")?;

    Ok(overlay_dir.join("playtime.json"))
}

/// Get current Unix timestamp
fn current_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread::sleep;

    #[test]
    fn test_session_tracking() {
        let mut tracker = PlaytimeTracker::new().unwrap();

        tracker.start_session("test-game".to_string());
        sleep(Duration::from_secs(2));
        tracker.end_session();

        let entry = tracker.database.entries.get("test-game").unwrap();
        assert!(entry.total_seconds >= 2);
        assert_eq!(entry.play_count, 1);
        assert!(entry.last_played.is_some());
    }

    #[test]
    fn test_multiple_sessions() {
        let mut tracker = PlaytimeTracker::new().unwrap();

        for _ in 0..3 {
            tracker.start_session("test-game".to_string());
            sleep(Duration::from_secs(1));
            tracker.end_session();
        }

        let entry = tracker.database.entries.get("test-game").unwrap();
        assert!(entry.total_seconds >= 3);
        assert_eq!(entry.play_count, 3);
    }

    #[test]
    fn test_multiple_games() {
        let mut tracker = PlaytimeTracker::new().unwrap();

        tracker.start_session("game1".to_string());
        sleep(Duration::from_secs(1));
        tracker.end_session();

        tracker.start_session("game2".to_string());
        sleep(Duration::from_secs(1));
        tracker.end_session();

        assert_eq!(tracker.database.entries.len(), 2);
        assert!(tracker.database.entries.contains_key("game1"));
        assert!(tracker.database.entries.contains_key("game2"));
    }
}
