use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use crate::themes::Theme;

/// Theme configuration
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ThemeConfig {
    pub theme_name: String,
    pub version: u32,
}

impl Default for ThemeConfig {
    fn default() -> Self {
        Self {
            theme_name: "Dark".to_string(),
            version: 1,
        }
    }
}

/// Manages theme configuration
pub struct ThemeConfigManager {
    config: ThemeConfig,
    config_path: PathBuf,
    current_theme: Theme,
}

impl ThemeConfigManager {
    /// Create new ThemeConfigManager with config loaded from disk
    pub fn new() -> Result<Self> {
        let config_path = Self::get_config_path()?;

        let config = if config_path.exists() {
            println!("[ThemeConfig] Loading config from {:?}", config_path);
            Self::load_config(&config_path)?
        } else {
            println!("[ThemeConfig] No config found, using defaults");
            let default_config = ThemeConfig::default();

            // Try to save default config
            if let Err(e) = Self::save_config(&config_path, &default_config) {
                eprintln!("[ThemeConfig] Failed to save default config: {}", e);
            }

            default_config
        };

        // Load the theme
        let current_theme = Theme::by_name(&config.theme_name)
            .unwrap_or_else(|| {
                eprintln!("[ThemeConfig] Theme '{}' not found, using Dark", config.theme_name);
                Theme::dark()
            });

        Ok(Self {
            config,
            config_path,
            current_theme,
        })
    }

    /// Get the configuration file path
    fn get_config_path() -> Result<PathBuf> {
        let data_dir = dirs::data_local_dir()
            .context("Could not determine local data directory")?;

        let overlay_dir = data_dir.join("kazeta-plus").join("overlay");

        // Create directory if it doesn't exist
        if !overlay_dir.exists() {
            fs::create_dir_all(&overlay_dir)
                .context("Failed to create overlay config directory")?;
        }

        Ok(overlay_dir.join("theme.json"))
    }

    /// Load configuration from file
    fn load_config(path: &PathBuf) -> Result<ThemeConfig> {
        let contents = fs::read_to_string(path)
            .context("Failed to read theme config file")?;

        let config: ThemeConfig = serde_json::from_str(&contents)
            .context("Failed to parse theme config JSON")?;

        Ok(config)
    }

    /// Save configuration to file
    fn save_config(path: &PathBuf, config: &ThemeConfig) -> Result<()> {
        let json = serde_json::to_string_pretty(config)
            .context("Failed to serialize theme config")?;

        fs::write(path, json)
            .context("Failed to write theme config file")?;

        println!("[ThemeConfig] Config saved to {:?}", path);
        Ok(())
    }

    /// Get a reference to the current theme
    pub fn theme(&self) -> &Theme {
        &self.current_theme
    }

    /// Get the current theme name
    pub fn theme_name(&self) -> &str {
        &self.config.theme_name
    }

    /// Set the theme by name
    pub fn set_theme(&mut self, theme_name: &str) -> Result<()> {
        let theme = Theme::by_name(theme_name)
            .ok_or_else(|| anyhow::anyhow!("Theme '{}' not found", theme_name))?;

        self.config.theme_name = theme_name.to_string();
        self.current_theme = theme;
        self.save()?;

        println!("[ThemeConfig] Theme changed to: {}", theme_name);
        Ok(())
    }

    /// Get all available theme names
    pub fn available_themes() -> Vec<String> {
        Theme::all_presets()
            .into_iter()
            .map(|t| t.name)
            .collect()
    }

    /// Save current configuration to disk
    pub fn save(&self) -> Result<()> {
        Self::save_config(&self.config_path, &self.config)
    }
}

