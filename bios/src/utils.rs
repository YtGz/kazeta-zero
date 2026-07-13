use crate::audio::play_new_bgm;
use crate::types::Screen;
use crate::{config, save, thread, Arc, BufReader, Child, Mutex};
use chrono::Local;
use kazeta_overlay::{OverlayClient, OverlayScreen, ToastStyle};
use macroquad::prelude::*;
use rodio::{buffer::SamplesBuffer, Sink};
use std::collections::HashMap;
use std::fs;
use std::io::BufRead;
use std::path::{Path, PathBuf};
use std::process::Command;
//use macroquad::audio::Sound;

// wrap text in certain menus so it doesn't clip outside the screen
pub fn wrap_text(text: &str, font: Font, font_size: u16, max_width: f32) -> Vec<String> {
    let mut lines = Vec::new();
    let space_width = measure_text(" ", Some(&font), font_size, 1.0).width;

    for paragraph in text.lines() {
        if paragraph.is_empty() {
            lines.push("".to_string());
            continue;
        }

        let mut current_line = String::new();
        let mut current_line_width = 0.0;

        for word in paragraph.split_whitespace() {
            let word_width = measure_text(word, Some(&font), font_size, 1.0).width;

            if !current_line.is_empty() && current_line_width + space_width + word_width > max_width
            {
                lines.push(current_line);
                current_line = String::new();
                current_line_width = 0.0;
            }

            if !current_line.is_empty() {
                current_line.push(' ');
                current_line_width += space_width;
            }

            current_line.push_str(word);
            current_line_width += word_width;
        }
        lines.push(current_line);
    }

    lines
}

/// Scans a directory and returns a sorted list of paths for files with given extensions.
pub fn find_asset_files(dir_path: &str, extensions: &[&str]) -> Vec<PathBuf> {
    if let Ok(entries) = fs::read_dir(dir_path) {
        let mut files: Vec<PathBuf> = entries
            .flatten()
            .map(|e| e.path())
            .filter(|path| {
                path.is_file()
                    && path
                        .extension()
                        .and_then(|s| s.to_str())
                        .map_or(false, |ext| extensions.contains(&ext))
            })
            .collect();
        files.sort();
        return files;
    }
    vec![]
}

// Helper to read the first line from a file containing a specific key
pub fn read_line_from_file(path: &str, key: &str) -> Option<String> {
    fs::read_to_string(path)
        .ok()?
        .lines()
        .find(|line| line.starts_with(key))
        .map(|line| line.replace(key, "").trim().to_string())
}

/// Calls a privileged helper script to copy session logs to the SD card.
pub fn copy_session_logs_to_sd() -> Result<String, String> {
    let output = Command::new("sudo")
        .arg("/usr/bin/kazeta-copy-logs")
        .output()
        .map_err(|e| format!("Failed to execute helper script: {}", e))?;

    if output.status.success() {
        // The script prints the destination path on success, so we can capture it.
        let stdout = String::from_utf8_lossy(&output.stdout);
        // Find the last line of output that contains the path
        let path_line = stdout.lines().last().unwrap_or("Log copy successful.");
        Ok(path_line.to_string())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(format!("Log copy script failed: {}", stderr.trim()))
    }
}

// FOR ACTUAL HARDWARE USE
pub fn trigger_session_restart(
    //current_bgm: &mut Option<Sound>,
    //music_cache: &HashMap<String, Sound>,
    current_bgm: &mut Option<Sink>,
    music_cache: &HashMap<String, SamplesBuffer>,
) -> (Screen, Option<f64>) {
    // Stop the BGM
    play_new_bgm("OFF", 0.0, music_cache, current_bgm);

    // Create the sentinel file at the correct system path
    let sentinel_path = Path::new("/var/kazeta/state/.RESTART_SESSION_SENTINEL");
    if let Some(parent) = sentinel_path.parent() {
        // Ensure the directory exists
        if fs::create_dir_all(parent).is_ok() {
            let _ = fs::File::create(sentinel_path);
        }
    }

    // Return the state to begin the fade-out
    (Screen::FadingOut, Some(get_time()))
}

