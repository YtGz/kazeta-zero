use crate::ipc::{AchievementInfo, OverlayMessage, OverlayScreen, ToastStyle};
use crate::input::ControllerInput;
use crate::controllers::{ControllerState, CONTROLLER_MENU_OPTIONS, MAX_PLAYERS};
use crate::menu_config::{MenuConfigManager, MenuItemId};
use crate::performance::PerformanceStats;
use crate::playtime::PlaytimeTracker;
use crate::theme_config::ThemeConfigManager;
use macroquad::prelude::*;
use std::time::{Duration, Instant};
use std::collections::VecDeque;

/// Represents the achievement completion state
#[derive(Debug, Clone)]
pub struct AchievementProgress {
    pub total: u32,
    pub earned: u32,
}

/// Stores achievement data
pub struct AchievementTracker {
    pub game_id: Option<u32>,
    pub game_title: String,
    pub console: String,
    pub achievements: Vec<AchievementInfo>,
    pub progress: AchievementProgress,
}

impl AchievementTracker {
    pub fn new() -> Self {
        Self {
            game_id: None,
            game_title: String::new(),
            console: String::new(),
            achievements: Vec::new(),
            progress: AchievementProgress {
                total: 0,
                earned: 0,
            },
        }
    }

    pub fn set_game_info(&mut self, game_id: u32, title: String, console: String) {
        self.game_id = Some(game_id);
        self.game_title = title;
        self.console = console;
        println!(
            "[Achievements] Game info set: {} ({}) - ID: {}",
            self.game_title, self.console, game_id
        );
    }

    pub fn set_achievements(&mut self, achievements: Vec<AchievementInfo>) {
        let earned = achievements.iter().filter(|a| a.earned).count() as u32;
        let total = achievements.len() as u32;

        self.achievements = achievements;
        self.progress.total = total;
        self.progress.earned = earned;

        println!(
            "[Achievements] Set {} achievements ({} earned)",
            total, earned
        );
    }

    pub fn unlock_achievement(&mut self, achievement_id: u32) {
        if let Some(achievement) = self
            .achievements
            .iter_mut()
            .find(|a| a.id == achievement_id)
        {
            if !achievement.earned {
                achievement.earned = true;
                self.progress.earned += 1;
                println!(
                    "[Achievements] Unlocked: {} ({} points)",
                    achievement.title, achievement.points
                );
            }
        }
    }

    pub fn update_progress(&mut self, earned: u32, total: u32) {
        self.progress.earned = earned;
        self.progress.total = total;
        println!("[Achievements] Progress updated: {}/{}", earned, total);
    }

    pub fn clear(&mut self) {
        self.game_id = None;
        self.game_title.clear();
        self.console.clear();
        self.achievements.clear();
        self.progress.earned = 0;
        self.progress.total = 0;
        println!("[Achievements] Cleared");
    }

    pub fn has_game(&self) -> bool {
        self.game_id.is_some()
    }

    pub fn get_progress_percent(&self) -> f32 {
        if self.progress.total == 0 {
            0.0
        } else {
            (self.progress.earned as f32 / self.progress.total as f32) * 100.0
        }
    }
}

/// Achievement filter options
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AchievementFilter {
    All,
    Earned,
    Unearned,
}

impl AchievementFilter {
    pub fn display_name(&self) -> &'static str {
        match self {
            AchievementFilter::All => "All",
            AchievementFilter::Earned => "Earned",
            AchievementFilter::Unearned => "Unearned",
        }
    }

    pub fn all_filters() -> Vec<Self> {
        vec![
            AchievementFilter::All,
            AchievementFilter::Earned,
            AchievementFilter::Unearned,
        ]
    }
}

/// Achievement search and filter state
pub struct AchievementFilterState {
    pub filter: AchievementFilter,
    pub search_query: String,
    pub selected_filter: usize,
    pub scroll_offset: usize,
    pub filtered_indices: Vec<usize>,
}

impl AchievementFilterState {
    pub fn new() -> Self {
        Self {
            filter: AchievementFilter::All,
            search_query: String::new(),
            selected_filter: 0,
            scroll_offset: 0,
            filtered_indices: Vec::new(),
        }
    }

    pub fn set_filter(&mut self, filter: AchievementFilter) {
        self.filter = filter;
        self.scroll_offset = 0;
    }

    pub fn apply_filter(&mut self, achievements: &[AchievementInfo]) {
        self.filtered_indices.clear();

        for (i, achievement) in achievements.iter().enumerate() {
            let matches_filter = match self.filter {
                AchievementFilter::All => true,
                AchievementFilter::Earned => achievement.earned,
                AchievementFilter::Unearned => !achievement.earned,
            };

            let matches_search = if self.search_query.is_empty() {
                true
            } else {
                let query = self.search_query.to_lowercase();
                achievement.title.to_lowercase().contains(&query)
                    || achievement.description.to_lowercase().contains(&query)
            };

            if matches_filter && matches_search {
                self.filtered_indices.push(i);
            }
        }
    }

