use anyhow::{Context, Result};
use macroquad::prelude::KeyCode;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

/// Actions that can be triggered by hotkeys
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum HotkeyAction {
    ToggleOverlay,
    TogglePerformance,
    QuickSave,
    QuickLoad,
    Screenshot,
}

impl HotkeyAction {
    pub fn description(&self) -> &'static str {
        match self {
            Self::ToggleOverlay => "Toggle Overlay Menu",
            Self::TogglePerformance => "Toggle Performance HUD",
            Self::QuickSave => "Quick Save",
            Self::QuickLoad => "Quick Load",
            Self::Screenshot => "Take Screenshot",
        }
    }

    pub fn all() -> Vec<Self> {
        vec![
            Self::ToggleOverlay,
            Self::TogglePerformance,
            Self::QuickSave,
            Self::QuickLoad,
            Self::Screenshot,
        ]
    }
}

/// Individual input components that can be combined into hotkeys
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum InputComponent {
    Key(String),                    // Keyboard key name (e.g., "F12", "A")
    GamepadButton(GamepadButtonType), // Gamepad button
    Modifier(ModifierKey),          // Modifier keys (Ctrl, Alt, Shift)
}

impl InputComponent {
    /// Convert KeyCode to InputComponent
    pub fn from_keycode(key: KeyCode) -> Self {
        Self::Key(format!("{:?}", key))
    }

    /// Create a user-friendly display name
    pub fn display_name(&self) -> String {
        match self {
            Self::Key(k) => k.clone(),
            Self::GamepadButton(btn) => format!("{:?}", btn),
            Self::Modifier(m) => match m {
                ModifierKey::Ctrl => "Ctrl".to_string(),
                ModifierKey::Alt => "Alt".to_string(),
                ModifierKey::Shift => "Shift".to_string(),
            },
        }
    }
}

/// Gamepad button types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum GamepadButtonType {
    South,       // A/Cross
    East,        // B/Circle
    West,        // X/Square
    North,       // Y/Triangle
    DPadUp,
    DPadDown,
    DPadLeft,
    DPadRight,
    LeftBumper,
    RightBumper,
    LeftTrigger,
    RightTrigger,
    Select,
    Start,
    Mode,        // Guide/Home
    LeftStick,   // L3
    RightStick,  // R3
}

/// Modifier keys
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ModifierKey {
    Ctrl,
    Alt,
    Shift,
}

/// A single hotkey binding (combination of inputs)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HotkeyBinding {
    pub components: Vec<InputComponent>,
    pub description: String,
}

impl HotkeyBinding {
    pub fn new(components: Vec<InputComponent>, description: String) -> Self {
        Self {
            components,
            description,
        }
    }

    /// Get display string for this binding (e.g., "Ctrl+F12")
    pub fn display_string(&self) -> String {
        if self.components.is_empty() {
            return "None".to_string();
        }

        self.components
            .iter()
            .map(|c| c.display_name())
            .collect::<Vec<_>>()
            .join("+")
    }
}

/// Complete hotkey configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HotkeyConfig {
    pub bindings: HashMap<HotkeyAction, Vec<HotkeyBinding>>,
    pub version: u32,
}

impl HotkeyConfig {
    /// Create default configuration with standard bindings
    pub fn default_config() -> Self {
        let mut bindings = HashMap::new();

        // Toggle Overlay: F12, Ctrl+O, or Guide button
        bindings.insert(
            HotkeyAction::ToggleOverlay,
            vec![
                HotkeyBinding::new(
                    vec![InputComponent::Key("F12".to_string())],
                    "F12 key".to_string(),
                ),
                HotkeyBinding::new(
                    vec![
                        InputComponent::Modifier(ModifierKey::Ctrl),
                        InputComponent::Key("O".to_string()),
                    ],
                    "Ctrl+O".to_string(),
                ),
                HotkeyBinding::new(
                    vec![InputComponent::GamepadButton(GamepadButtonType::Mode)],
                    "Guide button".to_string(),
                ),
            ],
        );

        // Toggle Performance: F3
        bindings.insert(
            HotkeyAction::TogglePerformance,
            vec![HotkeyBinding::new(
                vec![InputComponent::Key("F3".to_string())],
                "F3 key".to_string(),
            )],
        );

        // Quick Save: F5
        bindings.insert(
            HotkeyAction::QuickSave,
            vec![HotkeyBinding::new(
                vec![InputComponent::Key("F5".to_string())],
                "F5 key".to_string(),
            )],
        );

        // Quick Load: F9
        bindings.insert(
            HotkeyAction::QuickLoad,
            vec![HotkeyBinding::new(
                vec![InputComponent::Key("F9".to_string())],
                "F9 key".to_string(),
            )],
        );

        // Screenshot: F12
        bindings.insert(
            HotkeyAction::Screenshot,
            vec![HotkeyBinding::new(
                vec![InputComponent::Key("F12".to_string())],
                "F12 key".to_string(),
            )],
        );

        Self {
            bindings,
            version: 1,
        }
    }
}

impl Default for HotkeyConfig {
    fn default() -> Self {
        Self::default_config()
    }
}

/// Manages hotkey bindings and detection
pub struct HotkeyManager {
    config: HotkeyConfig,
    config_path: PathBuf,
    /// Track last state of each binding to detect rising edge
    binding_last_states: HashMap<Vec<InputComponent>, bool>,
}

