use anyhow::{Context, Result};
use rusqlite::{Connection, params};
use std::path::PathBuf;
use crate::types::*;

/// Local cache for RetroAchievements data
/// Reduces API calls and enables offline viewing
pub struct RACache {
    conn: Connection,
}

impl RACache {
    pub fn new() -> Result<Self> {
        let cache_dir = dirs::home_dir()
            .context("Could not find home directory")?
            .join(".local/share/kazeta-plus/ra_cache");

        std::fs::create_dir_all(&cache_dir)
            .context("Failed to create cache directory")?;

        let db_path = cache_dir.join("achievements.db");
        let conn = Connection::open(&db_path)
            .context("Failed to open cache database")?;

        let cache = Self { conn };
        cache.init_tables()?;

        Ok(cache)
    }

    fn init_tables(&self) -> Result<()> {
        self.conn.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS games (
                hash TEXT PRIMARY KEY,
                game_id INTEGER NOT NULL,
                title TEXT NOT NULL,
                console_id INTEGER NOT NULL,
                console_name TEXT,
                icon_url TEXT,
                num_achievements INTEGER DEFAULT 0,
                last_updated TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS achievements (
                id INTEGER PRIMARY KEY,
                game_hash TEXT NOT NULL,
                title TEXT NOT NULL,
                description TEXT,
                points INTEGER DEFAULT 0,
                badge_name TEXT,
                display_order INTEGER DEFAULT 0,
                FOREIGN KEY (game_hash) REFERENCES games(hash)
            );

            CREATE TABLE IF NOT EXISTS user_progress (
                achievement_id INTEGER PRIMARY KEY,
                date_earned TEXT,
                date_earned_hardcore TEXT,
                FOREIGN KEY (achievement_id) REFERENCES achievements(id)
            );

            CREATE INDEX IF NOT EXISTS idx_achievements_game ON achievements(game_hash);
            "#
        ).context("Failed to create cache tables")?;

        Ok(())
    }

    /// Store game info in cache
    pub fn cache_game(&self, hash: &str, info: &GameInfoAndProgress) -> Result<()> {
        let now = chrono::Utc::now().to_rfc3339();

        self.conn.execute(
            r#"
            INSERT OR REPLACE INTO games (hash, game_id, title, console_id, console_name, icon_url, num_achievements, last_updated)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
            "#,
            params![
                hash,
                info.id,
                info.title,
                info.console_id,
                info.console_name,
                info.image_icon,
                info.num_achievements,
                now,
            ],
        ).context("Failed to cache game info")?;

        // Cache achievements
        if let Some(ref achievements) = info.achievements {
            for achievement in achievements.values() {
                self.cache_achievement(hash, achievement)?;
            }
        }

        Ok(())
    }

    /// Store achievement info in cache
    fn cache_achievement(&self, game_hash: &str, achievement: &Achievement) -> Result<()> {
        self.conn.execute(
            r#"
            INSERT OR REPLACE INTO achievements (id, game_hash, title, description, points, badge_name, display_order)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
            "#,
            params![
                achievement.id,
                game_hash,
                achievement.title,
                achievement.description,
                achievement.points,
                achievement.badge_name,
                achievement.display_order,
            ],
        ).context("Failed to cache achievement")?;

        // Store user progress
        self.conn.execute(
            r#"
            INSERT OR REPLACE INTO user_progress (achievement_id, date_earned, date_earned_hardcore)
            VALUES (?1, ?2, ?3)
            "#,
            params![
                achievement.id,
                achievement.date_earned,
                achievement.date_earned_hardcore,
            ],
        ).context("Failed to cache user progress")?;

        Ok(())
    }

    /// Get cached game ID for a ROM hash
    pub fn get_game_id(&self, hash: &str) -> Result<Option<u32>> {
        let mut stmt = self.conn.prepare(
            "SELECT game_id FROM games WHERE hash = ?1"
        )?;

        let result = stmt.query_row(params![hash], |row| {
            row.get::<_, u32>(0)
        });

        match result {
            Ok(id) => Ok(Some(id)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    /// Get cached game title for a ROM hash
    pub fn get_game_title(&self, hash: &str) -> Result<Option<String>> {
        let mut stmt = self.conn.prepare(
            "SELECT title FROM games WHERE hash = ?1"
        )?;

        let result = stmt.query_row(params![hash], |row| {
            row.get::<_, String>(0)
        });

        match result {
            Ok(title) => Ok(Some(title)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    /// Get all cached achievements for a game
    pub fn get_achievements(&self, hash: &str) -> Result<Vec<CachedAchievement>> {
        let mut stmt = self.conn.prepare(
            r#"
            SELECT a.id, a.title, a.description, a.points, a.badge_name, a.display_order,
                   p.date_earned, p.date_earned_hardcore
            FROM achievements a
            LEFT JOIN user_progress p ON a.id = p.achievement_id
            WHERE a.game_hash = ?1
            ORDER BY a.display_order
            "#
        )?;

        let achievements = stmt.query_map(params![hash], |row| {
            Ok(CachedAchievement {
                id: row.get(0)?,
                title: row.get(1)?,
                description: row.get(2)?,
                points: row.get(3)?,
                badge_name: row.get(4)?,
                display_order: row.get(5)?,
                date_earned: row.get(6)?,
                date_earned_hardcore: row.get(7)?,
            })
        })?
        .filter_map(|r| r.ok())
        .collect();

        Ok(achievements)
    }

    /// Mark an achievement as earned in the cache
    pub fn mark_earned(&self, achievement_id: u32, hardcore: bool) -> Result<()> {
        let now = chrono::Utc::now().to_rfc3339();

        if hardcore {
            self.conn.execute(
                "UPDATE user_progress SET date_earned_hardcore = ?1 WHERE achievement_id = ?2",
                params![now, achievement_id],
            )?;
        } else {
            self.conn.execute(
                "UPDATE user_progress SET date_earned = ?1 WHERE achievement_id = ?2",
                params![now, achievement_id],
            )?;
        }

        Ok(())
    }

    /// Get achievement count (earned / total) for a game
    pub fn get_progress(&self, hash: &str) -> Result<(u32, u32)> {
        let mut stmt = self.conn.prepare(
            r#"
            SELECT 
                COUNT(*) as total,
                SUM(CASE WHEN p.date_earned IS NOT NULL OR p.date_earned_hardcore IS NOT NULL THEN 1 ELSE 0 END) as earned
            FROM achievements a
            LEFT JOIN user_progress p ON a.id = p.achievement_id
            WHERE a.game_hash = ?1
            "#
        )?;

        let result = stmt.query_row(params![hash], |row| {
            Ok((row.get::<_, u32>(1)?, row.get::<_, u32>(0)?))
        })?;

        Ok(result)
    }

    /// Clear all cached data
    pub fn clear(&self) -> Result<()> {
        self.conn.execute_batch(
            r#"
            DELETE FROM user_progress;
            DELETE FROM achievements;
            DELETE FROM games;
            "#
        )?;
        Ok(())
    }
}

/// Cached achievement data
#[derive(Debug, Clone)]
pub struct CachedAchievement {
    pub id: u32,
    pub title: String,
    pub description: Option<String>,
    pub points: u32,
    pub badge_name: Option<String>,
    pub display_order: u32,
    pub date_earned: Option<String>,
    pub date_earned_hardcore: Option<String>,
}

impl CachedAchievement {
    pub fn is_earned(&self) -> bool {
        self.date_earned.is_some() || self.date_earned_hardcore.is_some()
    }

    pub fn is_earned_hardcore(&self) -> bool {
        self.date_earned_hardcore.is_some()
    }
}

