use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::Path;

/// Local achievement definitions file format.
///
/// This is the JSON structure baked into each cartridge's SD card as
/// `achievements.json`. It contains the full achievement set — the same data
/// the RetroAchievements API returns, stored locally so the playing machine
/// needs no internet connection and no RA account.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocalDefinitions {
    pub game_id: u32,
    pub game_title: String,
    pub console_id: u32,
    pub console_name: String,
    #[serde(default)]
    pub icon_url: Option<String>,
    #[serde(default)]
    pub rich_presence_patch: String,
    pub achievements: Vec<LocalAchievement>,
    #[serde(default)]
    pub leaderboards: Vec<LocalLeaderboard>,
}

/// A single achievement definition in the local format.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocalAchievement {
    pub id: u32,
    pub title: String,
    pub description: String,
    pub points: u32,
    pub badge_name: String,
    /// The rcheevos condition string (the "MemAddr" field from the RA API).
    /// This is the full achievement logic that the evaluation engine parses
    /// via `rc_parse_trigger()`.
    pub mem_addr: String,
    #[serde(default = "default_achievement_type")]
    #[serde(rename = "type")]
    pub achievement_type: String,
    pub display_order: u32,
}

fn default_achievement_type() -> String {
    "standard".to_string()
}

/// A leaderboard definition (included for completeness but not used —
/// leaderboards are inherently online and this fork does not support them).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocalLeaderboard {
    pub id: u32,
    pub title: String,
    pub description: String,
    #[serde(default)]
    pub format: String,
    #[serde(default)]
    pub mem_addr: String,
    #[serde(default)]
    pub lower_is_better: bool,
}

impl LocalDefinitions {
    /// Load definitions from a JSON file on disk.
    pub fn load(path: &Path) -> Result<Self> {
        let content = std::fs::read_to_string(path)
            .with_context(|| format!("Failed to read definitions file: {:?}", path))?;
        Self::from_json(&content)
    }

    /// Parse definitions from a JSON string.
    pub fn from_json(json: &str) -> Result<Self> {
        let defs: LocalDefinitions =
            serde_json::from_str(json).context("Failed to parse definitions JSON")?;
        Ok(defs)
    }

    /// Save definitions to a JSON file on disk.
    pub fn save(&self, path: &Path) -> Result<()> {
        let json = self.to_json_pretty()?;
        std::fs::write(path, json)
            .with_context(|| format!("Failed to write definitions file: {:?}", path))?;
        Ok(())
    }

    /// Serialize to a pretty-printed JSON string.
    pub fn to_json_pretty(&self) -> Result<String> {
        serde_json::to_string_pretty(self).context("Failed to serialize definitions")
    }

    /// Check if a definitions file exists at the given path.
    pub fn exists(path: &Path) -> bool {
        path.exists()
    }

    /// Extract all unique memory addresses referenced in the achievement
    /// condition strings. These are the addresses that need to be watched
    /// via Dolphin's MemoryWatcher.
    ///
    /// rcheevos condition strings reference memory using patterns like:
    ///   `0xH00801234`  — 8-bit value at address 0x00801234
    ///   `0x 00801234`  — 16-bit value
    ///   `0xX00801234` — 32-bit value
    ///   `0xL00801234` — lower 16 bits
    ///   `0xU00801234` — upper 16 bits
    ///   `d0xH00801234` — delta (previous frame value)
    ///   `0xM00801234` — bit 0 (and bit1=M+0x1, etc.)
    ///
    /// MemoryWatcher reads 4-byte (u32) values, so we align all addresses
    /// down to 4-byte boundaries and deduplicate.
    pub fn extract_memory_addresses(&self) -> Vec<u32> {
        let mut addresses: std::collections::BTreeSet<u32> = std::collections::BTreeSet::new();

        for ach in &self.achievements {
            for addr in parse_addresses_from_condition(&ach.mem_addr) {
                // Align down to 4-byte boundary (MemoryWatcher reads u32)
                addresses.insert(addr & 0xFFFF_FFFC);
            }
        }

        addresses.into_iter().collect()
    }
}

