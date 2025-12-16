use crate::ipc::{AchievementInfo, OverlayMessage, OverlayScreen, ToastStyle};
use crate::input::ControllerInput;
use crate::controllers::{ControllerState, CONTROLLER_MENU_OPTIONS, MAX_PLAYERS};
use crate::performance::PerformanceStats;
use macroquad::prelude::*;
use std::time::{Duration, Instant};
use std::collections::VecDeque;

/// Tracks achievements for the current game
#[derive(Default, Clone)]
pub struct AchievementTracker {
    pub game_title: String,
    pub game_hash: String,
    pub achievements: Vec<AchievementInfo>,
    pub scroll_offset: usize,
    pub selected_index: usize,
}

impl AchievementTracker {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set_achievements(&mut self, game_title: String, game_hash: String, achievements: Vec<AchievementInfo>) {
        self.game_title = game_title;
        self.game_hash = game_hash;
        self.achievements = achievements;
        self.scroll_offset = 0;
        self.selected_index = 0;
    }

    pub fn mark_earned(&mut self, achievement_id: u32, is_hardcore: bool) {
        if let Some(ach) = self.achievements.iter_mut().find(|a| a.id == achievement_id) {
            ach.earned = true;
            if is_hardcore {
                ach.earned_hardcore = true;
            }
        }
    }

    pub fn earned_count(&self) -> usize {
        self.achievements.iter().filter(|a| a.earned).count()
    }

    pub fn total_count(&self) -> usize {
        self.achievements.len()
    }

    pub fn total_points(&self) -> u32 {
        self.achievements.iter().map(|a| a.points).sum()
    }

    pub fn earned_points(&self) -> u32 {
        self.achievements.iter().filter(|a| a.earned).map(|a| a.points).sum()
    }

    pub fn clear(&mut self) {
        self.game_title.clear();
        self.game_hash.clear();
        self.achievements.clear();
        self.scroll_offset = 0;
        self.selected_index = 0;
    }
}

pub struct OverlayState {
    pub visible: bool,
    pub current_screen: OverlayScreen,
    pub selected_option: usize,
    pub toasts: ToastManager,
    pub font_color: Color,
    pub cursor_color: Color,
    pub achievements: AchievementTracker,
    pub controllers: ControllerState,
    pub performance: PerformanceStats,
}

impl OverlayState {
    pub async fn new() -> Self {
        Self {
            visible: false,
            current_screen: OverlayScreen::Main,
            selected_option: 0,
            toasts: ToastManager::new(),
            font_color: WHITE,  // Default font color
            cursor_color: YELLOW,  // Default cursor/selection color
            achievements: AchievementTracker::new(),
            controllers: ControllerState::new(),
            performance: PerformanceStats::new(),
        }
    }

