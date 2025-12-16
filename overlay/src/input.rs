use anyhow::Result;
use macroquad::prelude::*;
use gilrs::{Gilrs, Button, Axis};

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
}

pub struct HotkeyMonitor {
    gilrs: Gilrs,
    analog_was_neutral: bool,
    // Track hotkey state to detect rising edge
    guide_button_last_state: bool,
    f12_last_state: bool,
    ctrl_o_last_o_state: bool,
    f3_last_state: bool,
}

impl HotkeyMonitor {
    pub fn new() -> Result<Self> {
        println!("[Input] Initializing input monitor (gilrs + macroquad)...");
        
        let gilrs = Gilrs::new()
            .map_err(|e| anyhow::anyhow!("Failed to initialize gilrs: {}", e))?;
        
        println!("[Input] Input monitor initialized");
        
        Ok(Self {
            gilrs,
            analog_was_neutral: true,
            guide_button_last_state: false,
            f12_last_state: false,
            ctrl_o_last_o_state: false,
            f3_last_state: false,
        })
    }

    pub fn check_hotkey_pressed(&mut self) -> bool {
        // Check for Guide button via gamepad
        let mut guide_pressed = false;
        while let Some(ev) = self.gilrs.next_event() {
            if let gilrs::EventType::ButtonPressed(Button::Mode, _) = ev.event {
                guide_pressed = true;
            }
        }
        
        // Check for F12 key
        let f12_currently_down = is_key_down(KeyCode::F12);
        let f12_just_pressed = f12_currently_down && !self.f12_last_state;
        self.f12_last_state = f12_currently_down;
        
        // Check for Ctrl+O
        let ctrl_held = is_key_down(KeyCode::LeftControl) || is_key_down(KeyCode::RightControl);
        let o_currently_down = is_key_down(KeyCode::O);
        let o_just_pressed = o_currently_down && !self.ctrl_o_last_o_state;
        self.ctrl_o_last_o_state = o_currently_down;
        let ctrl_o_detected = ctrl_held && o_just_pressed;
        
        guide_pressed || f12_just_pressed || ctrl_o_detected
    }

    pub fn check_performance_hotkey_pressed(&mut self) -> bool {
        // Check for F3 key
        let f3_currently_down = is_key_down(KeyCode::F3);
        let f3_just_pressed = f3_currently_down && !self.f3_last_state;
        self.f3_last_state = f3_currently_down;

        f3_just_pressed
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
