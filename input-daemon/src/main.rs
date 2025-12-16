//! kazeta-input: Global input daemon for Kazeta+ overlay
//!
//! This daemon reads raw input events from /dev/input/* using evdev,
//! allowing it to capture hotkeys regardless of which application has focus.
//!
//! Supports up to 4+ players with proper debouncing and hotplug detection.
//!
//! Monitored hotkeys:
//! - Guide/Home button on controllers (BTN_MODE)
//! - F12 key on keyboard
//! - Ctrl+O on keyboard
//!
//! When a hotkey is detected, it sends an IPC message to the overlay daemon.

use anyhow::{Context, Result};
use evdev::{Device, InputEventKind, Key};
use inotify::{Inotify, WatchMask};
use log::{debug, error, info, warn};
use std::collections::HashSet;
use std::fs;
use std::io::Write;
use std::os::unix::net::UnixStream;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

const OVERLAY_SOCKET: &str = "/tmp/kazeta-overlay.sock";
const INPUT_DIR: &str = "/dev/input";

// Global debounce time to prevent multiple triggers from different controllers
const HOTKEY_DEBOUNCE_MS: u64 = 300;

/// Global state shared across all device monitors
struct GlobalState {
    /// Last time any hotkey was triggered (global debounce)
    last_hotkey_time: Instant,
    /// Set of device paths currently being monitored
    monitored_devices: HashSet<String>,
}

impl GlobalState {
    fn new() -> Self {
        Self {
            last_hotkey_time: Instant::now() - Duration::from_secs(1), // Allow immediate first trigger
            monitored_devices: HashSet::new(),
        }
    }

    /// Try to trigger hotkey with global debounce
    /// Returns true if the trigger should proceed
    fn try_trigger(&mut self) -> bool {
        let now = Instant::now();
        if now.duration_since(self.last_hotkey_time).as_millis() as u64 > HOTKEY_DEBOUNCE_MS {
            self.last_hotkey_time = now;
            true
        } else {
            false
        }
    }
}

/// Track keyboard modifier state (per-device)
#[derive(Default)]
struct ModifierState {
    ctrl_left: bool,
    ctrl_right: bool,
}

impl ModifierState {
    fn ctrl_held(&self) -> bool {
        self.ctrl_left || self.ctrl_right
    }
}

/// Send a message to the overlay daemon
fn notify_overlay(message: &str) -> Result<()> {
    let socket_path = Path::new(OVERLAY_SOCKET);
    if !socket_path.exists() {
        debug!("Overlay socket not found, skipping notification");
        return Ok(());
    }

    let mut stream = UnixStream::connect(socket_path)
        .context("Failed to connect to overlay socket")?;
    
    stream.set_write_timeout(Some(Duration::from_millis(100)))?;
    writeln!(stream, "{}", message)?;
    
    info!("Sent to overlay: {}", message);
    Ok(())
}

/// Toggle the overlay visibility
fn toggle_overlay(state: &Arc<Mutex<GlobalState>>, device_name: &str) {
    // Use global debounce to prevent multiple controllers triggering at once
    let should_trigger = {
        let mut state = state.lock().unwrap();
        state.try_trigger()
    };

    if should_trigger {
        info!("Overlay toggle triggered by: {}", device_name);
        let message = r#"{"type":"toggle_overlay"}"#;
        if let Err(e) = notify_overlay(message) {
            warn!("Failed to toggle overlay: {}", e);
        }
    } else {
        debug!("Hotkey debounced (global) from: {}", device_name);
    }
}

/// Check if a device is a gamepad or keyboard we want to monitor
fn is_relevant_device(device: &Device) -> (bool, bool) {
    let supported = device.supported_keys();
    
    let is_gamepad = supported.as_ref()
        .map(|keys| keys.contains(Key::BTN_MODE) || keys.contains(Key::BTN_SOUTH))
        .unwrap_or(false);
    
    let is_keyboard = supported.as_ref()
        .map(|keys| keys.contains(Key::KEY_F12) || keys.contains(Key::KEY_A))
        .unwrap_or(false);

    (is_gamepad, is_keyboard)
}

