use macroquad::prelude::*;
use std::collections::HashMap;
use std::process::Command;

use crate::{
    audio::SoundEffects,
    config::Config,
    types::{AnimationState, BackgroundState, BatteryInfo, Screen},
    ui::text_with_color,
    render_background, render_ui_overlay, get_current_font, measure_text, text_with_config_color,
    FONT_SIZE, MENU_PADDING, MENU_OPTION_HEIGHT, InputState, VideoPlayer,
};

const RA_SETTINGS_OPTIONS: &[&str] = &[
    "ENABLED",
    "USERNAME",
    "API KEY",
    "HARDCORE MODE",
    "SHOW NOTIFICATIONS",
    "LOGIN / TEST",
    "LOGOUT",
];

/// State for the RetroAchievements settings screen
#[derive(Clone, Debug, Default)]
pub struct RASettingsState {
    pub selection: usize,
    pub username_input: String,
    pub api_key_input: String,
    pub editing_username: bool,
    pub editing_api_key: bool,
    pub status_message: Option<String>,
    pub is_logged_in: bool,
    pub logged_in_user: Option<String>,
}

impl RASettingsState {
    pub fn new() -> Self {
        let mut state = Self::default();
        state.refresh_status();
        state
    }

    /// Check if kazeta-ra is logged in
    pub fn refresh_status(&mut self) {
        // Check login status via kazeta-ra CLI
        if let Ok(output) = Command::new("kazeta-ra")
            .arg("status")
            .output()
        {
            if output.status.success() {
                let stdout = String::from_utf8_lossy(&output.stdout);
                // Parse JSON response
                if stdout.contains("\"enabled\":true") || stdout.contains("\"enabled\": true") {
                    self.is_logged_in = true;
                    // Extract username
                    if let Some(start) = stdout.find("\"username\":\"") {
                        let rest = &stdout[start + 12..];
                        if let Some(end) = rest.find('"') {
                            self.logged_in_user = Some(rest[..end].to_string());
                        }
                    } else if let Some(start) = stdout.find("\"username\": \"") {
                        let rest = &stdout[start + 13..];
                        if let Some(end) = rest.find('"') {
                            self.logged_in_user = Some(rest[..end].to_string());
                        }
                    }
                } else {
                    self.is_logged_in = false;
                    self.logged_in_user = None;
                }
            }
        }
    }

    /// Attempt to login with the entered credentials
    pub fn attempt_login(&mut self) {
        if self.username_input.is_empty() || self.api_key_input.is_empty() {
            self.status_message = Some("Please enter username and API key".to_string());
            return;
        }

        self.status_message = Some("Logging in...".to_string());

        let result = Command::new("kazeta-ra")
            .arg("login")
            .arg("--username")
            .arg(&self.username_input)
            .arg("--api-key")
            .arg(&self.api_key_input)
            .output();

        match result {
            Ok(output) => {
                if output.status.success() {
                    self.status_message = Some("Login successful!".to_string());
                    self.is_logged_in = true;
                    self.logged_in_user = Some(self.username_input.clone());
                    // Clear sensitive input
                    self.api_key_input.clear();
                } else {
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    self.status_message = Some(format!("Login failed: {}", stderr.trim()));
                }
            }
            Err(e) => {
                self.status_message = Some(format!("Error: {}", e));
            }
        }
    }

    /// Logout from RetroAchievements
    pub fn logout(&mut self) {
        let _ = Command::new("kazeta-ra")
            .arg("logout")
            .output();

        self.is_logged_in = false;
        self.logged_in_user = None;
        self.username_input.clear();
        self.api_key_input.clear();
        self.status_message = Some("Logged out".to_string());
    }
}

