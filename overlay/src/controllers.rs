//! Controller management for the overlay
//!
//! Handles:
//! - Bluetooth device discovery and pairing
//! - Controller-to-player assignment
//! - Gamepad testing/visualization

use std::collections::HashMap;
use std::time::{Duration, Instant};

/// Maximum number of players supported
pub const MAX_PLAYERS: usize = 4;

/// Represents a connected controller
#[derive(Debug, Clone)]
pub struct ConnectedController {
    pub id: usize,
    pub name: String,
    pub uuid: String,
    pub is_wireless: bool,
    pub battery_level: Option<u8>, // 0-100%
    pub assigned_player: Option<usize>, // 1-4, or None if unassigned
}

/// Represents a discovered Bluetooth device
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct BluetoothDevice {
    pub mac_address: String,
    pub name: String,
    pub is_paired: bool,
    pub is_connected: bool,
    pub signal_strength: Option<i16>, // dBm
}

/// Current state of Bluetooth scanning
#[derive(Debug, Clone, PartialEq)]
pub enum BluetoothScanState {
    Idle,
    Scanning,
    Pairing(String), // MAC address being paired
    Connecting(String),
    Error(String),
}

/// Gamepad button state for tester
#[derive(Debug, Clone, Default)]
pub struct GamepadButtonState {
    // Face buttons
    pub a: bool,
    pub b: bool,
    pub x: bool,
    pub y: bool,
    // D-Pad
    pub dpad_up: bool,
    pub dpad_down: bool,
    pub dpad_left: bool,
    pub dpad_right: bool,
    // Shoulder buttons
    pub lb: bool,
    pub rb: bool,
    pub lt: f32, // 0.0 - 1.0
    pub rt: f32, // 0.0 - 1.0
    // Sticks (pressed)
    pub ls_press: bool,
    pub rs_press: bool,
    // Stick axes
    pub left_stick_x: f32,  // -1.0 to 1.0
    pub left_stick_y: f32,  // -1.0 to 1.0
    pub right_stick_x: f32, // -1.0 to 1.0
    pub right_stick_y: f32, // -1.0 to 1.0
    // Special buttons
    pub start: bool,
    pub select: bool,
    pub guide: bool,
}

/// State for the controller management screens
#[derive(Debug)]
pub struct ControllerState {
    // Connected controllers
    pub controllers: Vec<ConnectedController>,
    
    // Player assignments (index = player number - 1, value = controller id)
    pub player_assignments: [Option<usize>; MAX_PLAYERS],
    
    // Bluetooth state
    pub bluetooth_devices: Vec<BluetoothDevice>,
    pub bluetooth_state: BluetoothScanState,
    pub bt_selected_index: usize,
    pub bt_scroll_offset: usize,
    
    // Controller assignment state
    pub assign_selected_player: usize, // 0-3 for player 1-4
    pub assign_selected_controller: usize,
    
    // Gamepad tester state
    pub tester_selected_controller: usize,
    pub tester_button_state: GamepadButtonState,
    pub tester_last_input_time: Instant,
    
    // UI state
    pub selected_menu_item: usize,
    pub error_message: Option<String>,
    pub success_message: Option<(String, Instant)>,
}

impl ControllerState {
    pub fn new() -> Self {
        Self {
            controllers: Vec::new(),
            player_assignments: [None; MAX_PLAYERS],
            bluetooth_devices: Vec::new(),
            bluetooth_state: BluetoothScanState::Idle,
            bt_selected_index: 0,
            bt_scroll_offset: 0,
            assign_selected_player: 0,
            assign_selected_controller: 0,
            tester_selected_controller: 0,
            tester_button_state: GamepadButtonState::default(),
            tester_last_input_time: Instant::now(),
            selected_menu_item: 0,
            error_message: None,
            success_message: None,
        }
    }

    /// Update the list of connected controllers from gilrs
    #[cfg(feature = "daemon")]
    pub fn update_from_gilrs(&mut self, gilrs: &gilrs::Gilrs) {
        let mut new_controllers = Vec::new();
        
        for (id, gamepad) in gilrs.gamepads() {
            let name = gamepad.name().to_string();
            let uuid = format!("{:?}", gamepad.uuid());
            
            // Check if already in our list
            let existing = self.controllers.iter().find(|c| c.uuid == uuid);
            let assigned_player = existing.and_then(|c| c.assigned_player);
            
            new_controllers.push(ConnectedController {
                id: id.into(),
                name,
                uuid,
                is_wireless: false, // gilrs doesn't provide this info directly
                battery_level: None, // Would need platform-specific code
                assigned_player,
            });
        }
        
        self.controllers = new_controllers;
        
        // Clean up player assignments for disconnected controllers
        for assignment in &mut self.player_assignments {
            if let Some(controller_id) = *assignment {
                if !self.controllers.iter().any(|c| c.id == controller_id) {
                    *assignment = None;
                }
            }
        }
    }