    pub fn toggle_visibility(&mut self) {
        self.visible = !self.visible;
        if self.visible {
            // Reset to main screen when opening
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

    pub fn handle_message(&mut self, message: OverlayMessage) {
        match message {
            OverlayMessage::ShowToast { message, icon, duration_ms, style } => {
                self.toasts.add_toast(message, icon, style, duration_ms);
            }
            OverlayMessage::ShowOverlay { screen } => {
                self.visible = true;
                self.current_screen = screen;
            }
            OverlayMessage::HideOverlay => {
                self.visible = false;
            }
            OverlayMessage::UnlockAchievement { cart_id, achievement_id, .. } => {
                // TODO: Implement achievement tracking
                println!("[State] Achievement unlocked: {} in {}", achievement_id, cart_id);
                self.toasts.add_toast(
                    format!("Achievement Unlocked: {}", achievement_id),
                    None,
                    ToastStyle::Success,
                    5000,
                );
            }
            OverlayMessage::GetStatus => {
                // TODO: Send status response
                println!("[State] Status query received");
            }
            OverlayMessage::SetTheme { font_color, cursor_color } => {
                use crate::utils::string_to_color;
                self.font_color = string_to_color(&font_color);
                self.cursor_color = string_to_color(&cursor_color);
                println!("[State] Theme updated: font={}, cursor={}", font_color, cursor_color);
            }
            // RetroAchievements messages
            OverlayMessage::RaGameStart { game_title, total_achievements, earned_achievements, .. } => {
                println!("[State] RA Game started: {} ({}/{} achievements)", game_title, earned_achievements, total_achievements);
                if total_achievements > 0 {
                    self.toasts.add_toast(
                        format!("🎮 {} - {}/{} achievements", game_title, earned_achievements, total_achievements),
                        None,
                        ToastStyle::Info,
                        4000,
                    );
                }
            }
            OverlayMessage::RaAchievementUnlocked { title, points, is_hardcore, .. } => {
                let hc_badge = if is_hardcore { " ⭐" } else { "" };
                println!("[State] RA Achievement unlocked: {} ({} pts){}", title, points, hc_badge);
                self.toasts.add_toast(
                    format!("🏆 {} ({} pts){}", title, points, hc_badge),
                    None,
                    ToastStyle::Success,
                    5000,
                );
            }
            OverlayMessage::RaProgressUpdate { earned, total } => {
                println!("[State] RA Progress: {}/{}", earned, total);
                // Could update a progress indicator in the UI
            }
            OverlayMessage::RaAchievementList { game_title, game_hash, achievements } => {
                let count = achievements.len();
                let earned = achievements.iter().filter(|a| a.earned).count();
                println!("[State] RA Achievement list received: {} - {} achievements ({} earned)", game_title, count, earned);
                self.achievements.set_achievements(game_title, game_hash, achievements);
            }
            OverlayMessage::ToggleOverlay => {
                println!("[State] Toggle overlay received from input daemon");
                self.toggle_visibility();
            }
            OverlayMessage::GameStarted { cart_id, game_name, runtime } => {
                println!("[State] Game started: {} ({}) using {}", game_name, cart_id, runtime);
                // Could show a toast or update internal state
                self.toasts.add_toast(
                    format!("▶ {}", game_name),
                    None,
                    ToastStyle::Info,
                    2000,
                );
            }
            OverlayMessage::GameStopped { cart_id } => {
                println!("[State] Game stopped: {}", cart_id);
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

    pub fn update(&mut self) {
        self.toasts.update();
        self.controllers.update_messages();
    }

    pub fn handle_input(&mut self, input: ControllerInput) {
        // Only process inputs when overlay is visible
        if !self.visible {
            return;
        }

        match self.current_screen {
            OverlayScreen::Main => self.handle_main_menu_input(input),
            OverlayScreen::Settings => self.handle_settings_input(input),
            OverlayScreen::Achievements => self.handle_achievements_input(input),
            OverlayScreen::Controllers => self.handle_controllers_menu_input(input),
            OverlayScreen::BluetoothPairing => self.handle_bluetooth_input(input),
            OverlayScreen::ControllerAssign => self.handle_assign_input(input),
            OverlayScreen::GamepadTester => self.handle_tester_input(input),
            OverlayScreen::QuitConfirm => self.handle_quit_confirm_input(input),
        }
    }

    fn handle_main_menu_input(&mut self, input: ControllerInput) {
        const MENU_OPTION_COUNT: usize = 6; // Controllers, Settings, Achievements, Quick Save, Resume, Quit

        match input {
            ControllerInput::Up => {
                if self.selected_option > 0 {
                    self.selected_option -= 1;
                    println!("[State] Menu selection: {}", self.selected_option);
                }
            }
            ControllerInput::Down => {
                if self.selected_option < MENU_OPTION_COUNT - 1 {
                    self.selected_option += 1;
                    println!("[State] Menu selection: {}", self.selected_option);
                }
            }
            ControllerInput::Select => {
                println!("[State] Menu option selected: {}", self.selected_option);
                match self.selected_option {
                    0 => {
                        // Controllers
                        self.current_screen = OverlayScreen::Controllers;
                        self.controllers.selected_menu_item = 0;
                        println!("[State] Navigating to Controllers");
                    }
                    1 => {
                        // Settings
                        self.current_screen = OverlayScreen::Settings;
                        println!("[State] Navigating to Settings");
                    }
                    2 => {
                        // Achievements
                        self.current_screen = OverlayScreen::Achievements;
                        println!("[State] Navigating to Achievements");
                    }
                    3 => {
                        // Quick Save
                        println!("[State] Quick Save triggered");
                        self.toasts.add_toast(
                            "Game saved".to_string(),
                            None,
                            ToastStyle::Success,
                            2000,
                        );
                        // TODO: Implement actual save functionality
                    }
                    4 => {
                        // Resume Game
                        println!("[State] Resuming game");
                        self.visible = false;
                    }
                    5 => {
                        // Quit to BIOS - show confirmation
                        self.current_screen = OverlayScreen::QuitConfirm;
                        println!("[State] Showing quit confirmation");
                    }
                    _ => {}
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

    fn handle_settings_input(&mut self, input: ControllerInput) {
        match input {
            ControllerInput::Back => {
                // Return to main menu
                self.current_screen = OverlayScreen::Main;
                self.selected_option = 0;
                println!("[State] Returning to main menu");
            }
            _ => {}
        }
    }

    fn handle_achievements_input(&mut self, input: ControllerInput) {
        let max_visible = 6; // Number of achievements visible at once
        let total = self.achievements.total_count();

        match input {
            ControllerInput::Up => {
                if self.achievements.selected_index > 0 {
                    self.achievements.selected_index -= 1;
                    // Adjust scroll if needed
                    if self.achievements.selected_index < self.achievements.scroll_offset {
                        self.achievements.scroll_offset = self.achievements.selected_index;
                    }
                    println!("[State] Achievement selection: {}", self.achievements.selected_index);
                }
            }
            ControllerInput::Down => {
                if self.achievements.selected_index < total.saturating_sub(1) {
                    self.achievements.selected_index += 1;
                    // Adjust scroll if needed
                    if self.achievements.selected_index >= self.achievements.scroll_offset + max_visible {
                        self.achievements.scroll_offset = self.achievements.selected_index - max_visible + 1;
                    }
                    println!("[State] Achievement selection: {}", self.achievements.selected_index);
                }
            }
            ControllerInput::Back => {
                // Return to main menu
                self.current_screen = OverlayScreen::Main;
                self.selected_option = 0;
                self.achievements.selected_index = 0;
                self.achievements.scroll_offset = 0;
                println!("[State] Returning to main menu");
            }
            _ => {}
        }
    }

    fn handle_controllers_menu_input(&mut self, input: ControllerInput) {
        let menu_len = CONTROLLER_MENU_OPTIONS.len();

        match input {
            ControllerInput::Up => {
                if self.controllers.selected_menu_item > 0 {
                    self.controllers.selected_menu_item -= 1;
                }
            }
            ControllerInput::Down => {
                if self.controllers.selected_menu_item < menu_len - 1 {
                    self.controllers.selected_menu_item += 1;
                }
            }
            ControllerInput::Select => {
                match self.controllers.selected_menu_item {
                    0 => {
                        // Bluetooth Controllers
                        self.current_screen = OverlayScreen::BluetoothPairing;
                        self.controllers.bt_selected_index = 0;
                        println!("[State] Navigating to Bluetooth Pairing");
                    }
                    1 => {
                        // Assign Controllers
                        self.current_screen = OverlayScreen::ControllerAssign;
                        self.controllers.assign_selected_player = 0;
                        println!("[State] Navigating to Controller Assignment");
                    }
                    2 => {
                        // Gamepad Tester
                        self.current_screen = OverlayScreen::GamepadTester;
                        self.controllers.reset_tester_state();
                        println!("[State] Navigating to Gamepad Tester");
                    }
                    3 => {
                        // Auto-assign All
                        self.controllers.auto_assign_all();
                        let count = self.controllers.controllers.len().min(MAX_PLAYERS);
                        self.controllers.show_success(format!("Auto-assigned {} controller(s)", count));
                        self.toasts.add_toast(
                            format!("Auto-assigned {} controller(s)", count),
                            None,
                            ToastStyle::Success,
                            2000,
                        );
                        println!("[State] Auto-assigned controllers");
                    }
                    4 => {
                        // Back
                        self.current_screen = OverlayScreen::Main;
                        self.selected_option = 0;
                    }
                    _ => {}
                }
            }
            ControllerInput::Back => {
                self.current_screen = OverlayScreen::Main;
                self.selected_option = 0;
                println!("[State] Returning to main menu");
            }
            _ => {}
        }
    }

    fn handle_bluetooth_input(&mut self, input: ControllerInput) {
        use crate::controllers::BluetoothScanState;
        
        let device_count = self.controllers.bluetooth_devices.len();

        match input {
            ControllerInput::Up => {
                if self.controllers.bt_selected_index > 0 {
                    self.controllers.bt_selected_index -= 1;
                }
            }
            ControllerInput::Down => {
                if device_count > 0 && self.controllers.bt_selected_index < device_count - 1 {
                    self.controllers.bt_selected_index += 1;
                }
            }
            ControllerInput::Select => {
                // Start pairing with selected device
                if let Some(device) = self.controllers.bluetooth_devices.get(self.controllers.bt_selected_index) {
                    if !device.is_paired {
                        self.controllers.bluetooth_state = BluetoothScanState::Pairing(device.mac_address.clone());
                        self.toasts.add_toast(
                            format!("Pairing with {}...", device.name),
                            None,
                            ToastStyle::Info,
                            3000,
                        );
                        // TODO: Trigger actual pairing via IPC to BIOS/system
                        println!("[State] Starting pairing with: {}", device.name);
                    } else if !device.is_connected {
                        self.controllers.bluetooth_state = BluetoothScanState::Connecting(device.mac_address.clone());
                        self.toasts.add_toast(
                            format!("Connecting to {}...", device.name),
                            None,
                            ToastStyle::Info,
                            3000,
                        );
                        println!("[State] Connecting to: {}", device.name);
                    }
                }
            }
            ControllerInput::Secondary => {
                // Start/stop scanning
                match &self.controllers.bluetooth_state {
                    BluetoothScanState::Idle => {
                        self.controllers.bluetooth_state = BluetoothScanState::Scanning;
                        self.toasts.add_toast(
                            "Scanning for Bluetooth devices...".to_string(),
                            None,
                            ToastStyle::Info,
                            2000,
                        );
                        // TODO: Trigger actual scan via IPC
                        println!("[State] Started Bluetooth scan");
                    }
                    BluetoothScanState::Scanning => {
                        self.controllers.bluetooth_state = BluetoothScanState::Idle;
                        println!("[State] Stopped Bluetooth scan");
                    }
                    _ => {}
                }
            }
            ControllerInput::Back => {
                self.current_screen = OverlayScreen::Controllers;
                self.controllers.selected_menu_item = 0;
                self.controllers.bluetooth_state = BluetoothScanState::Idle;
                println!("[State] Returning to Controllers menu");
            }
            _ => {}
        }
    }

    fn handle_assign_input(&mut self, input: ControllerInput) {
        let controller_count = self.controllers.controllers.len();

        match input {
            ControllerInput::Up => {
                if self.controllers.assign_selected_player > 0 {
                    self.controllers.assign_selected_player -= 1;
                }
            }
            ControllerInput::Down => {
                if self.controllers.assign_selected_player < MAX_PLAYERS - 1 {
                    self.controllers.assign_selected_player += 1;
                }
            }
            ControllerInput::Left | ControllerInput::Right => {
                // Cycle through available controllers for this player
                if controller_count == 0 {
                    return;
                }
                
                let player = self.controllers.assign_selected_player + 1;
                let current_controller = self.controllers.player_assignments[self.controllers.assign_selected_player];
                
                // Find next available controller
                let available: Vec<usize> = self.controllers.controllers.iter()
                    .filter(|c| c.assigned_player.is_none() || c.assigned_player == Some(player))
                    .map(|c| c.id)
                    .collect();
                
                if available.is_empty() {
                    return;
                }
                
                let current_idx = current_controller
                    .and_then(|id| available.iter().position(|&aid| aid == id))
                    .unwrap_or(0);
                
                let next_idx = match input {
                    ControllerInput::Right => (current_idx + 1) % (available.len() + 1),
                    ControllerInput::Left => {
                        if current_idx == 0 && current_controller.is_some() {
                            available.len() // Unassign
                        } else if current_idx == 0 {
                            available.len().saturating_sub(1)
                        } else {
                            current_idx - 1
                        }
                    }
                    _ => current_idx,
                };
                
                if next_idx >= available.len() {
                    // Unassign
                    if let Some(cid) = current_controller {
                        self.controllers.unassign_controller(cid);
                        println!("[State] Unassigned controller from player {}", player);
                    }
                } else {
                    let new_controller_id = available[next_idx];
                    if let Err(e) = self.controllers.assign_controller_to_player(new_controller_id, player) {
                        self.controllers.show_error(e);
                    } else {
                        println!("[State] Assigned controller {} to player {}", new_controller_id, player);
                    }
                }
            }
            ControllerInput::Select => {
                // Quick assign - assign next unassigned controller to selected player
                let player = self.controllers.assign_selected_player + 1;
                let unassigned = self.controllers.controllers.iter()
                    .find(|c| c.assigned_player.is_none())
                    .map(|c| c.id);
                
                if let Some(controller_id) = unassigned {
                    if let Err(e) = self.controllers.assign_controller_to_player(controller_id, player) {
                        self.controllers.show_error(e);
                    } else {
                        self.toasts.add_toast(
                            format!("Assigned controller to Player {}", player),
                            None,
                            ToastStyle::Success,
                            2000,
                        );
                    }
                } else {
                    self.toasts.add_toast(
                        "No unassigned controllers available".to_string(),
                        None,
                        ToastStyle::Warning,
                        2000,
                    );
                }
            }
            ControllerInput::Back => {
                self.current_screen = OverlayScreen::Controllers;
                self.controllers.selected_menu_item = 1;
                println!("[State] Returning to Controllers menu");
            }
            _ => {}
        }
    }

    fn handle_tester_input(&mut self, input: ControllerInput) {
        let controller_count = self.controllers.controllers.len();

        match input {
            ControllerInput::Left => {
                // Previous controller
                if self.controllers.tester_selected_controller > 0 {
                    self.controllers.tester_selected_controller -= 1;
                    self.controllers.reset_tester_state();
                }
            }
            ControllerInput::Right => {
                // Next controller
                if controller_count > 0 && self.controllers.tester_selected_controller < controller_count - 1 {
                    self.controllers.tester_selected_controller += 1;
                    self.controllers.reset_tester_state();
                }
            }
            ControllerInput::Back => {
                self.current_screen = OverlayScreen::Controllers;
                self.controllers.selected_menu_item = 2;
                self.controllers.reset_tester_state();
                println!("[State] Returning to Controllers menu");
            }
            _ => {
                // All other inputs are captured by the tester display
            }
        }
    }

    fn handle_quit_confirm_input(&mut self, input: ControllerInput) {
        match input {
            ControllerInput::Select => {
                // Confirmed - trigger quit
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
            }
            ControllerInput::Back => {
                // Cancelled - return to main menu
                self.current_screen = OverlayScreen::Main;
                self.selected_option = 5; // Keep quit selected
                println!("[State] Quit cancelled");
            }
            _ => {}
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