/// Handles input and state logic for the RetroAchievements settings screen.
pub fn update(
    current_screen: &mut Screen,
    ra_state: &mut RASettingsState,
    input_state: &InputState,
    animation_state: &mut AnimationState,
    sound_effects: &SoundEffects,
    config: &mut Config,
) {
    // Handle text input mode for username
    if ra_state.editing_username {
        handle_text_input(&mut ra_state.username_input, input_state);
        if input_state.select || input_state.back {
            ra_state.editing_username = false;
            sound_effects.play_select(config);
        }
        return;
    }

    // Handle text input mode for API key
    if ra_state.editing_api_key {
        handle_text_input(&mut ra_state.api_key_input, input_state);
        if input_state.select || input_state.back {
            ra_state.editing_api_key = false;
            sound_effects.play_select(config);
        }
        return;
    }

    // Normal menu navigation
    if input_state.up {
        ra_state.selection = if ra_state.selection == 0 {
            RA_SETTINGS_OPTIONS.len() - 1
        } else {
            ra_state.selection - 1
        };
        animation_state.trigger_transition(&config.cursor_transition_speed);
        sound_effects.play_cursor_move(config);
    }
    if input_state.down {
        ra_state.selection = (ra_state.selection + 1) % RA_SETTINGS_OPTIONS.len();
        animation_state.trigger_transition(&config.cursor_transition_speed);
        sound_effects.play_cursor_move(config);
    }
    if input_state.back {
        *current_screen = Screen::GeneralSettings;
        sound_effects.play_back(config);
    }

    // Handle selection
    if input_state.select || input_state.left || input_state.right {
        match ra_state.selection {
            0 => { // ENABLED
                if input_state.left || input_state.right {
                    config.retroachievements.enabled = !config.retroachievements.enabled;
                    config.save();
                    sound_effects.play_cursor_move(config);
                }
            }
            1 => { // USERNAME
                if input_state.select {
                    ra_state.editing_username = true;
                    sound_effects.play_select(config);
                }
            }
            2 => { // API KEY
                if input_state.select {
                    ra_state.editing_api_key = true;
                    sound_effects.play_select(config);
                }
            }
            3 => { // HARDCORE MODE
                if input_state.left || input_state.right {
                    config.retroachievements.hardcore_mode = !config.retroachievements.hardcore_mode;
                    config.save();
                    // Also update kazeta-ra if logged in
                    if ra_state.is_logged_in {
                        let _ = Command::new("kazeta-ra")
                            .arg("set-hardcore")
                            .arg("--enabled")
                            .arg(if config.retroachievements.hardcore_mode { "true" } else { "false" })
                            .output();
                    }
                    sound_effects.play_cursor_move(config);
                }
            }
            4 => { // SHOW NOTIFICATIONS
                if input_state.left || input_state.right {
                    config.retroachievements.show_notifications = !config.retroachievements.show_notifications;
                    config.save();
                    sound_effects.play_cursor_move(config);
                }
            }
            5 => { // LOGIN / TEST
                if input_state.select {
                    sound_effects.play_select(config);
                    ra_state.attempt_login();
                }
            }
            6 => { // LOGOUT
                if input_state.select {
                    sound_effects.play_select(config);
                    ra_state.logout();
                }
            }
            _ => {}
        }
    }
}

/// Handle text input for username/api key fields
fn handle_text_input(text: &mut String, input_state: &InputState) {
    // Get typed characters from macroquad
    while let Some(c) = get_char_pressed() {
        if c.is_alphanumeric() || c == '_' || c == '-' {
            text.push(c);
        }
    }

    // Handle backspace
    if is_key_pressed(KeyCode::Backspace) && !text.is_empty() {
        text.pop();
    }
}