pub fn trigger_game_launch(
    cart_info: &save::CartInfo,
    kzi_path: &Path,
    //current_bgm: &mut Option<Sound>,
    //music_cache: &HashMap<String, Sound>,
    current_bgm: &mut Option<Sink>,
    music_cache: &HashMap<String, SamplesBuffer>,
) -> (Screen, Option<f64>) {
    // Start the overlay daemon before launching the game
    if let Err(e) = start_overlay_daemon() {
        eprintln!("[WARNING] Failed to start overlay daemon: {}", e);
        // Don't fail the launch if overlay fails to start
    }

    // Notify overlay that the game is starting
    notify_game_started(
        &cart_info.id,
        cart_info.name.as_deref().unwrap_or(&cart_info.id),
        cart_info.runtime.as_deref().unwrap_or("unknown"),
    );

    // Setup RetroAchievements if enabled
    setup_retroachievements(cart_info, kzi_path);

    // Write the specific launch command for the selected game
    if let Err(e) = save::write_launch_command(kzi_path) {
        // If we fail, we should probably show an error on the debug screen
        // For now, we'll just print it for desktop debugging.
        println!("[ERROR] Failed to write launch command: {}", e);
    }

    // Now, trigger the standard session restart process,
    // which will find and execute our command file.
    trigger_session_restart(current_bgm, music_cache)
}

pub fn save_log_to_file(log_messages: &[String]) -> std::io::Result<String> {
    let timestamp = Local::now().format("%Y-%m-%d_%H-%M-%S");
    let filename = format!("kazeta_log_{}.log", timestamp);

    // In a real application, you'd save this to a logs directory.
    // For now, it will save in the same directory as the executable.
    fs::write(&filename, log_messages.join("\n"))?;

    println!("Log saved to {}", filename);
    Ok(filename)
}

pub fn start_log_reader(process: &mut Child, logs: Arc<Mutex<Vec<String>>>) {
    // Take ownership of the output pipes
    if let (Some(stdout), Some(stderr)) = (process.stdout.take(), process.stderr.take()) {
        let logs_clone_stdout = logs.clone();
        thread::spawn(move || {
            let reader = BufReader::new(stdout);
            for line in reader.lines().filter_map(|l| l.ok()) {
                logs_clone_stdout.lock().unwrap().push(line);
            }
        });

        let logs_clone_stderr = logs.clone();
        thread::spawn(move || {
            let reader = BufReader::new(stderr);
            for line in reader.lines().filter_map(|l| l.ok()) {
                logs_clone_stderr.lock().unwrap().push(line);
            }
        });
    }
}

/// Removes the file extension from a filename string slice.
pub fn trim_extension(filename: &str) -> &str {
    if let Some(dot_index) = filename.rfind('.') {
        &filename[..dot_index]
    } else {
        filename
    }
}

pub fn string_to_color(color_str: &str) -> Color {
    match color_str {
        "BLACK" => BLACK,
        "PINK" => PINK,
        "RED" => RED,
        "ORANGE" => ORANGE,
        "YELLOW" => YELLOW,
        "GREEN" => GREEN,
        "BLUE" => BLUE,
        "PURPLE" => VIOLET, // USING VIOLET AS A CLOSE APPROXIMATION
        _ => WHITE,         // Default to WHITE
    }
}

/// Parses a resolution string and requests a window resize.
pub fn apply_resolution(resolution_str: &str) {
    if let Some((w_str, h_str)) = resolution_str.split_once('x') {
        // Parse to f32 for the resize function
        if let (Ok(w), Ok(h)) = (w_str.parse::<f32>(), h_str.parse::<f32>()) {
            // Use the correct function name
            request_new_screen_size(w, h);
        }
    }
}

// ===================================
// OVERLAY FUNCTIONS
// ===================================
//
// These functions allow the BIOS to communicate with the overlay daemon.
// The overlay daemon must be running (started when a game launches) for these to work.
//
// Example usage:
//   - Show overlay from a menu option:
//       use crate::utils::show_overlay;
//       use kazeta_overlay::OverlayScreen;
//       show_overlay(OverlayScreen::Main);
//
//   - Show a toast notification:
//       use crate::utils::{show_info_toast, show_success_toast};
//       show_info_toast("Game launched successfully!");
//
//   - Unlock an achievement:
//       use crate::utils::unlock_achievement;
//       unlock_achievement("pokemon-emerald", "catch_first_pokemon");
//
//   - Check if overlay is available before using:
//       use crate::utils::{is_overlay_available, show_overlay};
//       use kazeta_overlay::OverlayScreen;
//       if is_overlay_available() {
//           show_overlay(OverlayScreen::Settings);
//       }