/// Find all input devices (gamepads and keyboards)
fn find_input_devices(state: &Arc<Mutex<GlobalState>>) -> Vec<(String, Device)> {
    let mut devices = Vec::new();
    
    let input_path = Path::new(INPUT_DIR);
    if !input_path.exists() {
        error!("/dev/input does not exist - not running on Linux?");
        return devices;
    }

    let monitored = {
        let state = state.lock().unwrap();
        state.monitored_devices.clone()
    };

    if let Ok(entries) = fs::read_dir(input_path) {
        for entry in entries.flatten() {
            let path = entry.path();
            let path_str = path.to_string_lossy().to_string();
            
            let name = path.file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("");
            
            // Only look at event devices
            if !name.starts_with("event") {
                continue;
            }

            // Skip already monitored devices
            if monitored.contains(&path_str) {
                continue;
            }

            match Device::open(&path) {
                Ok(device) => {
                    let device_name = device.name().unwrap_or("Unknown");
                    let (is_gamepad, is_keyboard) = is_relevant_device(&device);

                    if is_gamepad || is_keyboard {
                        info!("Found input device: {} ({}) - gamepad={}, keyboard={}", 
                              path.display(), device_name, is_gamepad, is_keyboard);
                        devices.push((path_str, device));
                    }
                }
                Err(e) => {
                    // Permission denied is common for devices we don't have access to
                    if e.kind() != std::io::ErrorKind::PermissionDenied {
                        debug!("Failed to open {}: {}", path.display(), e);
                    }
                }
            }
        }
    }

    devices
}

/// Monitor a single input device for hotkeys
fn monitor_device(
    path: String,
    mut device: Device,
    running: Arc<AtomicBool>,
    state: Arc<Mutex<GlobalState>>,
) {
    let device_name = device.name().unwrap_or("Unknown").to_string();
    info!("Monitoring device: {} ({})", path, device_name);

    // Mark device as being monitored
    {
        let mut global = state.lock().unwrap();
        global.monitored_devices.insert(path.clone());
    }

    let mut modifiers = ModifierState::default();

    // Don't grab - let the game also receive inputs
    // We just want to monitor, not take exclusive control

    while running.load(Ordering::Relaxed) {
        // Fetch events with a timeout
        match device.fetch_events() {
            Ok(events) => {
                for event in events {
                    if let InputEventKind::Key(key) = event.kind() {
                        let pressed = event.value() == 1;

                        // Track modifier state
                        match key {
                            Key::KEY_LEFTCTRL => modifiers.ctrl_left = pressed,
                            Key::KEY_RIGHTCTRL => modifiers.ctrl_right = pressed,
                            _ => {}
                        }

                        // Check for hotkeys (only on press, not release)
                        if pressed {
                            let is_hotkey = match key {
                                // Guide/Home button on controller
                                Key::BTN_MODE => {
                                    debug!("Guide button pressed on {}", device_name);
                                    true
                                }
                                // F12 key
                                Key::KEY_F12 => {
                                    debug!("F12 pressed on {}", device_name);
                                    true
                                }
                                // O key (check if Ctrl held)
                                Key::KEY_O if modifiers.ctrl_held() => {
                                    debug!("Ctrl+O pressed on {}", device_name);
                                    true
                                }
                                _ => false,
                            };

                            if is_hotkey {
                                toggle_overlay(&state, &device_name);
                            }
                        }
                    }
                }
            }
            Err(e) => {
                if e.kind() == std::io::ErrorKind::WouldBlock {
                    // No events available, sleep briefly
                    thread::sleep(Duration::from_millis(10));
                } else {
                    // Device disconnected or error
                    warn!("Device {} disconnected or error: {}", path, e);
                    break;
                }
            }
        }
    }

    // Remove from monitored set when done
    {
        let mut global = state.lock().unwrap();
        global.monitored_devices.remove(&path);
    }

    info!("Stopped monitoring: {} ({})", path, device_name);
}

