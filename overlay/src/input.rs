use anyhow::Result;
use macroquad::prelude::*;
use gilrs::{Gilrs, Button, Axis};
use crate::hotkeys::{HotkeyManager, HotkeyAction, InputComponent, GamepadButtonType, ModifierKey};
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ControllerInput {
    Up,
    Down,
    Left,
    Right,
    Select,     // A button
    Back,       // B button
    Secondary,  // X button
    Guide,      // Guide/Home button
    LB,         // Left bumper
    RB,         // Right bumper
    LT,         // Left trigger
    RT,         // Right trigger
}

pub struct HotkeyMonitor {
    gilrs: Gilrs,
    analog_was_neutral: bool,
    hotkey_manager: HotkeyManager,
}

impl HotkeyMonitor {
    pub fn new() -> Result<Self> {
        println!("[Input] Initializing input monitor (gilrs + macroquad)...");

        let gilrs = Gilrs::new()
            .map_err(|e| anyhow::anyhow!("Failed to initialize gilrs: {}", e))?;

        let hotkey_manager = HotkeyManager::new()?;

        println!("[Input] Input monitor initialized");
        println!("[Input] Hotkey manager loaded");

        Ok(Self {
            gilrs,
            analog_was_neutral: true,
            hotkey_manager,
        })
    }

    /// Check if the overlay toggle hotkey was pressed
    pub fn check_hotkey_pressed(&mut self) -> bool {
        let current_inputs = self.get_current_inputs();
        self.hotkey_manager.check_action_pressed(HotkeyAction::ToggleOverlay, &current_inputs)
    }

    /// Check if the performance HUD toggle hotkey was pressed
    pub fn check_performance_hotkey_pressed(&mut self) -> bool {
        let current_inputs = self.get_current_inputs();
        self.hotkey_manager.check_action_pressed(HotkeyAction::TogglePerformance, &current_inputs)
    }

    /// Get current input states for all supported inputs
    fn get_current_inputs(&mut self) -> HashMap<InputComponent, bool> {
        let mut inputs = HashMap::new();

        // Process gamepad events (must call next_event to drain queue)
        while let Some(ev) = self.gilrs.next_event() {
            match ev.event {
                gilrs::EventType::ButtonPressed(Button::Mode, _) => {
                    inputs.insert(InputComponent::GamepadButton(GamepadButtonType::Mode), true);
                }
                _ => {}
            }
        }

        // Check keyboard keys
        inputs.insert(InputComponent::Key("F12".to_string()), is_key_down(KeyCode::F12));
        inputs.insert(InputComponent::Key("F3".to_string()), is_key_down(KeyCode::F3));
        inputs.insert(InputComponent::Key("F5".to_string()), is_key_down(KeyCode::F5));
        inputs.insert(InputComponent::Key("F9".to_string()), is_key_down(KeyCode::F9));
        inputs.insert(InputComponent::Key("O".to_string()), is_key_down(KeyCode::O));

        // Check modifiers
        inputs.insert(
            InputComponent::Modifier(ModifierKey::Ctrl),
            is_key_down(KeyCode::LeftControl) || is_key_down(KeyCode::RightControl),
        );
        inputs.insert(
            InputComponent::Modifier(ModifierKey::Alt),
            is_key_down(KeyCode::LeftAlt) || is_key_down(KeyCode::RightAlt),
        );
        inputs.insert(
            InputComponent::Modifier(ModifierKey::Shift),
            is_key_down(KeyCode::LeftShift) || is_key_down(KeyCode::RightShift),
        );

        inputs
    }

    /// Get reference to hotkey manager (for settings screen)
    pub fn hotkey_manager(&self) -> &HotkeyManager {
        &self.hotkey_manager
    }

    /// Get mutable reference to hotkey manager (for settings screen)
    pub fn hotkey_manager_mut(&mut self) -> &mut HotkeyManager {
        &mut self.hotkey_manager
    }