impl HotkeyManager {
    /// Create new HotkeyManager with config loaded from disk
    pub fn new() -> Result<Self> {
        let config_path = Self::get_config_path()?;

        let config = if config_path.exists() {
            println!("[Hotkeys] Loading config from {:?}", config_path);
            Self::load_config(&config_path)?
        } else {
            println!("[Hotkeys] No config found, using defaults");
            let default_config = HotkeyConfig::default_config();

            // Try to save default config
            if let Err(e) = Self::save_config(&config_path, &default_config) {
                eprintln!("[Hotkeys] Failed to save default config: {}", e);
            }

            default_config
        };

        Ok(Self {
            config,
            config_path,
            binding_last_states: HashMap::new(),
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

        Ok(overlay_dir.join("hotkeys.json"))
    }

    /// Load configuration from file
    fn load_config(path: &PathBuf) -> Result<HotkeyConfig> {
        let contents = fs::read_to_string(path)
            .context("Failed to read hotkey config file")?;

        let config: HotkeyConfig = serde_json::from_str(&contents)
            .context("Failed to parse hotkey config JSON")?;

        Ok(config)
    }

    /// Save configuration to file
    fn save_config(path: &PathBuf, config: &HotkeyConfig) -> Result<()> {
        let json = serde_json::to_string_pretty(config)
            .context("Failed to serialize hotkey config")?;

        fs::write(path, json)
            .context("Failed to write hotkey config file")?;

        println!("[Hotkeys] Config saved to {:?}", path);
        Ok(())
    }

    /// Save current configuration to disk
    pub fn save(&self) -> Result<()> {
        Self::save_config(&self.config_path, &self.config)
    }

    /// Check if a specific action's hotkey was just pressed (rising edge)
    pub fn check_action_pressed(
        &mut self,
        action: HotkeyAction,
        current_inputs: &HashMap<InputComponent, bool>,
    ) -> bool {
        // Clone bindings to avoid borrow checker issues
        let bindings = match self.config.bindings.get(&action) {
            Some(b) => b.clone(),
            None => return false,
        };

        for binding in bindings {
            if self.check_binding_pressed(&binding.components, current_inputs) {
                return true;
            }
        }

        false
    }

    /// Check if a specific binding combination was just pressed (rising edge)
    fn check_binding_pressed(
        &mut self,
        components: &[InputComponent],
        current_inputs: &HashMap<InputComponent, bool>,
    ) -> bool {
        // Check if all components are currently pressed
        let all_pressed = components.iter().all(|component| {
            current_inputs.get(component).copied().unwrap_or(false)
        });

        // Get last state (default to false if never checked)
        let key = components.to_vec();
        let was_pressed = self.binding_last_states.get(&key).copied().unwrap_or(false);

        // Update state
        self.binding_last_states.insert(key, all_pressed);

        // Rising edge: now pressed but wasn't before
        all_pressed && !was_pressed
    }

    /// Get all bindings for a specific action
    pub fn get_bindings(&self, action: HotkeyAction) -> Option<&Vec<HotkeyBinding>> {
        self.config.bindings.get(&action)
    }

    /// Add a new binding for an action
    pub fn add_binding(&mut self, action: HotkeyAction, binding: HotkeyBinding) {
        self.config
            .bindings
            .entry(action)
            .or_insert_with(Vec::new)
            .push(binding);
    }

    /// Remove a binding for an action by index
    pub fn remove_binding(&mut self, action: HotkeyAction, index: usize) -> bool {
        if let Some(bindings) = self.config.bindings.get_mut(&action) {
            if index < bindings.len() {
                bindings.remove(index);
                return true;
            }
        }
        false
    }

    /// Check if a binding conflicts with existing bindings
    pub fn has_conflict(&self, components: &[InputComponent]) -> Option<HotkeyAction> {
        for (action, bindings) in &self.config.bindings {
            for binding in bindings {
                if binding.components == components {
                    return Some(*action);
                }
            }
        }
        None
    }

    /// Get reference to current config
    pub fn config(&self) -> &HotkeyConfig {
        &self.config
    }

    /// Get mutable reference to current config
    pub fn config_mut(&mut self) -> &mut HotkeyConfig {
        &mut self.config
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config_has_all_actions() {
        let config = HotkeyConfig::default_config();

        for action in HotkeyAction::all() {
            assert!(
                config.bindings.contains_key(&action),
                "Default config missing binding for {:?}",
                action
            );
        }
    }

    #[test]
    fn test_binding_display_string() {
        let binding = HotkeyBinding::new(
            vec![
                InputComponent::Modifier(ModifierKey::Ctrl),
                InputComponent::Key("F12".to_string()),
            ],
            "Test binding".to_string(),
        );

        assert_eq!(binding.display_string(), "Ctrl+F12");
    }

    #[test]
    fn test_conflict_detection() {
        let mut manager = HotkeyManager::new().unwrap();

        let test_components = vec![InputComponent::Key("F12".to_string())];

        // F12 is bound to ToggleOverlay by default
        let conflict = manager.has_conflict(&test_components);
        assert_eq!(conflict, Some(HotkeyAction::ToggleOverlay));
    }

    #[test]
    fn test_add_remove_binding() {
        let mut manager = HotkeyManager::new().unwrap();

        let new_binding = HotkeyBinding::new(
            vec![InputComponent::Key("F1".to_string())],
            "Test F1".to_string(),
        );

        let initial_count = manager
            .get_bindings(HotkeyAction::ToggleOverlay)
            .map(|b| b.len())
            .unwrap_or(0);

        manager.add_binding(HotkeyAction::ToggleOverlay, new_binding);

        let after_add = manager
            .get_bindings(HotkeyAction::ToggleOverlay)
            .map(|b| b.len())
            .unwrap_or(0);

        assert_eq!(after_add, initial_count + 1);

        manager.remove_binding(HotkeyAction::ToggleOverlay, initial_count);

        let after_remove = manager
            .get_bindings(HotkeyAction::ToggleOverlay)
            .map(|b| b.len())
            .unwrap_or(0);

        assert_eq!(after_remove, initial_count);
    }
}