/// Event-driven device scanner using inotify (hotplug support)
fn device_scanner(
    running: Arc<AtomicBool>,
    state: Arc<Mutex<GlobalState>>,
) -> Vec<thread::JoinHandle<()>> {
    let mut handles = Vec::new();

    // Initialize inotify
    let mut inotify = match Inotify::init() {
        Ok(i) => i,
        Err(e) => {
            error!("Failed to initialize inotify: {}", e);
            error!("Falling back to initial device scan only (no hotplug)");
            return handles;
        }
    };

    // Watch /dev/input for new devices
    if let Err(e) = inotify.watches().add(INPUT_DIR, WatchMask::CREATE | WatchMask::ATTRIB) {
        error!("Failed to watch {}: {}", INPUT_DIR, e);
        error!("Falling back to initial device scan only (no hotplug)");
        return handles;
    }

    info!("Using inotify for event-driven device detection");

    // Buffer for inotify events
    let mut buffer = [0u8; 4096];

    // Run in a loop, blocking on inotify events
    // Note: This will block indefinitely until an event occurs or the process is terminated
    loop {
        if !running.load(Ordering::Relaxed) {
            break;
        }

        match inotify.read_events_blocking(&mut buffer) {
            Ok(events) => {
                for event in events {
                    if !running.load(Ordering::Relaxed) {
                        break;
                    }

                    // Check if this is an event device
                    if let Some(name) = event.name {
                        let name_str = name.to_string_lossy();
                        if !name_str.starts_with("event") {
                            continue;
                        }

                        let device_path = format!("{}/{}", INPUT_DIR, name_str);
                        debug!("inotify detected new device: {}", device_path);

                        // Give the device a moment to be ready
                        thread::sleep(Duration::from_millis(100));

                        // Check if it's already being monitored
                        let already_monitored = {
                            let state = state.lock().unwrap();
                            state.monitored_devices.contains(&device_path)
                        };

                        if already_monitored {
                            continue;
                        }

                        // Try to open the device
                        match Device::open(&device_path) {
                            Ok(device) => {
                                let device_name = device.name().unwrap_or("Unknown");
                                let (is_gamepad, is_keyboard) = is_relevant_device(&device);

                                if is_gamepad || is_keyboard {
                                    info!("New input device detected: {} ({}) - gamepad={}, keyboard={}",
                                          device_path, device_name, is_gamepad, is_keyboard);

                                    // Add to monitored set
                                    {
                                        let mut state = state.lock().unwrap();
                                        state.monitored_devices.insert(device_path.clone());
                                    }

                                    // Spawn monitor thread
                                    let running = running.clone();
                                    let state = state.clone();
                                    let handle = thread::spawn(move || {
                                        monitor_device(device_path, device, running, state);
                                    });
                                    handles.push(handle);
                                }
                            }
                            Err(e) => {
                                debug!("Failed to open device {}: {}", device_path, e);
                            }
                        }
                    }
                }
            }
            Err(e) => {
                error!("inotify read error: {}", e);
                // On error, sleep briefly before retrying
                thread::sleep(Duration::from_millis(1000));
            }
        }
    }

    handles
}

#[allow(unreachable_code)]
fn main() -> Result<()> {
    // Initialize logging
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
        .format_timestamp_millis()
        .init();

    info!("kazeta-input daemon starting...");
    info!("Supports up to 4+ players with hotplug detection");

    // This daemon only works on Linux (evdev)
    #[cfg(not(target_os = "linux"))]
    {
        error!("kazeta-input only works on Linux (requires evdev)");
        error!("On macOS/Windows, use the overlay's built-in input handling");
        return Ok(());
    }

    // Shared state for global debounce and device tracking
    let state = Arc::new(Mutex::new(GlobalState::new()));

    // Find initial devices
    let initial_devices = find_input_devices(&state);
    if initial_devices.is_empty() {
        warn!("No input devices found at startup.");
        warn!("Will continue scanning for hotplugged devices...");
        warn!("Troubleshooting:");
        warn!("  1. Add user to input group: sudo usermod -aG input $USER");
        warn!("  2. Log out and log back in");
        warn!("  3. Check device permissions: ls -la /dev/input/");
    } else {
        info!("Found {} input device(s) at startup", initial_devices.len());
    }

    // Shared running flag for graceful shutdown
    let running = Arc::new(AtomicBool::new(true));
    let running_ctrlc = running.clone();

    // Handle Ctrl+C
    ctrlc::set_handler(move || {
        info!("Received shutdown signal");
        running_ctrlc.store(false, Ordering::Relaxed);
    }).context("Failed to set Ctrl+C handler")?;

    // Spawn monitor threads for initial devices
    let mut handles = Vec::new();
    for (path, device) in initial_devices {
        let running = running.clone();
        let state = state.clone();
        let handle = thread::spawn(move || {
            monitor_device(path, device, running, state);
        });
        handles.push(handle);
    }

    info!("kazeta-input daemon ready");
    info!("Hotkeys: Guide button (any controller), F12, Ctrl+O");
    info!("Using inotify for event-driven hotplug detection");

    // Run device scanner in main thread, collecting new monitor handles
    let scanner_handles = device_scanner(running.clone(), state.clone());
    handles.extend(scanner_handles);

    // Wait for all threads to finish
    for handle in handles {
        let _ = handle.join();
    }

    info!("kazeta-input daemon stopped");
    Ok(())
}