    /// Assign a controller to a player
    pub fn assign_controller_to_player(&mut self, controller_id: usize, player: usize) -> Result<(), String> {
        if player == 0 || player > MAX_PLAYERS {
            return Err(format!("Invalid player number: {}", player));
        }
        
        // Check if controller exists
        let controller_idx = self.controllers.iter()
            .position(|c| c.id == controller_id)
            .ok_or_else(|| format!("Controller {} not found", controller_id))?;
        
        // Get old player assignment for this controller (to clear player_assignments)
        let old_player = self.controllers[controller_idx].assigned_player;
        
        // Clear old player assignment
        if let Some(old_p) = old_player {
            self.player_assignments[old_p - 1] = None;
        }
        
        // Clear any existing controller from this player slot
        if let Some(old_controller_id) = self.player_assignments[player - 1] {
            // Find and clear the old controller's assignment
            for controller in &mut self.controllers {
                if controller.id == old_controller_id {
                    controller.assigned_player = None;
                    break;
                }
            }
        }
        
        // Make the new assignment
        self.controllers[controller_idx].assigned_player = Some(player);
        self.player_assignments[player - 1] = Some(controller_id);
        
        Ok(())
    }

    /// Unassign a controller from its player
    pub fn unassign_controller(&mut self, controller_id: usize) {
        if let Some(controller) = self.controllers.iter_mut().find(|c| c.id == controller_id) {
            if let Some(player) = controller.assigned_player {
                self.player_assignments[player - 1] = None;
            }
            controller.assigned_player = None;
        }
    }

    /// Get the controller assigned to a specific player
    pub fn get_player_controller(&self, player: usize) -> Option<&ConnectedController> {
        if player == 0 || player > MAX_PLAYERS {
            return None;
        }
        
        self.player_assignments[player - 1]
            .and_then(|id| self.controllers.iter().find(|c| c.id == id))
    }

    /// Auto-assign controllers to players in order of connection
    pub fn auto_assign_all(&mut self) {
        // Clear existing assignments
        for assignment in &mut self.player_assignments {
            *assignment = None;
        }
        for controller in &mut self.controllers {
            controller.assigned_player = None;
        }
        
        // Assign in order
        for (i, controller) in self.controllers.iter_mut().enumerate() {
            if i < MAX_PLAYERS {
                controller.assigned_player = Some(i + 1);
                self.player_assignments[i] = Some(controller.id);
            }
        }
    }