    pub fn poll_inputs(&mut self) -> Vec<ControllerInput> {
        let mut inputs = Vec::new();
        
        // Process keyboard input (same as BIOS)
        if is_key_pressed(KeyCode::Up) {
            inputs.push(ControllerInput::Up);
        }
        if is_key_pressed(KeyCode::Down) {
            inputs.push(ControllerInput::Down);
        }
        if is_key_pressed(KeyCode::Left) {
            inputs.push(ControllerInput::Left);
        }
        if is_key_pressed(KeyCode::Right) {
            inputs.push(ControllerInput::Right);
        }
        if is_key_pressed(KeyCode::Enter) {
            inputs.push(ControllerInput::Select);
        }
        if is_key_pressed(KeyCode::Backspace) {
            inputs.push(ControllerInput::Back);
        }
        if is_key_pressed(KeyCode::F12) {
            inputs.push(ControllerInput::Guide);
        }
        if is_key_pressed(KeyCode::Q) {
            inputs.push(ControllerInput::LB);
        }
        if is_key_pressed(KeyCode::E) {
            inputs.push(ControllerInput::RB);
        }
        
        // Process gamepad input (same as BIOS)
        let was_neutral = self.analog_was_neutral;
        
        // Handle button events
        while let Some(ev) = self.gilrs.next_event() {
            match ev.event {
                gilrs::EventType::ButtonPressed(Button::DPadUp, _) => {
                    inputs.push(ControllerInput::Up);
                }
                gilrs::EventType::ButtonPressed(Button::DPadDown, _) => {
                    inputs.push(ControllerInput::Down);
                }
                gilrs::EventType::ButtonPressed(Button::DPadLeft, _) => {
                    inputs.push(ControllerInput::Left);
                }
                gilrs::EventType::ButtonPressed(Button::DPadRight, _) => {
                    inputs.push(ControllerInput::Right);
                }
                gilrs::EventType::ButtonPressed(Button::South, _) => {
                    inputs.push(ControllerInput::Select);  // A button
                }
                gilrs::EventType::ButtonPressed(Button::East, _) => {
                    inputs.push(ControllerInput::Back);  // B button
                }
                gilrs::EventType::ButtonPressed(Button::West, _) => {
                    inputs.push(ControllerInput::Secondary);  // X button
                }
                gilrs::EventType::ButtonPressed(Button::Mode, _) => {
                    inputs.push(ControllerInput::Guide);  // Guide button
                }
                gilrs::EventType::ButtonPressed(Button::LeftTrigger, _) => {
                    inputs.push(ControllerInput::LB);  // Left bumper
                }
                gilrs::EventType::ButtonPressed(Button::RightTrigger, _) => {
                    inputs.push(ControllerInput::RB);  // Right bumper
                }
                gilrs::EventType::ButtonPressed(Button::LeftTrigger2, _) => {
                    inputs.push(ControllerInput::LT);  // Left trigger
                }
                gilrs::EventType::ButtonPressed(Button::RightTrigger2, _) => {
                    inputs.push(ControllerInput::RT);  // Right trigger
                }
                _ => {}
            }
        }
        
        // Handle analog stick input (same logic as BIOS)
        let mut any_stick_active = false;
        const ANALOG_DEADZONE: f32 = 0.5;
        
        for (_, gamepad) in self.gilrs.gamepads() {
            let raw_x = gamepad.value(Axis::LeftStickX);
            let raw_y = gamepad.value(Axis::LeftStickY);
            
            let is_currently_neutral = raw_x.abs() < ANALOG_DEADZONE &&
                raw_y.abs() < ANALOG_DEADZONE;
            
            if !is_currently_neutral {
                any_stick_active = true;
                
                // Was the system neutral before this frame?
                if was_neutral {
                    // Yes. This is a "just pushed" event. Fire it.
                    // Prioritize dominant axis
                    if raw_y.abs() > raw_x.abs() {
                        // Vertical is stronger
                        if raw_y > -ANALOG_DEADZONE {       // -Y is UP
                            inputs.push(ControllerInput::Up);
                        } else if raw_y < ANALOG_DEADZONE { // +Y is DOWN
                            inputs.push(ControllerInput::Down);
                        }
                    } else {
                        // Horizontal is stronger
                        if raw_x < -ANALOG_DEADZONE {       // -X is LEFT
                            inputs.push(ControllerInput::Left);
                        } else if raw_x > ANALOG_DEADZONE { // +X is RIGHT
                            inputs.push(ControllerInput::Right);
                        }
                    }
                }
                
                // We found our active stick. Stop processing other gamepads
                break;
            }
        }
        
        // Update the global neutral state
        self.analog_was_neutral = !any_stick_active;
        
        inputs
    }
}
