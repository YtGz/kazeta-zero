use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

/// Menu item identifier
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MenuItemId {
    Controllers,
    Settings,
    Achievements,
    Performance,
    Playtime,
    QuickSave,
    Resume,
    Quit,
}

impl MenuItemId {
    pub fn display_name(&self) -> &'static str {
        match self {
            MenuItemId::Controllers => "Controllers",
            MenuItemId::Settings => "Settings",
            MenuItemId::Achievements => "Achievements",
            MenuItemId::Performance => "Performance",
            MenuItemId::Playtime => "Playtime",
            MenuItemId::QuickSave => "Quick Save",
            MenuItemId::Resume => "Resume Game",
            MenuItemId::Quit => "Quit to BIOS",
        }
    }

    pub fn all() -> Vec<MenuItemId> {
        vec![
            MenuItemId::Controllers,
            MenuItemId::Settings,
            MenuItemId::Achievements,
            MenuItemId::Performance,
            MenuItemId::Playtime,
            MenuItemId::QuickSave,
            MenuItemId::Resume,
            MenuItemId::Quit,
        ]
    }
}

/// Configuration for a single menu item
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct MenuItemConfig {
    pub id: MenuItemId,
    pub visible: bool,
    pub order: usize,
}

/// Menu configuration
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct MenuConfig {
    pub items: Vec<MenuItemConfig>,
    pub version: u32,
}

impl Default for MenuConfig {
    fn default() -> Self {
        let default_items = MenuItemId::all()
            .into_iter()
            .enumerate()
            .map(|(order, id)| MenuItemConfig {
                id,
                visible: true,
                order,
            })
            .collect();

        Self {
            items: default_items,
            version: 1,
        }
    }
}

impl MenuConfig {
    /// Get the default menu configuration
    pub fn default_config() -> Self {
        Self::default()
    }

    /// Get visible menu items in order
    pub fn get_visible_items(&self) -> Vec<MenuItemId> {
        let mut visible: Vec<_> = self.items
            .iter()
            .filter(|item| item.visible)
            .collect();
        
        visible.sort_by_key(|item| item.order);
        visible.into_iter().map(|item| item.id).collect()
    }

    /// Get all menu items in order (including hidden)
    pub fn get_all_items(&self) -> Vec<MenuItemId> {
        let mut items: Vec<_> = self.items.iter().collect();
        items.sort_by_key(|item| item.order);
        items.into_iter().map(|item| item.id).collect()
    }

    /// Toggle visibility of a menu item
    pub fn toggle_visibility(&mut self, id: MenuItemId) {
        if let Some(item) = self.items.iter_mut().find(|i| i.id == id) {
            item.visible = !item.visible;
        }
    }

    /// Set visibility of a menu item
    pub fn set_visibility(&mut self, id: MenuItemId, visible: bool) {
        if let Some(item) = self.items.iter_mut().find(|i| i.id == id) {
            item.visible = visible;
        }
    }

    /// Move a menu item up in order
    pub fn move_up(&mut self, id: MenuItemId) {
        if let Some(item) = self.items.iter().find(|i| i.id == id) {
            let current_order = item.order;
            if current_order > 0 {
                // Find item with order - 1 and swap
                if let Some(other) = self.items.iter_mut().find(|i| i.order == current_order - 1) {
                    other.order = current_order;
                    if let Some(this) = self.items.iter_mut().find(|i| i.id == id) {
                        this.order = current_order - 1;
                    }
                }
            }
        }
    }

    /// Move a menu item down in order
    pub fn move_down(&mut self, id: MenuItemId) {
        if let Some(item) = self.items.iter().find(|i| i.id == id) {
            let current_order = item.order;
            let max_order = self.items.len().saturating_sub(1);
            if current_order < max_order {
                // Find item with order + 1 and swap
                if let Some(other) = self.items.iter_mut().find(|i| i.order == current_order + 1) {
                    other.order = current_order;
                    if let Some(this) = self.items.iter_mut().find(|i| i.id == id) {
                        this.order = current_order + 1;
                    }
                }
            }
        }
    }

    /// Get the index of a menu item in the visible list
    pub fn get_visible_index(&self, id: MenuItemId) -> Option<usize> {
        self.get_visible_items().iter().position(|&item_id| item_id == id)
    }

    /// Get menu item by visible index
    pub fn get_item_by_visible_index(&self, index: usize) -> Option<MenuItemId> {
        self.get_visible_items().get(index).copied()
    }
}

/// Manages menu configuration
pub struct MenuConfigManager {
    config: MenuConfig,
    config_path: PathBuf,
}

impl MenuConfigManager {
    /// Create new MenuConfigManager with config loaded from disk
    pub fn new() -> Result<Self> {
        let config_path = Self::get_config_path()?;

        let config = if config_path.exists() {
            println!("[MenuConfig] Loading config from {:?}", config_path);
            Self::load_config(&config_path)?
        } else {
            println!("[MenuConfig] No config found, using defaults");
            let default_config = MenuConfig::default_config();

            // Try to save default config
            if let Err(e) = Self::save_config(&config_path, &default_config) {
                eprintln!("[MenuConfig] Failed to save default config: {}", e);
            }

            default_config
        };

        Ok(Self {
            config,
            config_path,
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

        Ok(overlay_dir.join("menu.json"))
    }

    /// Load configuration from file
    fn load_config(path: &PathBuf) -> Result<MenuConfig> {
        let contents = fs::read_to_string(path)
            .context("Failed to read menu config file")?;

        let config: MenuConfig = serde_json::from_str(&contents)
            .context("Failed to parse menu config JSON")?;

        Ok(config)
    }

    /// Save configuration to file
    fn save_config(path: &PathBuf, config: &MenuConfig) -> Result<()> {
        let json = serde_json::to_string_pretty(config)
            .context("Failed to serialize menu config")?;

        fs::write(path, json)
            .context("Failed to write menu config file")?;

        println!("[MenuConfig] Config saved to {:?}", path);
        Ok(())
    }

    /// Get a reference to the current config
    pub fn config(&self) -> &MenuConfig {
        &self.config
    }

    /// Get a mutable reference to the current config
    pub fn config_mut(&mut self) -> &mut MenuConfig {
        &mut self.config
    }

    /// Save current configuration to disk
    pub fn save(&self) -> Result<()> {
        Self::save_config(&self.config_path, &self.config)
    }
}