    /// Update gamepad tester state from gilrs events
    #[cfg(feature = "daemon")]
    pub fn update_tester_from_gilrs(&mut self, gilrs: &mut gilrs::Gilrs) {
        use gilrs::{Button, Axis, EventType};
        
        // Get the selected controller's gilrs ID
        let selected_id = self.controllers
            .get(self.tester_selected_controller)
            .map(|c| c.id);
        
        // Process events
        while let Some(event) = gilrs.next_event() {
            // Only process events from the selected controller
            let event_id: usize = event.id.into();
            if Some(event_id) != selected_id {
                continue;
            }
            
            self.tester_last_input_time = Instant::now();
            
            match event.event {
                // Face buttons
                EventType::ButtonPressed(Button::South, _) => self.tester_button_state.a = true,
                EventType::ButtonReleased(Button::South, _) => self.tester_button_state.a = false,
                EventType::ButtonPressed(Button::East, _) => self.tester_button_state.b = true,
                EventType::ButtonReleased(Button::East, _) => self.tester_button_state.b = false,
                EventType::ButtonPressed(Button::West, _) => self.tester_button_state.x = true,
                EventType::ButtonReleased(Button::West, _) => self.tester_button_state.x = false,
                EventType::ButtonPressed(Button::North, _) => self.tester_button_state.y = true,
                EventType::ButtonReleased(Button::North, _) => self.tester_button_state.y = false,
                
                // D-Pad
                EventType::ButtonPressed(Button::DPadUp, _) => self.tester_button_state.dpad_up = true,
                EventType::ButtonReleased(Button::DPadUp, _) => self.tester_button_state.dpad_up = false,
                EventType::ButtonPressed(Button::DPadDown, _) => self.tester_button_state.dpad_down = true,
                EventType::ButtonReleased(Button::DPadDown, _) => self.tester_button_state.dpad_down = false,
                EventType::ButtonPressed(Button::DPadLeft, _) => self.tester_button_state.dpad_left = true,
                EventType::ButtonReleased(Button::DPadLeft, _) => self.tester_button_state.dpad_left = false,
                EventType::ButtonPressed(Button::DPadRight, _) => self.tester_button_state.dpad_right = true,
                EventType::ButtonReleased(Button::DPadRight, _) => self.tester_button_state.dpad_right = false,
                
                // Shoulder buttons
                EventType::ButtonPressed(Button::LeftTrigger, _) => self.tester_button_state.lb = true,
                EventType::ButtonReleased(Button::LeftTrigger, _) => self.tester_button_state.lb = false,
                EventType::ButtonPressed(Button::RightTrigger, _) => self.tester_button_state.rb = true,
                EventType::ButtonReleased(Button::RightTrigger, _) => self.tester_button_state.rb = false,
                
                // Trigger axes
                EventType::AxisChanged(Axis::LeftZ, value, _) => {
                    self.tester_button_state.lt = (value + 1.0) / 2.0; // Convert -1..1 to 0..1
                }
                EventType::AxisChanged(Axis::RightZ, value, _) => {
                    self.tester_button_state.rt = (value + 1.0) / 2.0;
                }
                
                // Stick buttons
                EventType::ButtonPressed(Button::LeftThumb, _) => self.tester_button_state.ls_press = true,
                EventType::ButtonReleased(Button::LeftThumb, _) => self.tester_button_state.ls_press = false,
                EventType::ButtonPressed(Button::RightThumb, _) => self.tester_button_state.rs_press = true,
                EventType::ButtonReleased(Button::RightThumb, _) => self.tester_button_state.rs_press = false,
                
                // Stick axes
                EventType::AxisChanged(Axis::LeftStickX, value, _) => {
                    self.tester_button_state.left_stick_x = value;
                }
                EventType::AxisChanged(Axis::LeftStickY, value, _) => {
                    self.tester_button_state.left_stick_y = value;
                }
                EventType::AxisChanged(Axis::RightStickX, value, _) => {
                    self.tester_button_state.right_stick_x = value;
                }
                EventType::AxisChanged(Axis::RightStickY, value, _) => {
                    self.tester_button_state.right_stick_y = value;
                }
                
                // Special buttons
                EventType::ButtonPressed(Button::Start, _) => self.tester_button_state.start = true,
                EventType::ButtonReleased(Button::Start, _) => self.tester_button_state.start = false,
                EventType::ButtonPressed(Button::Select, _) => self.tester_button_state.select = true,
                EventType::ButtonReleased(Button::Select, _) => self.tester_button_state.select = false,
                EventType::ButtonPressed(Button::Mode, _) => self.tester_button_state.guide = true,
                EventType::ButtonReleased(Button::Mode, _) => self.tester_button_state.guide = false,
                
                _ => {}
            }
        }
    }

    /// Clear the tester button state
    pub fn reset_tester_state(&mut self) {
        self.tester_button_state = GamepadButtonState::default();
    }

    /// Show success message temporarily
    pub fn show_success(&mut self, message: String) {
        self.success_message = Some((message, Instant::now()));
        self.error_message = None;
    }

    /// Show error message
    pub fn show_error(&mut self, message: String) {
        self.error_message = Some(message);
        self.success_message = None;
    }

    /// Clear messages
    pub fn clear_messages(&mut self) {
        self.error_message = None;
        self.success_message = None;
    }

    /// Update messages (clear expired success messages)
    pub fn update_messages(&mut self) {
        if let Some((_, created)) = &self.success_message {
            if created.elapsed() > Duration::from_secs(3) {
                self.success_message = None;
            }
        }
    }
}

impl Default for ControllerState {
    fn default() -> Self {
        Self::new()
    }
}

// Menu options for the controller main screen
pub const CONTROLLER_MENU_OPTIONS: &[&str] = &[
    "BLUETOOTH CONTROLLERS",
    "ASSIGN CONTROLLERS",
    "GAMEPAD TESTER",
    "HOTKEY SETTINGS",
    "AUTO-ASSIGN ALL",
    "BACK",
];