/// Draws the RetroAchievements settings UI.
pub fn draw(
    ra_state: &RASettingsState,
    animation_state: &AnimationState,
    logo_cache: &HashMap<String, Texture2D>,
    background_cache: &HashMap<String, Texture2D>,
    video_cache: &mut HashMap<String, VideoPlayer>,
    font_cache: &HashMap<String, Font>,
    config: &Config,
    background_state: &mut BackgroundState,
    battery_info: &Option<BatteryInfo>,
    current_time_str: &str,
    gcc_adapter_poll_rate: &Option<u32>,
    scale_factor: f32,
) {
    render_background(background_cache, video_cache, config, background_state);

    // Dim the background for easier legibility
    draw_rectangle(0.0, 0.0, screen_width(), screen_height(), Color::new(0.0, 0.0, 0.0, 0.6));

    render_ui_overlay(logo_cache, font_cache, config, battery_info, current_time_str, gcc_adapter_poll_rate, scale_factor);

    let font_size = (FONT_SIZE as f32 * scale_factor) as u16;
    let large_font_size = (FONT_SIZE as f32 * scale_factor * 1.5) as u16;
    let menu_padding = MENU_PADDING * scale_factor;
    let menu_option_height = MENU_OPTION_HEIGHT * scale_factor;
    let current_font = get_current_font(font_cache, config);

    // Title
    let title = "RETROACHIEVEMENTS";
    let title_dims = measure_text(title, Some(current_font), large_font_size, 1.0);
    let title_x = screen_width() / 2.0 - title_dims.width / 2.0;
    let title_y = 50.0 * scale_factor;
    text_with_config_color(font_cache, config, title, title_x, title_y, large_font_size);

    // Login status
    let status_text = if ra_state.is_logged_in {
        format!("Logged in as: {}", ra_state.logged_in_user.as_deref().unwrap_or("Unknown"))
    } else {
        "Not logged in".to_string()
    };
    let status_color = if ra_state.is_logged_in { GREEN } else { Color::new(0.7, 0.7, 0.7, 1.0) };
    let status_dims = measure_text(&status_text, Some(current_font), font_size, 1.0);
    let status_x = screen_width() / 2.0 - status_dims.width / 2.0;
    text_with_color(font_cache, config, &status_text, status_x, title_y + 25.0 * scale_factor, font_size, status_color);

    // Menu options
    let start_y = 100.0 * scale_factor;
    let left_margin = 80.0 * scale_factor;
    let right_margin = 80.0 * scale_factor;

    for (i, &label) in RA_SETTINGS_OPTIONS.iter().enumerate() {
        let y_pos = start_y + (i as f32 * menu_option_height);
        let is_selected = i == ra_state.selection;

        // Get value for this option
        let value = get_option_value(i, ra_state, config);
        let value_dims = measure_text(&value, Some(current_font), font_size, 1.0);
        let value_x = screen_width() - value_dims.width - right_margin;
        let text_y = y_pos + menu_option_height / 2.0 + font_size as f32 * 0.3;

        // Draw selection highlight
        if is_selected && config.cursor_style == "BOX" {
            let cursor_color = animation_state.get_cursor_color(config);
            let cursor_scale = animation_state.get_cursor_scale();

            let base_width = value_dims.width + (menu_padding * 2.0);
            let base_height = value_dims.height + (menu_padding * 2.0);
            let scaled_width = base_width * cursor_scale;
            let scaled_height = base_height * cursor_scale;
            let offset_x = (scaled_width - base_width) / 2.0;
            let offset_y = (scaled_height - base_height) / 2.0;

            let rect_x = value_x - menu_padding;
            let rect_y = y_pos + (menu_option_height / 2.0) - (base_height / 2.0);

            draw_rectangle_lines(rect_x - offset_x, rect_y - offset_y, scaled_width, scaled_height, 4.0 * scale_factor, cursor_color);
        }

        // Draw label
        text_with_config_color(font_cache, config, label, left_margin, text_y, font_size);

        // Draw value (with cursor color if selected and TEXT style)
        if is_selected && config.cursor_style == "TEXT" {
            let highlight_color = animation_state.get_cursor_color(config);
            text_with_color(font_cache, config, &value, value_x, text_y, font_size, highlight_color);
        } else {
            text_with_config_color(font_cache, config, &value, value_x, text_y, font_size);
        }

        // Show editing indicator
        if (i == 1 && ra_state.editing_username) || (i == 2 && ra_state.editing_api_key) {
            let blink = (get_time() * 3.0) as i32 % 2 == 0;
            if blink {
                let cursor_text = "_";
                let cursor_x = value_x + value_dims.width;
                text_with_config_color(font_cache, config, cursor_text, cursor_x, text_y, font_size);
            }
        }
    }

    // Status message at bottom
    if let Some(ref msg) = ra_state.status_message {
        let msg_dims = measure_text(msg, Some(current_font), font_size, 1.0);
        let msg_x = screen_width() / 2.0 - msg_dims.width / 2.0;
        let msg_y = screen_height() - 40.0 * scale_factor;
        let msg_color = if msg.contains("success") || msg.contains("successful") {
            GREEN
        } else if msg.contains("fail") || msg.contains("Error") {
            RED
        } else {
            YELLOW
        };
        text_with_color(font_cache, config, msg, msg_x, msg_y, font_size, msg_color);
    }

    // Instructions
    let instructions = if ra_state.editing_username || ra_state.editing_api_key {
        "Type to enter text, ENTER/B to confirm"
    } else {
        "Get your API key from retroachievements.org > Settings > Keys"
    };
    let inst_dims = measure_text(instructions, Some(current_font), font_size, 1.0);
    let inst_x = screen_width() / 2.0 - inst_dims.width / 2.0;
    let inst_y = screen_height() - 20.0 * scale_factor;
    text_with_color(font_cache, config, instructions, inst_x, inst_y, font_size, Color::new(0.5, 0.5, 0.5, 1.0));
}

/// Get the display value for each option
fn get_option_value(index: usize, ra_state: &RASettingsState, config: &Config) -> String {
    match index {
        0 => if config.retroachievements.enabled { "ON" } else { "OFF" }.to_string(),
        1 => {
            if ra_state.editing_username {
                ra_state.username_input.clone()
            } else if ra_state.is_logged_in {
                ra_state.logged_in_user.clone().unwrap_or_default()
            } else if !ra_state.username_input.is_empty() {
                ra_state.username_input.clone()
            } else {
                "[ENTER]".to_string()
            }
        }
        2 => {
            if ra_state.editing_api_key {
                "*".repeat(ra_state.api_key_input.len())
            } else if ra_state.is_logged_in {
                "********".to_string()
            } else if !ra_state.api_key_input.is_empty() {
                "*".repeat(ra_state.api_key_input.len())
            } else {
                "[ENTER]".to_string()
            }
        }
        3 => if config.retroachievements.hardcore_mode { "ON" } else { "OFF" }.to_string(),
        4 => if config.retroachievements.show_notifications { "ON" } else { "OFF" }.to_string(),
        5 => if ra_state.is_logged_in { "TEST" } else { "LOGIN" }.to_string(),
        6 => "CONFIRM".to_string(),
        _ => "".to_string(),
    }
}

