use serde::{Deserialize, Serialize};
use std::fmt;

fn default_achievement_type() -> String {
    "standard".to_string()
}

/// Console IDs as defined by RetroAchievements
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u32)]
pub enum ConsoleId {
    MegaDrive = 1,
    Nintendo64 = 2,
    SNES = 3,
    GameBoy = 4,
    GameBoyAdvance = 5,
    GameBoyColor = 6,
    NES = 7,
    MasterSystem = 11,
    PlayStation = 12,
    NintendoDS = 18,
    PlayStation2 = 21,
    Atari2600 = 25,
    VirtualBoy = 28,
    GameCube = 16,
    Wii = 19,
}

impl ConsoleId {
    pub fn parse(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "gba" | "gameboy advance" | "game boy advance" => Some(Self::GameBoyAdvance),
            "gb" | "gameboy" | "game boy" => Some(Self::GameBoy),
            "gbc" | "gameboy color" | "game boy color" => Some(Self::GameBoyColor),
            "nes" | "famicom" => Some(Self::NES),
            "snes" | "super nintendo" | "super famicom" => Some(Self::SNES),
            "n64" | "nintendo 64" => Some(Self::Nintendo64),
            "psx" | "ps1" | "playstation" => Some(Self::PlayStation),
            "ps2" | "playstation 2" => Some(Self::PlayStation2),
            "genesis" | "mega drive" | "megadrive" => Some(Self::MegaDrive),
            "sms" | "master system" => Some(Self::MasterSystem),
            "nds" | "ds" | "nintendo ds" => Some(Self::NintendoDS),
            "atari2600" | "2600" => Some(Self::Atari2600),
            "vb" | "virtual boy" => Some(Self::VirtualBoy),
            "gamecube" | "gc" | "ngc" => Some(Self::GameCube),
            "wii" => Some(Self::Wii),
            _ => None,
        }
    }

    pub fn as_u32(&self) -> u32 {
        *self as u32
    }
}

impl fmt::Display for ConsoleId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Self::GameBoyAdvance => "gba",
            Self::GameBoy => "gb",
            Self::GameBoyColor => "gbc",
            Self::NES => "nes",
            Self::SNES => "snes",
            Self::Nintendo64 => "n64",
            Self::PlayStation => "psx",
            Self::PlayStation2 => "ps2",
            Self::MegaDrive => "genesis",
            Self::MasterSystem => "sms",
            Self::NintendoDS => "nds",
            Self::Atari2600 => "atari2600",
            Self::VirtualBoy => "vb",
            Self::GameCube => "gamecube",
            Self::Wii => "wii",
        };
        write!(f, "{}", s)
    }
}