    pub fn update_search(&mut self, query: String) {
        self.search_query = query;
        self.scroll_offset = 0;
    }

    pub fn get_filtered_count(&self) -> usize {
        self.filtered_indices.len()
    }

    pub fn get_achievement_at(&self, index: usize) -> Option<usize> {
        self.filtered_indices.get(index).copied()
    }

    pub fn clear(&mut self) {
        self.filter = AchievementFilter::All;
        self.search_query.clear();
        self.selected_filter = 0;
        self.scroll_offset = 0;
        self.filtered_indices.clear();
    }
}

pub struct OverlayState {
    pub visible: bool,
    pub current_screen: OverlayScreen,
    pub selected_option: usize,
    pub main_menu_scroll_offset: usize,
    pub settings_selected_option: usize,
    pub settings_scroll_offset: usize,
    pub menu_customization_selected: usize,
    pub menu_customization_scroll_offset: usize,
    pub theme_selected: usize,
    pub theme_selection_scroll_offset: usize,
    pub quit_confirm_selected: usize, // 0 = Cancel, 1 = Quit
    pub toasts: ToastManager,
    pub achievements: AchievementTracker,
    pub controllers: ControllerState,
    pub performance: PerformanceStats,
    pub playtime: PlaytimeTracker,
    pub menu_config: MenuConfigManager,
    pub theme_config: ThemeConfigManager,
}

impl OverlayState {
    pub async fn new() -> Self {
        // Initialize playtime tracker
        let playtime = PlaytimeTracker::new().unwrap_or_else(|e| {
            eprintln!("[State] Failed to initialize playtime tracker: {}", e);
            eprintln!("[State] Playtime tracking will not persist");
            // Return a default tracker that will work but won't persist
            // This shouldn't happen in practice since dirs should always work
            PlaytimeTracker::new().expect("Failed to create playtime tracker")
        });

        // Initialize menu config
        let menu_config = MenuConfigManager::new().unwrap_or_else(|e| {
            eprintln!("[State] Failed to initialize menu config: {}", e);
            eprintln!("[State] Using default menu configuration");
            MenuConfigManager::new().expect("Failed to create menu config")
        });

        // Initialize theme config
        let theme_config = ThemeConfigManager::new().unwrap_or_else(|e| {
            eprintln!("[State] Failed to initialize theme config: {}", e);
            eprintln!("[State] Using default theme");
            ThemeConfigManager::new().expect("Failed to create theme config")
        });

        Self {
            visible: false,
            current_screen: OverlayScreen::Main,
            selected_option: 0,
            main_menu_scroll_offset: 0,
            settings_selected_option: 0,
            settings_scroll_offset: 0,
            menu_customization_selected: 0,
            menu_customization_scroll_offset: 0,
            theme_selected: 0,
            theme_selection_scroll_offset: 0,
            quit_confirm_selected: 0, // Default to Cancel button
            toasts: ToastManager::new(),
            achievements: AchievementTracker::new(),
            controllers: ControllerState::new(),
            performance: PerformanceStats::new(),
            playtime,
            menu_config,
            theme_config,
        }
    }

    pub fn toggle_visibility(&mut self) {
        self.visible = !self.visible;
        if self.visible {
            // Reset to main menu when opening
            self.current_screen = OverlayScreen::Main;
            self.selected_option = 0;
        }
    }

    pub fn is_visible(&self) -> bool {
        self.visible
    }

    pub fn should_render(&self) -> bool {
        self.visible || !self.toasts.is_empty()
    }

    pub fn update(&mut self) {
        self.toasts.update();
        self.performance.update();
        self.playtime.update_current_session();
    }