/// Start the overlay daemon as a background process
pub fn start_overlay_daemon() -> std::io::Result<()> {
    use std::io::Write;
    use std::process::Stdio;

    // Clean up any stale socket file first
    let socket_path = Path::new("/tmp/kazeta-overlay.sock");
    if socket_path.exists() {
        // Try to connect AND send a message to verify daemon is actually responsive
        use std::os::unix::net::UnixStream;
        match UnixStream::connect(socket_path) {
            Ok(mut stream) => {
                // Set a short timeout
                let _ = stream.set_write_timeout(Some(std::time::Duration::from_millis(100)));
                let _ = stream.set_read_timeout(Some(std::time::Duration::from_millis(100)));

                // Try to send a status query - if this succeeds, daemon is alive
                let test_msg = r#"{"type":"get_status"}"#;
                if stream.write_all(test_msg.as_bytes()).is_ok() {
                    println!("[Overlay] Daemon is already running and responsive");
                    return Ok(());
                }
                // Write failed - daemon not responsive
                println!("[Overlay] Socket exists but daemon not responsive, restarting...");
                let _ = fs::remove_file(socket_path);
            }
            Err(_) => {
                // Socket exists but daemon not responding - remove stale socket
                println!("[Overlay] Cleaning up stale socket file");
                let _ = fs::remove_file(socket_path);
            }
        }
    }

    // Try to find the overlay binary
    let overlay_bin = if crate::DEV_MODE {
        // In dev mode, try to find it in the overlay target directory
        let current_exe = std::env::current_exe().ok();
        let mut possible_paths = vec![
            PathBuf::from("../overlay/target/debug/kazeta-overlay"),
            PathBuf::from("../../overlay/target/debug/kazeta-overlay"),
            PathBuf::from("overlay/target/debug/kazeta-overlay"),
        ];

        if let Some(exe_path) = current_exe.as_ref() {
            if let Some(exe_dir) = exe_path.parent() {
                possible_paths.push(exe_dir.join("../overlay/target/debug/kazeta-overlay"));
                possible_paths.push(exe_dir.join("../../overlay/target/debug/kazeta-overlay"));
            }
        }

        if let Some(home) = dirs::home_dir() {
            possible_paths
                .push(home.join("sandbox/kazeta-plus/overlay/target/debug/kazeta-overlay"));
        }

        if let Ok(project_root) = std::env::var("KAZETA_PROJECT_ROOT") {
            possible_paths
                .push(PathBuf::from(project_root).join("overlay/target/debug/kazeta-overlay"));
        }

        possible_paths.iter()
            .find(|p| p.exists())
            .cloned()
            .unwrap_or_else(|| {
                eprintln!("[Overlay] Warning: Could not find overlay binary in dev mode, trying production path");
                PathBuf::from("/usr/bin/kazeta-overlay")
            })
    } else {
        // In production, use the system binary
        PathBuf::from("/usr/bin/kazeta-overlay")
    };

    // Check if binary exists
    if !overlay_bin.exists() {
        let err_msg = format!(
            "Overlay binary not found at: {}. Please build it with: cargo build --bin kazeta-overlay --features daemon",
            overlay_bin.display()
        );
        eprintln!("[Overlay] {}", err_msg);
        return Err(std::io::Error::new(std::io::ErrorKind::NotFound, err_msg));
    }

    println!(
        "[Overlay] Starting overlay daemon: {}",
        overlay_bin.display()
    );

    // Create a log file for overlay output (helps debug startup issues)
    let log_path = "/tmp/kazeta-overlay.log";
    let log_file = fs::OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(log_path)
        .ok();

    // Spawn the overlay daemon as a detached background process
    let mut cmd = Command::new(&overlay_bin);

    if let Some(file) = log_file {
        let stderr_file = file.try_clone().ok();
        cmd.stdout(Stdio::from(file));
        if let Some(stderr) = stderr_file {
            cmd.stderr(Stdio::from(stderr));
        } else {
            cmd.stderr(Stdio::null());
        }
    } else {
        cmd.stdout(Stdio::null());
        cmd.stderr(Stdio::null());
    }

    cmd.spawn().map_err(|e| {
        eprintln!("[Overlay] Failed to start overlay daemon: {}", e);
        e
    })?;

    // Give it a moment to start up (macroquad needs time to initialize window)
    std::thread::sleep(std::time::Duration::from_millis(1000));

    // Verify it started successfully by checking for socket
    for attempt in 0..3 {
        if is_overlay_available() {
            println!("[Overlay] Daemon started successfully");
            return Ok(());
        }
        if attempt < 2 {
            std::thread::sleep(std::time::Duration::from_millis(500));
        }
    }

    // Check log file for errors
    if let Ok(log_content) = fs::read_to_string(log_path) {
        if !log_content.is_empty() {
            eprintln!("[Overlay] Daemon output:\n{}", log_content);
        }
    }

    eprintln!(
        "[Overlay] Warning: Daemon may not have started correctly (socket not found after 2s)"
    );
    Ok(()) // Don't fail, overlay might work when game launches
}

