mod controllers;
mod hotkeys;
mod ipc;
mod input;
mod menu_config;
mod performance;
mod playtime;
mod rendering;
mod state;
mod theme_config;
mod themes;
mod utils;

use anyhow::Result;
use state::OverlayState;
use std::time::{Duration, Instant};
use macroquad::prelude::*;

#[cfg(target_os = "macos")]
fn set_overlay_window_properties() {
    #[cfg(feature = "daemon")]
    {
        // Note: Window property setting via cocoa requires complex objc msg_send usage
        // For now, the window configuration (transparency, size) is handled via macroquad's Conf
        // Users can manually set "Always on Top" via macOS window menu or use a window manager
        // The overlay window should still appear above BIOS due to window creation order
        println!("[Overlay] macOS: Window properties configured via macroquad Conf");
        println!("[Overlay] macOS: For always-on-top, use Window > Always on Top in macOS menu");
    }
}

#[cfg(target_os = "linux")]
fn set_overlay_window_properties() {
    // On Linux, we'd need to use X11 or Wayland APIs
    // For now, rely on window manager or manual configuration
    // Users can use tools like `wmctrl` or `xdotool` to set always-on-top
    println!("[Overlay] Linux: Use window manager to set always-on-top");
    println!("[Overlay] Example: wmctrl -r 'Kazeta Overlay' -b add,above");
}

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
fn set_overlay_window_properties() {
    println!("[Overlay] Window properties not configured for this platform");
}

const TARGET_FPS: u64 = 60;
const FRAME_TIME: Duration = Duration::from_micros(1_000_000 / TARGET_FPS);

// Configure overlay window to be always-on-top and transparent
// Match BIOS window size for local testing
fn window_conf() -> Conf {
    Conf {
        window_title: "Kazeta Overlay".to_owned(),
        window_width: 640,   // Match BIOS window size
        window_height: 360,  // Match BIOS window size
        window_resizable: false,
        fullscreen: false,
        platform: miniquad::conf::Platform {
            apple_gfx_api: miniquad::conf::AppleGfxApi::Metal, // Prefer Metal on macOS to avoid GL pixel format issues
            linux_backend: miniquad::conf::LinuxBackend::X11Only,
            swap_interval: None,
            framebuffer_alpha: true,  // Enable transparency
            ..Default::default()
        },
        ..Default::default()
    }
}

#[macroquad::main(window_conf)]
async fn main() -> Result<()> {
    println!("[Overlay] Starting kazeta-overlay daemon...");

    // Initialize components
    let mut ipc_server = ipc::IpcServer::new()?;
    let mut input_monitor = input::HotkeyMonitor::new()?;
    let mut overlay_state = OverlayState::new().await;
    
    // Initialize gilrs for controller tracking
    #[cfg(feature = "daemon")]
    let mut gilrs = gilrs::Gilrs::new().unwrap_or_else(|e| {
        eprintln!("[Overlay] Warning: Failed to initialize gilrs: {}", e);
        panic!("gilrs required for controller support");
    });

    println!("[Overlay] IPC server listening on /tmp/kazeta-overlay.sock");
    println!("[Overlay] Hotkey monitor initialized");
    println!("[Overlay] Controller support enabled");
    println!("[Overlay] Overlay ready - press Guide button to toggle");

    // Set window properties for overlay behavior (after first frame to ensure window exists)
    let mut window_properties_set = false;

    loop {
        // Set window properties once after window is created
        if !window_properties_set {
            set_overlay_window_properties();
            window_properties_set = true;
        }
        let frame_start = Instant::now();

        // Check for hotkey press (Guide button, F12, or Ctrl+O)
        if input_monitor.check_hotkey_pressed() {
            overlay_state.toggle_visibility();
            println!("[Overlay] Toggled visibility: {}", overlay_state.is_visible());
        }

        // Check for performance overlay toggle (F3)
        if input_monitor.check_performance_hotkey_pressed() {
            overlay_state.performance.toggle_visibility();
            println!("[Overlay] Performance overlay: {}", overlay_state.performance.is_visible());
        }

        // Update connected controllers from gilrs
        #[cfg(feature = "daemon")]
        overlay_state.controllers.update_from_gilrs(&gilrs);

        // Process controller inputs (only when overlay is visible)
        if overlay_state.is_visible() {
            for input in input_monitor.poll_inputs() {
                overlay_state.handle_input(input);
            }
            
            // Update gamepad tester if on that screen
            #[cfg(feature = "daemon")]
            if overlay_state.current_screen == ipc::OverlayScreen::GamepadTester {
                overlay_state.controllers.update_tester_from_gilrs(&mut gilrs);
            }
        }

        // Process IPC messages
        for message in ipc_server.poll_messages() {
            overlay_state.handle_message(message);
        }

        // Update state
        overlay_state.update();

        // Record frame for performance tracking
        overlay_state.performance.record_frame();

        // If overlay is completely hidden (not rendering anything), reduce CPU usage
        if !overlay_state.should_render() && !overlay_state.performance.is_visible() {
            // Run at 20 FPS when idle to save CPU
            std::thread::sleep(Duration::from_millis(50));
            macroquad::prelude::next_frame().await;
            continue;
        }

        // Render (only if visible or toasts active)
        if overlay_state.should_render() {
            rendering::render(&overlay_state).await;
        } else {
            // Still need to call next_frame() for macroquad
            macroquad::prelude::next_frame().await;
        }

        // Frame timing
        let elapsed = frame_start.elapsed();
        if elapsed < FRAME_TIME {
            std::thread::sleep(FRAME_TIME - elapsed);
        }
    }
}