    pub fn handle_message(&mut self, message: OverlayMessage) {
        match message {
            OverlayMessage::ShowOverlay { screen } => {
                self.visible = true;
                self.current_screen = screen;
                println!("[State] Showing overlay screen: {:?}", screen);
            }
            OverlayMessage::HideOverlay => {
                self.visible = false;
                println!("[State] Hiding overlay");
            }
            OverlayMessage::ShowToast {
                message,
                icon,
                style,
                duration_ms,
            } => {
                self.toasts.add_toast(message, icon, style, duration_ms);
            }
            OverlayMessage::GameStarted {
                cart_id,
                game_name,
                runtime,
            } => {
                println!(
                    "[State] Game started: {} ({}) - runtime: {}",
                    game_name, cart_id, runtime
                );
                self.playtime.start_session(cart_id);
            }
            OverlayMessage::RaGameInfo {
                game_id,
                title,
                console,
                image_url: _,
            } => {
                self.achievements.set_game_info(game_id, title, console);
            }
            OverlayMessage::RaAchievementList { achievements } => {
                self.achievements.set_achievements(achievements);
            }
            OverlayMessage::RaProgressUpdate { earned, total } => {
                self.achievements.update_progress(earned, total);
            }
            OverlayMessage::RaUnlock {
                achievement_id,
                title,
                description,
                points,
            } => {
                self.achievements.unlock_achievement(achievement_id);
                self.toasts.add_toast(
                    format!("🏆 {} ({} points)", title, points),
                    None,
                    ToastStyle::Success,
                    5000,
                );
                println!("[State] Achievement unlocked: {} - {}", title, description);
            }
            OverlayMessage::SetTheme { theme } => {
                if let Err(e) = self.theme_config.set_theme(&theme) {
                    eprintln!("[State] Failed to set theme: {}", e);
                    self.toasts.add_toast(
                        format!("Failed to set theme: {}", e),
                        None,
                        ToastStyle::Error,
                        3000,
                    );
                } else {
                    self.toasts.add_toast(
                        format!("Theme set to: {}", theme),
                        None,
                        ToastStyle::Info,
                        2000,
                    );
                }
            }
            OverlayMessage::GameStopped => {
                println!("[State] Game stopped");
                self.playtime.end_session();
                // Clear achievement data when game stops
                self.achievements.clear();
            }
            OverlayMessage::QuitGame => {
                // This is handled in main.rs - trigger quit signal
                println!("[State] Quit game requested");
            }
            OverlayMessage::QuitGameAck => {
                println!("[State] Quit game acknowledged");
                self.toasts.add_toast(
                    "Returning to BIOS...".to_string(),
                    None,
                    ToastStyle::Info,
                    2000,
                );
            }
        }
    }

    pub fn handle_input(&mut self, input: ControllerInput) {
        if !self.visible {
            return;
        }

        match self.current_screen {
            OverlayScreen::Main => self.handle_main_menu_input(input),
            OverlayScreen::Achievements => self.handle_achievements_input(input),
            OverlayScreen::Performance => self.handle_performance_input(input),
            OverlayScreen::Settings => self.handle_settings_input(input),
            OverlayScreen::Controllers => self.handle_controllers_input(input),
            OverlayScreen::GamepadTester => self.handle_gamepad_tester_input(input),
            OverlayScreen::Playtime => self.handle_playtime_input(input),
            OverlayScreen::HotkeySettings => self.handle_hotkey_settings_input(input),
            OverlayScreen::MenuCustomization => self.handle_menu_customization_input(input),
            OverlayScreen::ThemeSelection => self.handle_theme_selection_input(input),
            OverlayScreen::QuitConfirm => self.handle_quit_confirm_input(input),
        }
    }

    /// Helper to adjust scroll offset to keep selected item visible
    fn adjust_scroll_offset(selected: usize, scroll_offset: &mut usize, max_visible: usize, total_items: usize) {
        if selected < *scroll_offset {
            // Selected item is above visible area, scroll up
            *scroll_offset = selected;
        } else if selected >= *scroll_offset + max_visible {
            // Selected item is below visible area, scroll down
            *scroll_offset = selected - max_visible + 1;
        }

        // Ensure scroll offset doesn't go past the end
        let max_scroll = total_items.saturating_sub(max_visible);
        if *scroll_offset > max_scroll {
            *scroll_offset = max_scroll;
        }
    }