/// Stop the overlay daemon (kills any running instance)
pub fn stop_overlay_daemon() {
    // Try to find and kill the overlay process
    // This is a simple approach - in production you might want to use a PID file
    let _ = Command::new("pkill")
        .arg("-f")
        .arg("kazeta-overlay")
        .output();

    // Also remove the socket file if it exists
    let socket_path = Path::new("/tmp/kazeta-overlay.sock");
    if socket_path.exists() {
        let _ = fs::remove_file(socket_path);
    }

    println!("[Overlay] Daemon stopped");
}

/// Check if the overlay daemon is available
pub fn is_overlay_available() -> bool {
    let client = OverlayClient::new();
    client.is_available()
}

/// Show the overlay menu with a specific screen
pub fn show_overlay(screen: OverlayScreen) {
    let client = OverlayClient::new();
    if client.is_available() {
        if let Err(e) = client.show_overlay(screen) {
            eprintln!("[Overlay] Failed to show overlay: {}", e);
        }
    } else {
        eprintln!("[Overlay] Overlay daemon is not available (socket not found)");
    }
}

/// Hide the overlay menu
pub fn hide_overlay() {
    let client = OverlayClient::new();
    if client.is_available() {
        if let Err(e) = client.hide_overlay() {
            eprintln!("[Overlay] Failed to hide overlay: {}", e);
        }
    }
}

/// Show a toast notification
pub fn show_toast(message: &str, style: ToastStyle) {
    let client = OverlayClient::new();
    if client.is_available() {
        if let Err(e) = client.show_toast(message, style, 3000) {
            eprintln!("[Overlay] Failed to show toast: {}", e);
        }
    }
}

/// Show an info toast
pub fn show_info_toast(message: &str) {
    show_toast(message, ToastStyle::Info);
}

/// Show a success toast
pub fn show_success_toast(message: &str) {
    show_toast(message, ToastStyle::Success);
}

/// Show a warning toast
pub fn show_warning_toast(message: &str) {
    show_toast(message, ToastStyle::Warning);
}

/// Show an error toast
pub fn show_error_toast(message: &str) {
    show_toast(message, ToastStyle::Error);
}

/// Unlock an achievement
pub fn unlock_achievement(cart_id: &str, achievement_id: &str) {
    let client = OverlayClient::new();
    if client.is_available() {
        if let Err(e) = client.unlock_achievement(cart_id, achievement_id) {
            eprintln!("[Overlay] Failed to unlock achievement: {}", e);
        }
    }
}

/// Notify the overlay that a game has started
pub fn notify_game_started(cart_id: &str, game_name: &str, runtime: &str) {
    use std::io::Write;
    use std::os::unix::net::UnixStream;

    let socket_path = "/tmp/kazeta-overlay.sock";
    if !Path::new(socket_path).exists() {
        return;
    }

    let message = serde_json::json!({
        "type": "game_started",
        "cart_id": cart_id,
        "game_name": game_name,
        "runtime": runtime,
    });

    if let Ok(mut stream) = UnixStream::connect(socket_path) {
        let _ = stream.set_write_timeout(Some(std::time::Duration::from_millis(100)));
        let _ = writeln!(stream, "{}", message);
        println!(
            "[Overlay] Notified game started: {} ({})",
            game_name, runtime
        );
    }
}

