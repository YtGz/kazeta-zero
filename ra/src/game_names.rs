use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

/// Game name mapping: ROM hash -> custom game name
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GameNameMapping {
    /// Map of ROM hash to custom game name
    #[serde(default)]
    pub games: HashMap<String, GameNameEntry>,
}

/// Entry for a single game name mapping
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameNameEntry {
    /// Custom name for the game
    pub name: String,
    /// Optional console ID (for reference)
    #[serde(default)]
    pub console: Option<String>,
}

impl GameNameMapping {
    /// Load game name mappings from disk
    pub fn load() -> Result<Self> {
        let path = Self::get_config_path()?;
        
        if !path.exists() {
            return Ok(Self::default());
        }

        let content = fs::read_to_string(&path)
            .context("Failed to read game names config file")?;

        let mapping: GameNameMapping = serde_json::from_str(&content)
            .context("Failed to parse game names config JSON")?;

        Ok(mapping)
    }

    /// Save game name mappings to disk
    pub fn save(&self) -> Result<()> {
        let path = Self::get_config_path()?;
        
        // Create directory if it doesn't exist
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .context("Failed to create config directory")?;
        }

        let json = serde_json::to_string_pretty(self)
            .context("Failed to serialize game names config")?;

        fs::write(&path, json)
            .context("Failed to write game names config file")?;

        Ok(())
    }

    /// Get custom name for a ROM hash
    /// Checks cartridge TOML first (if cart_path provided), then JSON mapping
    pub fn get_name(&self, hash: &str, cart_path: Option<&Path>) -> Option<String> {
        // First, try to get from cartridge TOML if path is provided
        if let Some(path) = cart_path {
            if let Ok(name) = Self::get_name_from_cartridge(path) {
                return Some(name);
            }
        }

        // Fall back to JSON mapping
        self.games.get(hash).map(|e| e.name.clone())
    }

    /// Get game name from cartridge TOML file
    fn get_name_from_cartridge(cart_path: &Path) -> Result<String> {
        use std::io::Read;
        use flate2::read::GzDecoder;
        use tar::Archive;

        // Open the .kzi file (which is a gzip-compressed tar archive)
        let file = fs::File::open(cart_path)
            .context("Failed to open cartridge file")?;
        let decoder = GzDecoder::new(file);
        let mut archive = Archive::new(decoder);

        // Find and read the cartridge.toml file from the archive
        let mut content = String::new();

        for entry in archive.entries()? {
            let mut entry = entry?;
            let path = entry.path()?;

            if path.file_name().and_then(|n| n.to_str()) == Some("cartridge.toml") {
                entry.read_to_string(&mut content)?;
                
                // Parse the TOML content
                let toml_value: toml::Value = toml::from_str(&content)
                    .context("Failed to parse cartridge.toml")?;

                // Extract ra_game_name field
                if let Some(name) = toml_value.get("ra_game_name").and_then(|v| v.as_str()) {
                    return Ok(name.to_string());
                }
                break;
            }
        }

        bail!("No ra_game_name found in cartridge.toml")
    }

    /// Set custom name for a ROM hash
    pub fn set_name(&mut self, hash: String, name: String, console: Option<String>) -> Result<()> {
        self.games.insert(hash, GameNameEntry { name, console });
        self.save()?;
        Ok(())
    }

    /// Remove custom name for a ROM hash
    pub fn remove_name(&mut self, hash: &str) -> Result<()> {
        self.games.remove(hash);
        self.save()?;
        Ok(())
    }

    /// Get the configuration file path
    fn get_config_path() -> Result<PathBuf> {
        let data_dir = dirs::home_dir()
            .context("Could not find home directory")?
            .join(".local/share/kazeta-plus");

        fs::create_dir_all(&data_dir)
            .context("Failed to create kazeta data directory")?;

        Ok(data_dir.join("ra_game_names.json"))
    }
}