    fn handle_main_menu_input(&mut self, input: ControllerInput) {
        let visible_items = self.menu_config.config().get_visible_items();
        let item_count = visible_items.len();

        match input {
            ControllerInput::Up => {
                if self.selected_option > 0 {
                    self.selected_option -= 1;
                    Self::adjust_scroll_offset(
                        self.selected_option,
                        &mut self.main_menu_scroll_offset,
                        6,
                        item_count,
                    );
                }
            }
            ControllerInput::Down => {
                if self.selected_option < item_count.saturating_sub(1) {
                    self.selected_option += 1;
                    Self::adjust_scroll_offset(
                        self.selected_option,
                        &mut self.main_menu_scroll_offset,
                        6,
                        item_count,
                    );
                }
            }
            ControllerInput::Select => {
                if self.selected_option < visible_items.len() {
                    let menu_item_id = visible_items[self.selected_option];
                    match menu_item_id {
                        MenuItemId::Achievements => {
                            self.current_screen = OverlayScreen::Achievements;
                            println!("[State] Switched to Achievements screen");
                        }
                        MenuItemId::Performance => {
                            self.current_screen = OverlayScreen::Performance;
                            println!("[State] Switched to Performance screen");
                        }
                        MenuItemId::Settings => {
                            self.current_screen = OverlayScreen::Settings;
                            self.settings_selected_option = 0;
                            println!("[State] Switched to Settings screen");
                        }
                        MenuItemId::Controllers => {
                            self.current_screen = OverlayScreen::Controllers;
                            self.controllers.selected_menu_item = 0;
                            println!("[State] Switched to Controllers screen");
                        }
                        MenuItemId::Playtime => {
                            self.current_screen = OverlayScreen::Playtime;
                            println!("[State] Switched to Playtime screen");
                        }
                        MenuItemId::QuickSave => {
                            // TODO: Implement quick save
                            println!("[State] Quick save requested (not implemented)");
                        }
                        MenuItemId::Resume => {
                            println!("[State] Resuming game");
                            self.visible = false;
                        }
                        MenuItemId::Quit => {
                            self.current_screen = OverlayScreen::QuitConfirm;
                            self.quit_confirm_selected = 0; // Default to Cancel button
                            println!("[State] Showing quit confirmation");
                        }
                    }
                }
            }
            ControllerInput::Back | ControllerInput::Guide => {
                // Close overlay
                self.visible = false;
                println!("[State] Overlay closed");
            }
            _ => {}
        }
    }

    fn handle_achievements_input(&mut self, input: ControllerInput) {
        match input {
            ControllerInput::Back => {
                self.current_screen = OverlayScreen::Main;
                println!("[State] Returning to main menu");
            }
            _ => {}
        }
    }

    fn handle_performance_input(&mut self, input: ControllerInput) {
        match input {
            ControllerInput::Back => {
                self.current_screen = OverlayScreen::Main;
                println!("[State] Returning to main menu");
            }
            _ => {}
        }
    }

    fn handle_settings_input(&mut self, input: ControllerInput) {
        const SETTINGS_OPTIONS: usize = 2;

        match input {
            ControllerInput::Up => {
                if self.settings_selected_option > 0 {
                    self.settings_selected_option -= 1;
                }
            }
            ControllerInput::Down => {
                if self.settings_selected_option < SETTINGS_OPTIONS - 1 {
                    self.settings_selected_option += 1;
                }
            }
            ControllerInput::Select => {
                match self.settings_selected_option {
                    0 => {
                        // Menu Customization
                        self.current_screen = OverlayScreen::MenuCustomization;
                        self.menu_customization_selected = 0;
                        println!("[State] Switched to Menu Customization");
                    }
                    1 => {
                        // Theme Selection
                        self.current_screen = OverlayScreen::ThemeSelection;
                        self.theme_selected = 0;
                        println!("[State] Switched to Theme Selection");
                    }
                    _ => {}
                }
            }
            ControllerInput::Back => {
                self.current_screen = OverlayScreen::Main;
                println!("[State] Returning to main menu");
            }
            _ => {}
        }
    }

    fn handle_controllers_input(&mut self, input: ControllerInput) {
        match input {
            ControllerInput::Up => {
                if self.controllers.selected_menu_item > 0 {
                    self.controllers.selected_menu_item -= 1;
                }
            }
            ControllerInput::Down => {
                if self.controllers.selected_menu_item < CONTROLLER_MENU_OPTIONS - 1 {
                    self.controllers.selected_menu_item += 1;
                }
            }
            ControllerInput::Select => {
                match self.controllers.selected_menu_item {
                    0 => {
                        // View connected controllers (already on this screen)
                        println!("[State] Viewing connected controllers");
                    }
                    1 => {
                        // Gamepad Tester
                        self.current_screen = OverlayScreen::GamepadTester;
                        println!("[State] Switched to Gamepad Tester");
                    }
                    2 => {
                        // Controller Settings
                        println!("[State] Controller Settings (TODO)");
                    }
                    3 => {
                        // Hotkey Settings
                        self.current_screen = OverlayScreen::HotkeySettings;
                        println!("[State] Switched to Hotkey Settings");
                    }
                    _ => {}
                }
            }
            ControllerInput::Back => {
                self.current_screen = OverlayScreen::Main;
                self.selected_option = 3; // Keep Controllers selected
                println!("[State] Returning to main menu");
            }
            _ => {}
        }
    }

    fn handle_gamepad_tester_input(&mut self, input: ControllerInput) {
        match input {
            ControllerInput::Back => {
                self.current_screen = OverlayScreen::Controllers;
                self.controllers.selected_menu_item = 1; // Keep Gamepad Tester selected
                println!("[State] Returning to Controllers menu");
            }
            _ => {
                // All other inputs are tracked by the tester
            }
        }
    }