/// Notify the overlay that a game has stopped
pub fn notify_game_stopped(cart_id: &str) {
    use std::io::Write;
    use std::os::unix::net::UnixStream;

    let socket_path = "/tmp/kazeta-overlay.sock";
    if !Path::new(socket_path).exists() {
        return;
    }

    let message = serde_json::json!({
        "type": "game_stopped",
        "cart_id": cart_id,
    });

    if let Ok(mut stream) = UnixStream::connect(socket_path) {
        let _ = stream.set_write_timeout(Some(std::time::Duration::from_millis(100)));
        let _ = writeln!(stream, "{}", message);
        println!("[Overlay] Notified game stopped: {}", cart_id);
    }
}

/// Setup RetroAchievements for a game launch
/// This is called by the BIOS when launching a game
fn setup_retroachievements(cart_info: &save::CartInfo, kzi_path: &Path) {
    // If overlay isn't up, warn but continue (kazeta-ra will still run and may connect later)
    if !is_overlay_available() {
        println!("[RA] Overlay socket not available; RA notifications may not be shown");
        show_warning_toast("Overlay not running – achievements may not display");
    }

    // Check if kazeta-ra is available
    if Command::new("kazeta-ra").arg("status").output().is_err() {
        println!("[RA] kazeta-ra not found, skipping RetroAchievements");
        return;
    }

    // Check if RA is configured and enabled
    let status_output = match Command::new("kazeta-ra").arg("status").output() {
        Ok(output) => output,
        Err(_) => return,
    };

    let status_str = String::from_utf8_lossy(&status_output.stdout);
    if !status_str.contains("\"enabled\":true") && !status_str.contains("\"enabled\": true") {
        println!("[RA] RetroAchievements not enabled");
        return;
    }

    // Get ROM path from cartridge
    let rom_path = match get_rom_path_from_cartridge(cart_info, kzi_path) {
        Some(path) => path,
        None => {
            println!("[RA] Could not determine ROM path from cartridge (possibly a compressed .kzp package) - skipping RetroAchievements setup");
            show_warning_toast("RetroAchievements unavailable for this game (no ROM path)");
            return;
        }
    };

    if !rom_path.exists() {
        println!("[RA] ROM file not found: {}", rom_path.display());
        return;
    }

    println!(
        "[RA] Setting up RetroAchievements for: {}",
        rom_path.display()
    );

    // Call kazeta-ra game-start (this will hash the ROM, fetch game info, and notify overlay)
    // Run in background so it doesn't block game launch
    let rom_path_str = rom_path.to_string_lossy().to_string();
    thread::spawn(move || {
        let _ = Command::new("kazeta-ra")
            .arg("game-start")
            .arg("--path")
            .arg(&rom_path_str)
            .arg("--notify-overlay")
            .output();
    });

    // Also send achievement list to overlay (run in background)
    let rom_path_str2 = rom_path.to_string_lossy().to_string();
    thread::spawn(move || {
        // Small delay to let game-start complete first
        thread::sleep(std::time::Duration::from_millis(500));
        let _ = Command::new("kazeta-ra")
            .arg("send-achievements-to-overlay")
            .arg("--path")
            .arg(&rom_path_str2)
            .output();
    });
}

/// Get the ROM path from a cartridge
/// For .kzp files, returns None (will be handled by wrapper)
fn get_rom_path_from_cartridge(cart_info: &save::CartInfo, kzi_path: &Path) -> Option<PathBuf> {
    // For .kzp files, we can't easily get the ROM path without mounting
    // Return None and let the wrapper handle it
    if kzi_path.extension().and_then(|e| e.to_str()) == Some("kzp") {
        return None;
    }

    // For .kzi metadata files, assume the ROM lives next to the metadata
    kzi_path
        .parent()
        .map(|p| p.join(&cart_info.exec))
        .or_else(|| Some(PathBuf::from(&cart_info.exec)))
}