/// User summary from RetroAchievements API
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserSummary {
    #[serde(rename = "User")]
    pub user: String,
    #[serde(rename = "TotalPoints")]
    pub total_points: u32,
    #[serde(rename = "TotalSoftcorePoints")]
    pub total_softcore_points: u32,
    #[serde(rename = "TotalTruePoints")]
    pub total_true_points: u32,
    #[serde(rename = "Rank")]
    pub rank: Option<u32>,
    #[serde(rename = "RecentlyPlayed")]
    pub recently_played: Option<Vec<RecentGame>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecentGame {
    #[serde(rename = "GameID")]
    pub game_id: u32,
    #[serde(rename = "Title")]
    pub title: String,
    #[serde(rename = "ConsoleID")]
    pub console_id: u32,
    #[serde(rename = "ConsoleName")]
    pub console_name: String,
    #[serde(rename = "ImageIcon")]
    pub image_icon: String,
}

/// Game info with user progress
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameInfoAndProgress {
    #[serde(rename = "ID")]
    pub id: u32,
    #[serde(rename = "Title")]
    pub title: String,
    #[serde(rename = "ConsoleID")]
    pub console_id: u32,
    #[serde(rename = "ConsoleName")]
    pub console_name: String,
    #[serde(rename = "ImageIcon")]
    pub image_icon: String,
    #[serde(rename = "ImageTitle")]
    pub image_title: Option<String>,
    #[serde(rename = "ImageIngame")]
    pub image_ingame: Option<String>,
    #[serde(rename = "ImageBoxArt")]
    pub image_boxart: Option<String>,
    #[serde(rename = "NumAchievements")]
    pub num_achievements: u32,
    #[serde(rename = "NumDistinctPlayersCasual")]
    pub num_players_casual: u32,
    #[serde(rename = "NumDistinctPlayersHardcore")]
    pub num_players_hardcore: u32,
    #[serde(rename = "Achievements")]
    pub achievements: Option<std::collections::HashMap<String, Achievement>>,
    #[serde(rename = "NumAwardedToUser")]
    pub num_awarded_to_user: Option<u32>,
    #[serde(rename = "NumAwardedToUserHardcore")]
    pub num_awarded_to_user_hardcore: Option<u32>,
    #[serde(rename = "UserCompletion")]
    pub user_completion: Option<String>,
    #[serde(rename = "UserCompletionHardcore")]
    pub user_completion_hardcore: Option<String>,
}

/// Individual achievement
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Achievement {
    #[serde(rename = "ID")]
    pub id: u32,
    #[serde(rename = "Title")]
    pub title: String,
    #[serde(rename = "Description")]
    pub description: String,
    #[serde(rename = "Points")]
    pub points: u32,
    #[serde(rename = "BadgeName")]
    pub badge_name: String,
    #[serde(rename = "DisplayOrder")]
    pub display_order: u32,
    #[serde(rename = "MemAddr")]
    pub mem_addr: Option<String>,
    #[serde(rename = "Type")]
    #[serde(default = "default_achievement_type")]
    pub achievement_type: String,
    #[serde(rename = "DateEarned")]
    pub date_earned: Option<String>,
    #[serde(rename = "DateEarnedHardcore")]
    pub date_earned_hardcore: Option<String>,
}

impl Achievement {
    pub fn is_earned(&self) -> bool {
        self.date_earned.is_some() || self.date_earned_hardcore.is_some()
    }

    pub fn is_earned_hardcore(&self) -> bool {
        self.date_earned_hardcore.is_some()
    }

    pub fn badge_url(&self) -> String {
        format!(
            "https://media.retroachievements.org/Badge/{}.png",
            self.badge_name
        )
    }
}

/// Game ID lookup result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameIdLookup {
    #[serde(rename = "Success")]
    pub success: bool,
    #[serde(rename = "GameID")]
    pub game_id: u32,
}

/// Achievement unlock response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AwardAchievementResponse {
    #[serde(rename = "Success")]
    pub success: bool,
    #[serde(rename = "AchievementID")]
    pub achievement_id: Option<u32>,
    #[serde(rename = "AchievementsRemaining")]
    pub achievements_remaining: Option<u32>,
    #[serde(rename = "Score")]
    pub score: Option<u32>,
    #[serde(rename = "SoftcoreScore")]
    pub softcore_score: Option<u32>,
}

/// Simple game info for listing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameListEntry {
    #[serde(rename = "ID")]
    pub id: u32,
    #[serde(rename = "Title")]
    pub title: String,
    #[serde(rename = "ConsoleID")]
    pub console_id: u32,
    #[serde(rename = "ConsoleName")]
    pub console_name: String,
    #[serde(rename = "NumAchievements")]
    pub num_achievements: u32,
    #[serde(rename = "Points")]
    pub points: u32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gamecube_from_str() {
        assert_eq!(ConsoleId::parse("gamecube"), Some(ConsoleId::GameCube));
        assert_eq!(ConsoleId::parse("gc"), Some(ConsoleId::GameCube));
        assert_eq!(ConsoleId::parse("ngc"), Some(ConsoleId::GameCube));
        assert_eq!(ConsoleId::parse("GAMECUBE"), Some(ConsoleId::GameCube));
    }

    #[test]
    fn test_wii_from_str() {
        assert_eq!(ConsoleId::parse("wii"), Some(ConsoleId::Wii));
    }

    #[test]
    fn test_gamecube_as_u32() {
        assert_eq!(ConsoleId::GameCube.as_u32(), 16);
    }

    #[test]
    fn test_wii_as_u32() {
        assert_eq!(ConsoleId::Wii.as_u32(), 19);
    }

    #[test]
    fn test_gamecube_to_string() {
        assert_eq!(ConsoleId::GameCube.to_string(), "gamecube");
    }

    #[test]
    fn test_wii_to_string() {
        assert_eq!(ConsoleId::Wii.to_string(), "wii");
    }
}