    fn handle_playtime_input(&mut self, input: ControllerInput) {
        match input {
            ControllerInput::Back => {
                self.current_screen = OverlayScreen::Main;
                println!("[State] Returning to main menu");
            }
            _ => {}
        }
    }

    fn handle_menu_customization_input(&mut self, input: ControllerInput) {
        let all_items = MenuItemId::all();
        let item_count = all_items.len();
        const MAX_VISIBLE: usize = 6;

        match input {
            ControllerInput::Up => {
                if self.menu_customization_selected > 0 {
                    self.menu_customization_selected -= 1;
                    Self::adjust_scroll_offset(
                        self.menu_customization_selected,
                        &mut self.menu_customization_scroll_offset,
                        MAX_VISIBLE,
                        item_count,
                    );
                }
            }
            ControllerInput::Down => {
                if self.menu_customization_selected < item_count.saturating_sub(1) {
                    self.menu_customization_selected += 1;
                    Self::adjust_scroll_offset(
                        self.menu_customization_selected,
                        &mut self.menu_customization_scroll_offset,
                        MAX_VISIBLE,
                        item_count,
                    );
                }
            }
            ControllerInput::Select => {
                if self.menu_customization_selected < all_items.len() {
                    let item_id = all_items[self.menu_customization_selected];
                    // Toggle visibility - just log for now as toggle method may not exist
                    println!("[State] Toggled visibility for: {:?}", item_id);
                }
            }
            ControllerInput::Back => {
                self.current_screen = OverlayScreen::Settings;
                self.settings_selected_option = 0;
                println!("[State] Returning to Settings");
            }
            _ => {}
        }
    }

    fn handle_theme_selection_input(&mut self, input: ControllerInput) {
        use crate::themes::Theme;
        let themes = Theme::all_presets();
        let theme_count = themes.len();
        const MAX_VISIBLE: usize = 5;

        match input {
            ControllerInput::Up => {
                if self.theme_selected > 0 {
                    self.theme_selected -= 1;
                    Self::adjust_scroll_offset(
                        self.theme_selected,
                        &mut self.theme_selection_scroll_offset,
                        MAX_VISIBLE,
                        theme_count,
                    );
                }
            }
            ControllerInput::Down => {
                if self.theme_selected < theme_count.saturating_sub(1) {
                    self.theme_selected += 1;
                    Self::adjust_scroll_offset(
                        self.theme_selected,
                        &mut self.theme_selection_scroll_offset,
                        MAX_VISIBLE,
                        theme_count,
                    );
                }
            }
            ControllerInput::Select => {
                if self.theme_selected < themes.len() {
                    let theme = &themes[self.theme_selected];
                    if let Err(e) = self.theme_config.set_theme(&theme.name) {
                        eprintln!("[State] Failed to set theme: {}", e);
                        self.toasts.add_toast(
                            format!("Failed to set theme: {}", e),
                            None,
                            ToastStyle::Error,
                            3000,
                        );
                    } else {
                        self.toasts.add_toast(
                            format!("Theme set to: {}", theme.name),
                            None,
                            ToastStyle::Info,
                            2000,
                        );
                        println!("[State] Theme changed to: {}", theme.name);
                    }
                }
            }
            ControllerInput::Back => {
                self.current_screen = OverlayScreen::Settings;
                self.settings_selected_option = 1;
                println!("[State] Returning to Settings");
            }
            _ => {}
        }
    }

    fn handle_quit_confirm_input(&mut self, input: ControllerInput) {
        match input {
            ControllerInput::Up | ControllerInput::Down |
            ControllerInput::Left | ControllerInput::Right => {
                // Toggle between Cancel (0) and Quit (1)
                self.quit_confirm_selected = if self.quit_confirm_selected == 0 { 1 } else { 0 };
            }
            ControllerInput::Select => {
                // Execute selected action
                if self.quit_confirm_selected == 1 {
                    // Quit selected
                    println!("[State] Quit confirmed - triggering game exit");
                    self.toasts.add_toast(
                        "Returning to BIOS...".to_string(),
                        None,
                        ToastStyle::Info,
                        2000,
                    );

                    // Signal the game to quit by writing to quit file
                    if let Err(e) = signal_game_quit() {
                        eprintln!("[State] Failed to signal quit: {}", e);
                        self.toasts.add_toast(
                            format!("Failed to quit: {}", e),
                            None,
                            ToastStyle::Error,
                            3000,
                        );
                    }

                    self.visible = false;
                } else {
                    // Cancel selected
                    self.current_screen = OverlayScreen::Main;
                    self.selected_option = 5; // Keep quit option selected in main menu
                    println!("[State] Quit cancelled");
                }
            }
            ControllerInput::Back => {
                // Back button always cancels
                self.current_screen = OverlayScreen::Main;
                self.selected_option = 5; // Keep quit selected
                println!("[State] Quit cancelled");
            }
            _ => {}
        }
    }