/// Parse all memory addresses from a rcheevos condition string.
///
/// The condition string format uses memory references like `0xH00801234`,
/// `d0xX00801234`, `0x00801234`, etc. This function extracts the hex
/// address portion from each reference.
fn parse_addresses_from_condition(condition: &str) -> Vec<u32> {
    let mut addresses = Vec::new();
    let bytes = condition.as_bytes();
    let mut i = 0;

    while i < bytes.len() {
        // Look for "0x" patterns
        if i + 2 < bytes.len() && bytes[i] == b'0' && bytes[i + 1] == b'x' {
            // Skip optional 'd' prefix (delta) — but that's before 0x, so check preceding char
            // Move past "0x"
            let mut j = i + 2;

            // Skip the size/type modifier character(s):
            // H, X, L, U, M, or a space (16-bit), or a digit (32-bit)
            // The modifier is a single character, but for 'M' (bitN) it can
            // be followed by a bitmask expression. We skip non-hex chars until
            // we find the hex address.
            while j < bytes.len()
                && !bytes[j].is_ascii_hexdigit()
                && bytes[j] != b'+'
                && bytes[j] != b'-'
            {
                j += 1;
            }

            // Now collect hex digits
            let start = j;
            while j < bytes.len() && bytes[j].is_ascii_hexdigit() {
                j += 1;
            }

            if j > start {
                let hex_str = &condition[start..j];
                if let Ok(addr) = u32::from_str_radix(hex_str, 16) {
                    addresses.push(addr);
                }
            }

            i = j;
        } else {
            i += 1;
        }
    }

    addresses
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_roundtrip_json() {
        let defs = LocalDefinitions {
            game_id: 7693,
            game_title: "Test Game".to_string(),
            console_id: 16,
            console_name: "Nintendo GameCube".to_string(),
            icon_url: None,
            rich_presence_patch: String::new(),
            achievements: vec![LocalAchievement {
                id: 55001,
                title: "Test Achievement".to_string(),
                description: "Do the thing".to_string(),
                points: 10,
                badge_name: "55001".to_string(),
                mem_addr: "0xH00801234=1.0.5.0=d0xH00801234".to_string(),
                achievement_type: "standard".to_string(),
                display_order: 1,
            }],
            leaderboards: vec![],
        };

        let json = defs.to_json_pretty().unwrap();
        let parsed = LocalDefinitions::from_json(&json).unwrap();
        assert_eq!(parsed.game_id, 7693);
        assert_eq!(parsed.achievements.len(), 1);
        assert_eq!(
            parsed.achievements[0].mem_addr,
            "0xH00801234=1.0.5.0=d0xH00801234"
        );
    }

    #[test]
    fn test_parse_simple_address() {
        let addrs = parse_addresses_from_condition("0xH00801234=1");
        assert_eq!(addrs, vec![0x00801234]);
    }

    #[test]
    fn test_parse_multiple_addresses() {
        let addrs = parse_addresses_from_condition("0xH00801234=1.0.5.0=d0xH00805678");
        assert_eq!(addrs, vec![0x00801234, 0x00805678]);
    }

    #[test]
    fn test_parse_32bit_address() {
        let addrs = parse_addresses_from_condition("0xX00801234=42");
        assert_eq!(addrs, vec![0x00801234]);
    }

    #[test]
    fn test_parse_16bit_address() {
        let addrs = parse_addresses_from_condition("0x 00801234=1");
        assert_eq!(addrs, vec![0x00801234]);
    }

    #[test]
    fn test_extract_memory_addresses_aligns_to_4() {
        let defs = LocalDefinitions {
            game_id: 1,
            game_title: "T".to_string(),
            console_id: 16,
            console_name: "GC".to_string(),
            icon_url: None,
            rich_presence_patch: String::new(),
            achievements: vec![
                LocalAchievement {
                    id: 1,
                    title: "A".to_string(),
                    description: "D".to_string(),
                    points: 5,
                    badge_name: "1".to_string(),
                    mem_addr: "0xH00801235=1".to_string(),
                    achievement_type: "standard".to_string(),
                    display_order: 1,
                },
                LocalAchievement {
                    id: 2,
                    title: "B".to_string(),
                    description: "D".to_string(),
                    points: 5,
                    badge_name: "2".to_string(),
                    mem_addr: "0xH00801237=1".to_string(),
                    achievement_type: "standard".to_string(),
                    display_order: 2,
                },
            ],
            leaderboards: vec![],
        };

        let addrs = defs.extract_memory_addresses();
        // Both 0x00801235 and 0x00801237 align down to 0x00801234
        assert_eq!(addrs, vec![0x00801234]);
    }

    #[test]
    fn test_extract_memory_addresses_deduplicates() {
        let defs = LocalDefinitions {
            game_id: 1,
            game_title: "T".to_string(),
            console_id: 16,
            console_name: "GC".to_string(),
            icon_url: None,
            rich_presence_patch: String::new(),
            achievements: vec![LocalAchievement {
                id: 1,
                title: "A".to_string(),
                description: "D".to_string(),
                points: 5,
                badge_name: "1".to_string(),
                mem_addr: "0xH00801234=1.0.5.0=d0xH00801234".to_string(),
                achievement_type: "standard".to_string(),
                display_order: 1,
            }],
            leaderboards: vec![],
        };

        let addrs = defs.extract_memory_addresses();
        assert_eq!(addrs, vec![0x00801234]);
    }
}