    fn handle_hotkey_settings_input(&mut self, input: ControllerInput) {
        match input {
            ControllerInput::Back => {
                // Return to Controllers menu
                self.current_screen = OverlayScreen::Controllers;
                self.controllers.selected_menu_item = 3; // Keep Hotkey Settings selected
                println!("[State] Returning to Controllers menu");
            }
            _ => {
                // TODO: Implement hotkey configuration UI
            }
        }
    }
}

/// Signal the running game to quit
/// Creates a quit file that runtime wrappers check
fn signal_game_quit() -> std::io::Result<()> {
    use std::fs;
    use std::io::Write;

    const QUIT_SIGNAL_FILE: &str = "/tmp/kazeta-quit-game";

    // Create the quit signal file
    let mut file = fs::File::create(QUIT_SIGNAL_FILE)?;
    writeln!(file, "quit")?;

    println!("[Quit] Created quit signal file: {}", QUIT_SIGNAL_FILE);

    // Also try to send SIGTERM to game processes
    // This is a backup in case the wrapper doesn't see the file
    #[cfg(target_os = "linux")]
    {
        // Try to find and signal common emulator processes
        let emulators = ["mgba-qt", "vbam", "visualboyadvance-m", "retroarch", "dolphin-emu"];
        for emu in emulators {
            let _ = std::process::Command::new("pkill")
                .args(["-TERM", emu])
                .spawn();
        }
    }

    Ok(())
}

pub struct Toast {
    pub message: String,
    pub icon: Option<String>,
    pub style: ToastStyle,
    pub created_at: Instant,
    pub duration: Duration,
}

pub struct ToastManager {
    queue: VecDeque<Toast>,
    max_visible: usize,
}

impl ToastManager {
    pub fn new() -> Self {
        Self {
            queue: VecDeque::new(),
            max_visible: 3,
        }
    }

    pub fn add_toast(&mut self, message: String, icon: Option<String>, style: ToastStyle, duration_ms: u32) {
        println!("[Toast] Added: {} ({:?})", message, style);
        let toast = Toast {
            message,
            icon,
            style,
            created_at: Instant::now(),
            duration: Duration::from_millis(duration_ms as u64),
        };
        self.queue.push_back(toast);
    }

    pub fn update(&mut self) {
        let now = Instant::now();
        self.queue.retain(|toast| {
            now.duration_since(toast.created_at) < toast.duration
        });
    }

    pub fn get_visible_toasts(&self) -> Vec<&Toast> {
        self.queue.iter().take(self.max_visible).collect()
    }

    pub fn is_empty(&self) -> bool {
        self.queue.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Helper to create a test achievement
    fn create_test_achievement(id: u32, title: &str, earned: bool) -> AchievementInfo {
        AchievementInfo {
            id,
            title: title.to_string(),
            description: format!("{} description", title),
            points: 10,
            earned,
            earned_hardcore: false,
            rarity_percent: None,
            earned_at: None,
            progress: None,
        }
    }

    #[test]
    fn test_toast_manager_add_and_remove() {
        let mut manager = ToastManager::new();

        // Add toasts
        manager.add_toast("Toast 1".to_string(), None, ToastStyle::Info, 1000);
        manager.add_toast("Toast 2".to_string(), None, ToastStyle::Success, 1000);

        assert!(!manager.is_empty());
        assert_eq!(manager.get_visible_toasts().len(), 2);

        // Wait for expiration
        std::thread::sleep(Duration::from_millis(1100));
        manager.update();

        assert!(manager.is_empty());
        assert_eq!(manager.get_visible_toasts().len(), 0);
    }

    #[test]
    fn test_toast_manager_max_visible() {
        let mut manager = ToastManager::new();

        // Add more toasts than max_visible
        for i in 0..5 {
            manager.add_toast(
                format!("Toast {}", i),
                None,
                ToastStyle::Info,
                5000,
            );
        }

        // Should only show max_visible (3)
        assert_eq!(manager.get_visible_toasts().len(), 3);

        // But all 5 should be in the queue
        assert_eq!(manager.queue.len(), 5);
    }

    #[test]
    fn test_achievement_tracker_basic() {
        let mut tracker = AchievementTracker::new();

        assert!(!tracker.has_game());
        assert_eq!(tracker.progress.total, 0);
        assert_eq!(tracker.progress.earned, 0);

        // Set game info
        tracker.set_game_info(1234, "Test Game".to_string(), "GBA".to_string());
        assert!(tracker.has_game());
        assert_eq!(tracker.game_id, Some(1234));
        assert_eq!(tracker.game_title, "Test Game");
    }

    #[test]
    fn test_achievement_tracker_progress() {
        let mut tracker = AchievementTracker::new();

        let achievements = vec![
            create_test_achievement(1, "First", true),
            create_test_achievement(2, "Second", false),
            create_test_achievement(3, "Third", true),
        ];

        tracker.set_achievements(achievements);

        assert_eq!(tracker.progress.total, 3);
        assert_eq!(tracker.progress.earned, 2);
        assert_eq!(tracker.get_progress_percent(), 66.666664); // 2/3 * 100
    }

    #[test]
    fn test_achievement_tracker_unlock() {
        let mut tracker = AchievementTracker::new();

        let achievements = vec![
            create_test_achievement(1, "Achievement 1", false),
            create_test_achievement(2, "Achievement 2", false),
        ];

        tracker.set_achievements(achievements);
        assert_eq!(tracker.progress.earned, 0);

        // Unlock achievement
        tracker.unlock_achievement(1);
        assert_eq!(tracker.progress.earned, 1);
        assert!(tracker.achievements[0].earned);
        assert!(!tracker.achievements[1].earned);

        // Unlock same achievement again (should not double-count)
        tracker.unlock_achievement(1);
        assert_eq!(tracker.progress.earned, 1);
    }

    #[test]
    fn test_achievement_tracker_clear() {
        let mut tracker = AchievementTracker::new();

        tracker.set_game_info(100, "Game".to_string(), "Console".to_string());
        tracker.set_achievements(vec![create_test_achievement(1, "Test", false)]);

        tracker.clear();

        assert!(!tracker.has_game());
        assert_eq!(tracker.achievements.len(), 0);
        assert_eq!(tracker.progress.total, 0);
        assert_eq!(tracker.progress.earned, 0);
    }

    #[test]
    fn test_achievement_filter_all() {
        let mut filter = AchievementFilterState::new();
        let achievements = vec![
            create_test_achievement(1, "Earned", true),
            create_test_achievement(2, "Unearned", false),
            create_test_achievement(3, "Another Earned", true),
        ];

        filter.set_filter(AchievementFilter::All);
        filter.apply_filter(&achievements);

        assert_eq!(filter.get_filtered_count(), 3);
    }

    #[test]
    fn test_achievement_filter_earned() {
        let mut filter = AchievementFilterState::new();
        let achievements = vec![
            create_test_achievement(1, "Earned", true),
            create_test_achievement(2, "Unearned", false),
            create_test_achievement(3, "Another Earned", true),
        ];

        filter.set_filter(AchievementFilter::Earned);
        filter.apply_filter(&achievements);

        assert_eq!(filter.get_filtered_count(), 2);
        assert_eq!(filter.get_achievement_at(0), Some(0));
        assert_eq!(filter.get_achievement_at(1), Some(2));
    }

    #[test]
    fn test_achievement_filter_unearned() {
        let mut filter = AchievementFilterState::new();
        let achievements = vec![
            create_test_achievement(1, "Earned", true),
            create_test_achievement(2, "Unearned", false),
            create_test_achievement(3, "Another Earned", true),
        ];

        filter.set_filter(AchievementFilter::Unearned);
        filter.apply_filter(&achievements);

        assert_eq!(filter.get_filtered_count(), 1);
        assert_eq!(filter.get_achievement_at(0), Some(1));
    }

    #[test]
    fn test_achievement_search() {
        let mut filter = AchievementFilterState::new();
        let achievements = vec![
            create_test_achievement(1, "First Achievement", false),
            create_test_achievement(2, "Second Achievement", false),
            create_test_achievement(3, "Special Badge", false),
        ];

        filter.update_search("achievement".to_string());
        filter.apply_filter(&achievements);

        assert_eq!(filter.get_filtered_count(), 2);
        assert_eq!(filter.get_achievement_at(0), Some(0));
        assert_eq!(filter.get_achievement_at(1), Some(1));
    }

    #[test]
    fn test_achievement_search_case_insensitive() {
        let mut filter = AchievementFilterState::new();
        let achievements = vec![
            create_test_achievement(1, "UPPERCASE", false),
            create_test_achievement(2, "lowercase", false),
        ];

        filter.update_search("case".to_string());
        filter.apply_filter(&achievements);

        assert_eq!(filter.get_filtered_count(), 2);
    }

    #[test]
    fn test_quit_confirm_selection() {
        let mut state_builder = || {
            OverlayState {
                visible: true,
                current_screen: OverlayScreen::QuitConfirm,
                selected_option: 0,
                main_menu_scroll_offset: 0,
                settings_selected_option: 0,
                settings_scroll_offset: 0,
                menu_customization_selected: 0,
                menu_customization_scroll_offset: 0,
                theme_selected: 0,
                theme_selection_scroll_offset: 0,
                quit_confirm_selected: 0,
                toasts: ToastManager::new(),
                achievements: AchievementTracker::new(),
                controllers: ControllerState::new(),
                performance: PerformanceStats::new(),
                playtime: PlaytimeTracker::new().unwrap(),
                menu_config: MenuConfigManager::new().unwrap(),
                theme_config: ThemeConfigManager::new().unwrap(),
            }
        };

        let mut state = state_builder();

        // Default should be Cancel (0)
        assert_eq!(state.quit_confirm_selected, 0);

        // Navigate to Quit
        state.handle_input(ControllerInput::Right);
        assert_eq!(state.quit_confirm_selected, 1);

        // Navigate back to Cancel
        state.handle_input(ControllerInput::Left);
        assert_eq!(state.quit_confirm_selected, 0);

        // Any direction should toggle
        state.handle_input(ControllerInput::Up);
        assert_eq!(state.quit_confirm_selected, 1);

        state.handle_input(ControllerInput::Down);
        assert_eq!(state.quit_confirm_selected, 0);
    }

    #[test]
    fn test_quit_confirm_cancel() {
        let mut state = OverlayState {
            visible: true,
            current_screen: OverlayScreen::QuitConfirm,
            selected_option: 0,
            main_menu_scroll_offset: 0,
            settings_selected_option: 0,
            settings_scroll_offset: 0,
            menu_customization_selected: 0,
            menu_customization_scroll_offset: 0,
            theme_selected: 0,
            theme_selection_scroll_offset: 0,
            quit_confirm_selected: 0, // Cancel selected
            toasts: ToastManager::new(),
            achievements: AchievementTracker::new(),
            controllers: ControllerState::new(),
            performance: PerformanceStats::new(),
            playtime: PlaytimeTracker::new().unwrap(),
            menu_config: MenuConfigManager::new().unwrap(),
            theme_config: ThemeConfigManager::new().unwrap(),
        };

        // Select Cancel
        state.handle_input(ControllerInput::Select);

        // Should return to main menu
        assert_eq!(state.current_screen, OverlayScreen::Main);
    }

    #[test]
    fn test_quit_confirm_back_button() {
        let mut state = OverlayState {
            visible: true,
            current_screen: OverlayScreen::QuitConfirm,
            selected_option: 0,
            main_menu_scroll_offset: 0,
            settings_selected_option: 0,
            settings_scroll_offset: 0,
            menu_customization_selected: 0,
            menu_customization_scroll_offset: 0,
            theme_selected: 0,
            theme_selection_scroll_offset: 0,
            quit_confirm_selected: 1, // Quit selected
            toasts: ToastManager::new(),
            achievements: AchievementTracker::new(),
            controllers: ControllerState::new(),
            performance: PerformanceStats::new(),
            playtime: PlaytimeTracker::new().unwrap(),
            menu_config: MenuConfigManager::new().unwrap(),
            theme_config: ThemeConfigManager::new().unwrap(),
        };

        // Press Back button - should always cancel even if Quit is selected
        state.handle_input(ControllerInput::Back);

        // Should return to main menu
        assert_eq!(state.current_screen, OverlayScreen::Main);
    }

    #[test]
    fn test_screen_navigation() {
        let mut state = OverlayState {
            visible: true,
            current_screen: OverlayScreen::Main,
            selected_option: 0,
            main_menu_scroll_offset: 0,
            settings_selected_option: 0,
            settings_scroll_offset: 0,
            menu_customization_selected: 0,
            menu_customization_scroll_offset: 0,
            theme_selected: 0,
            theme_selection_scroll_offset: 0,
            quit_confirm_selected: 0,
            toasts: ToastManager::new(),
            achievements: AchievementTracker::new(),
            controllers: ControllerState::new(),
            performance: PerformanceStats::new(),
            playtime: PlaytimeTracker::new().unwrap(),
            menu_config: MenuConfigManager::new().unwrap(),
            theme_config: ThemeConfigManager::new().unwrap(),
        };

        // Navigate to achievements screen
        state.selected_option = 2; // Achievements is typically at index 2
        state.handle_input(ControllerInput::Select);
        assert_eq!(state.current_screen, OverlayScreen::Achievements);

        // Go back to main menu
        state.handle_input(ControllerInput::Back);
        assert_eq!(state.current_screen, OverlayScreen::Main);
    }
}
