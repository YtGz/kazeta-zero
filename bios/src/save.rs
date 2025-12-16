use walkdir;
use chrono::DateTime;
use std::{
    fs, fmt,
    collections::VecDeque,
    io::{self, BufRead, Write, Read},
    path::{Path, PathBuf},
    process::{Command, Child, Stdio},
    sync::Arc,
    sync::atomic::{AtomicU16, Ordering},
};
use sysinfo::Disks;
use tar::{Builder, Archive};
use flate2::read::GzDecoder;

use crate::{
    DEV_MODE,
    config::get_user_data_dir,
    types::StorageMedia,
};

// ===================================
// CONSTANTS
// ===================================

// Directories to exclude from size calculation and copying
const EXCLUDED_DIRS: &[&str] = &[
    ".cache",
    ".config/pulse/cookie",
    ".kazeta/share",
    ".kazeta/var/prefix/dosdevices",
    ".kazeta/var/prefix/drive_c/windows",
    ".kazeta/var/prefix/pfx"
];

// ===================================
// STRUCTS
// ===================================

// get cart info
#[derive(Default, Clone, Debug)]
pub struct CartInfo {
    pub name: Option<String>,
    pub id: String,
    pub exec: String,
    pub icon: String,
    pub runtime: Option<String>, // runtime is optional
    // Multiplayer metadata (for mGBA and other emulators)
    pub multiplayer_support: Option<bool>,
    pub max_players: Option<u8>,
    pub multiplayer_type: Option<String>, // "link-cable", "wireless", "both"
    // RetroAchievements custom game name
    pub ra_game_name: Option<String>,
}

#[derive(Clone, Debug)]
pub struct StorageMediaState {
    pub all_media: Vec<StorageMedia>, // all storage media, including disabled media
    pub media: Vec<StorageMedia>,    // media that can actually be used
    pub selected: usize,    // the index of selection in 'media'
    pub needs_memory_refresh: bool,
}

// ===================================
// ENUMS
// ===================================

#[derive(Debug)]
pub enum SaveError {
    Io(io::Error),
    Message(String),
    Walkdir(walkdir::Error), // Add this variant
    StripPrefix(std::path::StripPrefixError), // Add this variant
}

// ===================================
// IMPLEMENTATIONS
// ===================================

impl StorageMediaState {
    pub fn new() -> Self {
        StorageMediaState {
            all_media: Vec::new(),
            media: Vec::new(),
            selected: 0,
            needs_memory_refresh: false,
        }
    }

    pub fn update_media(&mut self) {
        let mut all_new_media = Vec::new();

        if let Ok(devices) = list_devices() {
            for (id, free) in devices {
                all_new_media.push(StorageMedia {
                    id,
                    free,
                });
            }
        }

        // Done if media list has not changed
        if self.all_media.len() == all_new_media.len() &&
            !self.all_media.iter().zip(all_new_media.iter()).any(|(a, b)| a.id != b.id) {

                //  update free space
                self.all_media = all_new_media;
                for media in &mut self.media {
                    if let Some(pos) = self.all_media.iter().position(|m| m.id == media.id) {
                        media.free = self.all_media.get(pos).unwrap().free
                    }
                }

                return;
            }

            let new_media: Vec<StorageMedia> = all_new_media
            .clone()
            .into_iter()
            .filter(|m| has_save_dir(&m.id) && !is_cart(&m.id))
            .collect();

            // Try to keep the same device selected if it still exists
            let mut new_pos = 0;
            if let Some(old_selected_media) = self.media.get(self.selected) {
                if let Some(pos) = new_media.iter().position(|m| m.id == old_selected_media.id) {
                    new_pos = pos;
                }
            }

            self.all_media = all_new_media;
            self.media = new_media;
            self.selected = new_pos;
            self.needs_memory_refresh = true;
    }
}

// Implement Display to make the error printable
impl fmt::Display for SaveError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            SaveError::Io(err) => write!(f, "IO Error: {}", err),
            SaveError::Message(msg) => write!(f, "Save Error: {}", msg),
            SaveError::Walkdir(err) => write!(f, "Directory walking error: {}", err),
            SaveError::StripPrefix(err) => write!(f, "Path stripping error: {}", err),
        }
    }
}
impl std::error::Error for SaveError {}

impl From<io::Error> for SaveError { fn from(err: io::Error) -> Self { SaveError::Io(err) } }
impl From<String> for SaveError { fn from(msg: String) -> Self { SaveError::Message(msg) } }
impl From<walkdir::Error> for SaveError { fn from(err: walkdir::Error) -> Self { SaveError::Walkdir(err) } }
impl From<std::path::StripPrefixError> for SaveError { fn from(err: std::path::StripPrefixError) -> Self { SaveError::StripPrefix(err) } }

// ===================================
// FUNCTIONS
// ===================================

fn should_exclude_path(path: &Path) -> bool {
    let path_str = path.to_str().unwrap_or("");
    EXCLUDED_DIRS.iter().any(|&excluded| path_str.contains(excluded))
}

// [UPDATED] Accept a slice of extensions instead of a single &str
fn search_breadth_first(
    start_dir: &Path,
    extensions: &[&str],
    max_depth: usize,
    find_first: bool,
    results: &mut Vec<PathBuf>,
) {
    let mut queue = VecDeque::new();
    queue.push_back((start_dir.to_path_buf(), 0));

    while let Some((current_dir, depth)) = queue.pop_front() {
        if depth > max_depth {
            continue;
        }

        let entries = match fs::read_dir(&current_dir) {
            Ok(entries) => entries,
            Err(_) => continue, // Skip directories we can't read
        };

        let mut subdirs = Vec::new();

        // First, process all files in the current directory
        for entry in entries {
            let entry = match entry {
                Ok(entry) => entry,
                Err(_) => continue, // Skip entries we can't read
            };

            let path = entry.path();

            let metadata = match path.metadata() {
                Ok(metadata) => metadata,
                Err(_) => continue, // Skip files/dirs we can't get metadata for
            };

            if metadata.is_file() {
                // Check if file has any of the desired extensions
                if let Some(file_ext) = path.extension() {
                    let ext_str = file_ext.to_string_lossy();
                    // [UPDATED] Check against all allowed extensions
                    if extensions.iter().any(|e| ext_str.eq_ignore_ascii_case(e)) {
                        results.push(path);
                        if find_first {
                            return; // Exit immediately if we only want the first match
                        }
                    }
                }
            } else if metadata.is_dir() && depth < max_depth {
                // Collect subdirectories to process later
                subdirs.push(path);
            }
        }

        // Then add subdirectories to the queue for next level processing
        for subdir in subdirs {
            queue.push_back((subdir, depth + 1));
        }
    }
}

fn get_attribute(info_file: &Path, attribute: &str) -> io::Result<String> {
    let file = fs::File::open(info_file)?;
    let reader = io::BufReader::new(file);

    for line in reader.lines() {
        let line = line?;
        if line.starts_with(&format!("{}=", attribute)) {
            return Ok(line[attribute.len() + 1..].to_string());
        }
    }
    Ok(String::new())
}

/// Calculate playtime from a tar archive (external drives)
fn calculate_playtime_from_tar(tar_path: &Path, _cart_id: &str) -> f32 {
    let file = match fs::File::open(tar_path) {
        Ok(file) => file,
        Err(e) => {
            eprintln!("Failed to open tar file {}: {}", tar_path.display(), e);
            return 0.0;
        }
    };

    let mut archive = tar::Archive::new(file);
    let entries = match archive.entries() {
        Ok(entries) => entries,
        Err(e) => {
            eprintln!("Failed to read archive entries: {}", e);
            return 0.0;
        }
    };

    let mut content = String::new();
    let mut start_content = String::new();
    let mut end_content = String::new();

    for entry_result in entries {
        let mut entry = match entry_result {
            Ok(entry) => entry,
            Err(e) => {
                eprintln!("Failed to read tar entry: {}", e);
                continue;
            }
        };

        let path = match entry.path() {
            Ok(path) => path,
            Err(e) => {
                eprintln!("Failed to get tar entry path: {}", e);
                continue;
            }
        };

        if path.display().to_string() == ".kazeta/var/playtime.log" {
            let _ = entry.read_to_string(&mut content);
        } else if path.display().to_string() == ".kazeta/var/playtime_start" {
            let _ = entry.read_to_string(&mut start_content);
        } else if path.display().to_string() == ".kazeta/var/playtime_end" {
            let _ = entry.read_to_string(&mut end_content);
        }
    }

    parse_playtime_content(&format!("{}\n{} {}", content.trim(), start_content.trim(), end_content.trim()))
}

/// Calculate playtime from a directory (internal drives)
fn calculate_playtime_from_dir(dir_path: &Path, _cart_id: &str) -> f32 {
    let playtime_log_path = dir_path.join(".kazeta/var/playtime.log");
    let playtime_start_path = dir_path.join(".kazeta/var/playtime_start");
    let playtime_end_path = dir_path.join(".kazeta/var/playtime_end");

    let content = match fs::read_to_string(&playtime_log_path) {
        Ok(content) => content.trim().to_string(),
        Err(_) => "".to_string(),
    };

    let start_content = match fs::read_to_string(&playtime_start_path) {
        Ok(content) => content.trim().to_string(),
        Err(_) => "".to_string(),
    };

    let end_content = match fs::read_to_string(&playtime_end_path) {
        Ok(content) => content.trim().to_string(),
        Err(_) => "".to_string(),
    };

    return parse_playtime_content(&format!("{}\n{} {}", content.trim(), start_content.trim(), end_content.trim()));
}

/// Parse playtime content from a string (common logic for both tar and directory)
fn parse_playtime_content(content: &str) -> f32 {
    let mut total_seconds: i64 = 0;

    for line in content.lines() {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() != 2 {
            continue;
        }

        let start_time = match DateTime::parse_from_rfc3339(parts[0]) {
            Ok(dt) => dt,
            Err(e) => {
                eprintln!("Failed to parse start time '{}': {}", parts[0], e);
                continue;
            }
        };

        let end_time = match DateTime::parse_from_rfc3339(parts[1]) {
            Ok(dt) => dt,
            Err(e) => {
                eprintln!("Failed to parse end time '{}': {}", parts[1], e);
                continue;
            }
        };

        let duration = end_time.signed_duration_since(start_time);
        total_seconds += duration.num_seconds();
    }

    // Convert to hours rounded to one decimal place
    ((total_seconds as f64 / 360.0).round() / 10.0) as f32
}

/// Calculate size from a tar archive (external drives)
fn calculate_size_from_tar(tar_path: &Path) -> u64 {
    let metadata = match fs::metadata(tar_path) {
        Ok(metadata) => metadata,
        Err(e) => {
            eprintln!("Failed to get tar file metadata: {}", e);
            return 0;
        }
    };
    metadata.len()
}

/// Calculate size from a directory (internal drives)
fn calculate_size_from_dir(dir_path: &Path) -> u64 {
    let mut total_size = 0u64;

    for entry in walkdir::WalkDir::new(dir_path)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| {
            let path = e.path();
            // Skip excluded directories and their contents
            !should_exclude_path(path) &&
            path.is_file()
        }) {
            if let Ok(metadata) = entry.metadata() {
                total_size += metadata.len();
            }
        }
        total_size
}

fn sync_to_disk() {
    if let Ok(output) = Command::new("sync")
        .output()
        .map_err(|e| format!("Failed to execute sync command: {}", e)) {

            if !output.status.success() {
                println!("Sync command failed with status: {}", output.status);
            }
        }
}

/// Returns the correct directory for state files based on the environment.
fn get_state_dir() -> std::io::Result<PathBuf> {
    let path = if DEV_MODE {
        // In dev mode, use a user-writable path like ~/.local/share/kazeta-plus/state
        get_user_data_dir().unwrap().join("state")
    } else {
        // In production on the device, use the system path
        PathBuf::from("/var/kazeta/state")
    };

    // Ensure the directory exists, creating it if necessary.
    fs::create_dir_all(&path)?;
    Ok(path)
}

// ===================================
// PUBLIC FUNCTIONS
// ===================================

pub fn write_launch_command(kzi_path: &Path) -> std::io::Result<()> {
    //let state_dir = Path::new("/var/kazeta/state");
    //fs::create_dir_all(state_dir)?; // Ensure the directory exists
    let state_dir = get_state_dir()?;

    let launch_cmd_path = state_dir.join(".LAUNCH_CMD");
    let mut file = fs::File::create(launch_cmd_path)?;

    // The command tells the kazeta script which specific .kzi to launch,
    // bypassing the auto-detection.
    // The single quotes are important to handle paths with spaces.
    let command = format!("/usr/bin/kazeta '{}'", kzi_path.display());

    writeln!(file, "{}", command)?;

    Ok(())
}

// [UPDATED] Searches for both kzi and kzp
pub fn find_all_game_files() -> Result<(Vec<PathBuf>, Vec<String>), SaveError> {
    let mut debug_log = Vec::new();

    // Check for development games directory first (for macOS/dev testing)
    // Only check in dev mode
    let dev_games_dir = if DEV_MODE {
        dirs::home_dir()
            .map(|h| h.join("kazeta-games"))
            .filter(|p| p.exists())
    } else {
        None
    };

    let mount_dir = if let Some(dev_dir) = dev_games_dir {
        debug_log.push(format!("[Debug] Using development games directory: {}", dev_dir.display()));
        dev_dir.to_string_lossy().to_string()
    } else {
        "/run/media/".to_string()
    };

    debug_log.push(format!("[Debug] Searching for .kzi and .kzp files in '{}' (max depth: 2)...", mount_dir));

    // Search for both extensions
    match find_files_by_extension(&mount_dir, &["kzi", "kzp"], 2, false) {
        Ok(files) => {
            debug_log.push(format!("[Debug] Found {} potential game file(s).", files.len()));
            for (i, path) in files.iter().enumerate() {
                debug_log.push(format!("[Debug]    {}: {}", i + 1, path.display()));
            }
            Ok((files, debug_log))
        }
        Err(e) => {
            let error_msg = format!("Error while scanning '{}': {}", mount_dir, e);
            debug_log.push(error_msg.clone());
            Err(SaveError::Message(error_msg))
        }
    }
}

/// Parses a specific .kzi file and returns its metadata.
pub fn parse_kzi_file(kzi_path: &Path) -> Result<CartInfo, SaveError> {
    // Open the .kzi file (which is a gzip-compressed tar archive)
    let file = fs::File::open(kzi_path)?;
    let decoder = GzDecoder::new(file);
    let mut archive = Archive::new(decoder);

    // Find and read the cartridge.toml file from the archive
    let mut content = String::new();
    let mut found_toml = false;

    for entry in archive.entries()? {
        let mut entry = entry?;
        let path = entry.path()?;

        if path.file_name().and_then(|n| n.to_str()) == Some("cartridge.toml") {
            entry.read_to_string(&mut content)?;
            found_toml = true;
            break;
        }
    }

    if !found_toml {
        return Err(SaveError::Message(format!(
            "No cartridge.toml found in .kzi file: '{}'",
            kzi_path.display()
        )));
    }

    // Parse the TOML content
    let toml_value: toml::Value = toml::from_str(&content)
        .map_err(|e| SaveError::Message(format!("Failed to parse cartridge.toml: {}", e)))?;

    // Extract fields from TOML
    let name = toml_value.get("name").and_then(|v| v.as_str()).map(String::from);
    let id = toml_value.get("id").and_then(|v| v.as_str()).map(String::from);
    let exec = toml_value.get("exec").and_then(|v| v.as_str()).map(String::from);
    let icon = toml_value.get("icon").and_then(|v| v.as_str()).map(String::from)
        .or_else(|| Some("default.png".to_string())); // Default icon if not specified
    let runtime = toml_value.get("runtime").and_then(|v| v.as_str()).map(String::from);
    let multiplayer_support = toml_value.get("multiplayer_support").and_then(|v| v.as_bool());
    let max_players = toml_value.get("max_players").and_then(|v| v.as_integer()).map(|n| n as u8);
    let multiplayer_type = toml_value.get("multiplayer_type").and_then(|v| v.as_str()).map(String::from);
    let ra_game_name = toml_value.get("ra_game_name").and_then(|v| v.as_str()).map(String::from);

    if let (Some(id), Some(exec), Some(icon)) = (id, exec, icon) {
        Ok(CartInfo {
            name,
            id,
            exec,
            icon,
            runtime,
            multiplayer_support,
            max_players,
            multiplayer_type,
            ra_game_name,
        })
    } else {
        Err(SaveError::Message(format!(
            "Invalid cartridge.toml in '{}'. Missing required fields (id, exec, or icon).",
            kzi_path.display()
        )))
    }
}

/// Options for launching an mGBA game with multiplayer and save slot selection
#[derive(Clone, Debug, Default)]
pub struct MgbaLaunchOptions {
    pub player_count: u8,           // 1-4 players
    pub save_slots: Vec<String>,    // Save slot for each player (e.g., ["p1", "p2"])
}

// for debug game launch
// [UPDATED] Added logic to handle .kzp files by invoking the wrapper script directly
// [UPDATED] Added multiplayer support with environment variables for mGBA and other emulators
pub fn launch_game(cart_info: &CartInfo, kzi_path: &Path) -> std::io::Result<Child> {
    // Default to single player with default save slot
    launch_game_with_options(cart_info, kzi_path, None)
}

/// Launch a game with optional VBA-M-specific options (player count, save slots)
pub fn launch_game_with_options(
    cart_info: &CartInfo,
    kzi_path: &Path,
    mgba_options: Option<&MgbaLaunchOptions>,
) -> std::io::Result<Child> {
    // Setup RetroAchievements if enabled (for dev mode)
    setup_retroachievements_for_launch(cart_info, kzi_path);
    // Check if this is a compressed package (.kzp)
    if kzi_path.extension().map_or(false, |ext| ext.eq_ignore_ascii_case("kzp")) {
        println!("[Debug] Launching compressed package directly via kazeta wrapper: {}", kzi_path.display());

        // We cannot use standard 'Exec' logic because the exec path is inside the image.
        // We just tell the wrapper script to handle this package.
        let mut command = Command::new("/usr/bin/kazeta");
        command.arg(kzi_path);

        // Set multiplayer environment variables based on options or cart_info
        if let Some(opts) = mgba_options {
            if opts.player_count > 1 {
                command.env("MGBA_MULTIPLAYER", "true");
                command.env("MGBA_PLAYERS", opts.player_count.to_string());
                // Pass save slots as comma-separated list
                command.env("MGBA_SAVE_SLOTS", opts.save_slots.join(","));
                println!("[Debug] Multiplayer enabled - {} players, slots: {:?}", opts.player_count, opts.save_slots);
            } else {
                // Single player with selected save slot
                if !opts.save_slots.is_empty() {
                    command.env("MGBA_SAVE_SLOT", &opts.save_slots[0]);
                    println!("[Debug] Single player with save slot: {}", opts.save_slots[0]);
                }
            }
            if let Some(max_players) = cart_info.max_players {
                command.env("MGBA_MAX_PLAYERS", max_players.to_string());
            }
            if let Some(ref mp_type) = cart_info.multiplayer_type {
                command.env("MGBA_MULTIPLAYER_TYPE", mp_type);
            }
        } else if let Some(true) = cart_info.multiplayer_support {
            // Legacy behavior: default to 2 players if no options provided
            command.env("MGBA_MULTIPLAYER", "true");
            if let Some(max_players) = cart_info.max_players {
                command.env("MGBA_MAX_PLAYERS", max_players.to_string());
            }
            if let Some(ref mp_type) = cart_info.multiplayer_type {
                command.env("MGBA_MULTIPLAYER_TYPE", mp_type);
            }
            command.env("MGBA_PLAYERS", "2");
            println!("[Debug] Multiplayer enabled - defaulting to 2 players");
        }

        return command
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn();
    }

    // --- Standard Folder-Based Launch Logic (.kzi) ---
    // For .kzi files, we need to extract them to a temporary location first
    let extract_dir = if DEV_MODE {
        // In dev mode, use a cache directory to avoid re-extracting every time
        get_user_data_dir().unwrap().join("kzi-cache").join(&cart_info.id)
    } else {
        // In production, extract to /tmp
        PathBuf::from("/tmp").join("kazeta-kzi").join(&cart_info.id)
    };

    // Extract the .kzi if it hasn't been extracted yet or if it's newer than the cache
    let needs_extraction = !extract_dir.exists() ||
        fs::metadata(kzi_path).and_then(|kzi_meta|
            fs::metadata(&extract_dir).map(|cache_meta|
                kzi_meta.modified().unwrap() > cache_meta.modified().unwrap()
            )
        ).unwrap_or(true);

    if needs_extraction {
        println!("[Debug] Extracting .kzi to: {}", extract_dir.display());
        fs::create_dir_all(&extract_dir)?;

        let file = fs::File::open(kzi_path)?;
        let decoder = GzDecoder::new(file);
        let mut archive = Archive::new(decoder);
        archive.unpack(&extract_dir)?;

        println!("[Debug] Extraction complete");
    } else {
        println!("[Debug] Using cached extraction at: {}", extract_dir.display());
    }

    let game_root = extract_dir;

    // Check for embedded save files and copy them to the save directory
    // Save files can be included in the .kzi archive for distribution
    // Use "internal" as the drive name for dev mode, or get it from the KZI path
    let drive_name = if DEV_MODE { "internal" } else { "internal" }; // TODO: detect actual drive
    let save_dir = PathBuf::from(get_save_dir_from_drive_name(drive_name)).join(&cart_info.id);
    fs::create_dir_all(&save_dir)?;

    // Use a marker file to track if saves have been imported from this .kzi
    // This prevents re-importing even if the user deletes their save file
    let import_marker = save_dir.join(".saves-imported");
    let should_import_saves = !import_marker.exists();

    if should_import_saves {
        // Look for .sav files in the extracted game root
        if let Ok(entries) = fs::read_dir(&game_root) {
            let mut copied_saves = Vec::new();
            let rom_name = get_rom_name_from_exec(&cart_info.exec);

            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().and_then(|e| e.to_str()) == Some("sav") {
                    let save_filename = path.file_stem().unwrap().to_string_lossy().to_string();

                    // Determine destination filename based on runtime and save file naming
                    // For VBA-M multiplayer: Support both slotted saves and base saves
                    let is_vbam = cart_info.runtime.as_deref() == Some("vba-m");

                    // Check if save file already has a slot suffix (e.g., "pokemon_p1.sav")
                    let has_slot_suffix = save_filename.ends_with("_p1")
                                       || save_filename.ends_with("_p2")
                                       || save_filename.ends_with("_p3")
                                       || save_filename.ends_with("_p4");

                    let dest_filename = if is_vbam && !has_slot_suffix {
                        // Base save file (e.g., "pokemon.sav") - keep as is for discovery
                        // Also create a _p1 version for initial Player 1 usage
                        let base_dest = save_dir.join(path.file_name().unwrap());
                        let p1_dest = save_dir.join(format!("{}_p1.sav", rom_name));

                        // Copy to both locations
                        if !base_dest.exists() {
                            if let Ok(_) = fs::copy(&path, &base_dest) {
                                println!("[Debug] Copied embedded save file: {}", path.file_name().unwrap().to_string_lossy());
                                copied_saves.push(path.file_name().unwrap().to_string_lossy().to_string());
                            }
                        }

                        // Also create p1 version if it doesn't exist
                        if !p1_dest.exists() {
                            if let Ok(_) = fs::copy(&path, &p1_dest) {
                                println!("[Debug] Created p1 save from base: {}_p1.sav", rom_name);
                            }
                        }

                        continue; // Already handled, skip to next file
                    } else {
                        // Either not a GBA game, or already has slot suffix - copy as-is
                        path.file_name().unwrap().to_string_lossy().to_string()
                    };

                    let dest_path = save_dir.join(&dest_filename);

                    // Only copy if the save doesn't already exist (don't overwrite user saves)
                    if !dest_path.exists() {
                        match fs::copy(&path, &dest_path) {
                            Ok(_) => {
                                println!("[Debug] Copied embedded save file: {}", dest_filename);
                                copied_saves.push(dest_filename);
                            }
                            Err(e) => {
                                eprintln!("[Warning] Failed to copy save file {}: {}", dest_filename, e);
                            }
                        }
                    } else {
                        println!("[Debug] Save file already exists, skipping: {}", dest_filename);
                    }
                }
            }

            if !copied_saves.is_empty() {
                println!("[Info] Imported {} embedded save file(s): {}", copied_saves.len(), copied_saves.join(", "));
            }

            // Create the marker file to prevent future imports
            if let Err(e) = fs::File::create(&import_marker) {
                eprintln!("[Warning] Failed to create import marker: {}", e);
            }
        }
    } else {
        println!("[Debug] Save files already imported (marker exists), skipping");
    }

    println!("[Debug] Game Root: {}", game_root.display());
    println!("[Debug] Exec Command: {}", &cart_info.exec);

    // Handle vba-m runtime specially - it needs the wrapper script
    if cart_info.runtime.as_deref() == Some("vba-m") {
        if DEV_MODE {
            // In dev mode, look for the wrapper in the runtimes directory
            let current_exe = std::env::current_exe().ok();
            let mut possible_paths = vec![
                PathBuf::from("../runtimes/gba/vba-run-wrapper.sh"),
                PathBuf::from("../../runtimes/gba/vba-run-wrapper.sh"),
                PathBuf::from("runtimes/gba/vba-run-wrapper.sh"),
            ];

            // Try relative to the executable location
            if let Some(exe_path) = current_exe.as_ref() {
                if let Some(exe_dir) = exe_path.parent() {
                    possible_paths.push(exe_dir.join("../runtimes/gba/vba-run-wrapper.sh"));
                    possible_paths.push(exe_dir.join("../../runtimes/gba/vba-run-wrapper.sh"));
                }
            }

            // Try absolute path based on common dev setup
            if let Some(home) = dirs::home_dir() {
                possible_paths.push(home.join("sandbox/kazeta-plus/runtimes/gba/vba-run-wrapper.sh"));
            }

            // Check environment variable for project root
            if let Ok(project_root) = std::env::var("KAZETA_PROJECT_ROOT") {
                possible_paths.push(PathBuf::from(project_root).join("runtimes/gba/vba-run-wrapper.sh"));
            }

            let wrapper_path = possible_paths.iter()
                .find(|p| p.exists())
                .cloned()
                .unwrap_or_else(|| {
                    eprintln!("[Warning] Could not find vba-run-wrapper.sh, trying default path");
                    PathBuf::from("runtimes/gba/vba-run-wrapper.sh")
                });

            // Canonicalize to absolute path to avoid issues when changing working directory
            let wrapper_path = wrapper_path.canonicalize()
                .unwrap_or_else(|e| {
                    eprintln!("[Warning] Failed to canonicalize wrapper path: {}", e);
                    wrapper_path.clone()
                });

            println!("[Debug] Using VBA-M wrapper script: {}", wrapper_path.display());

            let rom_path = game_root.join(&cart_info.exec);
            let mut command = Command::new("bash");
            command.arg(&wrapper_path);
            command.arg(&rom_path);
            command.arg(&cart_info.id);

            // Set multiplayer environment variables based on options or cart_info
            if let Some(opts) = mgba_options {
                if opts.player_count > 1 {
                    command.env("VBA_MULTIPLAYER", "true");
                    command.env("VBA_PLAYERS", opts.player_count.to_string());
                    command.env("VBA_SAVE_SLOTS", opts.save_slots.join(","));
                    println!("[Debug] VBA-M Multiplayer enabled - {} players, slots: {:?}", opts.player_count, opts.save_slots);
                } else {
                    // Single player with selected save slot
                    if !opts.save_slots.is_empty() {
                        command.env("VBA_SAVE_SLOT", &opts.save_slots[0]);
                        println!("[Debug] VBA-M Single player with save slot: {}", opts.save_slots[0]);
                    }
                }
                if let Some(max_players) = cart_info.max_players {
                    command.env("VBA_MAX_PLAYERS", max_players.to_string());
                }
                if let Some(ref mp_type) = cart_info.multiplayer_type {
                    command.env("VBA_MULTIPLAYER_TYPE", mp_type);
                }
            } else if let Some(true) = cart_info.multiplayer_support {
                command.env("VBA_MULTIPLAYER", "true");
                if let Some(max_players) = cart_info.max_players {
                    command.env("VBA_MAX_PLAYERS", max_players.to_string());
                }
                if let Some(ref mp_type) = cart_info.multiplayer_type {
                    command.env("VBA_MULTIPLAYER_TYPE", mp_type);
                }
                command.env("VBA_PLAYERS", "2");
                println!("[Debug] VBA-M Multiplayer enabled - defaulting to 2 players");
            }

            return command
                .current_dir(game_root)
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn();
        } else {
            // In production, use the kazeta wrapper script
            println!("[Debug] Launching .kzi with vba-m runtime via kazeta wrapper: {}", kzi_path.display());

            let mut command = Command::new("/usr/bin/kazeta");
            command.arg(kzi_path);

            // Set multiplayer environment variables
            if let Some(opts) = mgba_options {
                if opts.player_count > 1 {
                    command.env("VBA_MULTIPLAYER", "true");
                    command.env("VBA_PLAYERS", opts.player_count.to_string());
                    command.env("VBA_SAVE_SLOTS", opts.save_slots.join(","));
                } else if !opts.save_slots.is_empty() {
                    command.env("VBA_SAVE_SLOT", &opts.save_slots[0]);
                }
                if let Some(max_players) = cart_info.max_players {
                    command.env("VBA_MAX_PLAYERS", max_players.to_string());
                }
                if let Some(ref mp_type) = cart_info.multiplayer_type {
                    command.env("VBA_MULTIPLAYER_TYPE", mp_type);
                }
            } else if let Some(true) = cart_info.multiplayer_support {
                command.env("VBA_MULTIPLAYER", "true");
                if let Some(max_players) = cart_info.max_players {
                    command.env("VBA_MAX_PLAYERS", max_players.to_string());
                }
                if let Some(ref mp_type) = cart_info.multiplayer_type {
                    command.env("VBA_MULTIPLAYER_TYPE", mp_type);
                }
                command.env("VBA_PLAYERS", "2");
            }

            return command
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn();
        }
    }

    // Use a `match` block to create the base command
    let mut cmd = match cart_info.runtime.as_deref().unwrap_or("linux") {
        "windows" => {
            let mut command = Command::new("wine");
            command.arg(&cart_info.exec);
            command // Return the command builder
        }
        _ => { // Default to "linux"
            let mut command = Command::new("sh");
            command.arg("-c").arg(&cart_info.exec);
            command // Return the command builder
        }
    };

    // Set multiplayer environment variables if supported
    if let Some(true) = cart_info.multiplayer_support {
        cmd.env("MGBA_MULTIPLAYER", "true");
        if let Some(max_players) = cart_info.max_players {
            cmd.env("MGBA_MAX_PLAYERS", max_players.to_string());
        }
        if let Some(ref mp_type) = cart_info.multiplayer_type {
            cmd.env("MGBA_MULTIPLAYER_TYPE", mp_type);
        }

        // Use options if provided, otherwise default to 2 players
        if let Some(opts) = mgba_options {
            cmd.env("MGBA_PLAYERS", opts.player_count.to_string());
            if !opts.save_slots.is_empty() {
                cmd.env("MGBA_SAVE_SLOTS", opts.save_slots.join(","));
            }
        } else {
            cmd.env("MGBA_PLAYERS", "2");
        }

        println!("[Debug] Multiplayer enabled");
    }

    // Now, apply the common settings and spawn the process
    cmd.current_dir(game_root)
    .stdout(Stdio::piped())
    .stderr(Stdio::piped())
    .spawn()
}

/// Get the save directory path for a cart
pub fn get_mgba_save_dir(cart_id: &str) -> PathBuf {
    let base_dir = dirs::home_dir().unwrap().join(".local/share/kazeta/saves/default");
    base_dir.join(cart_id)
}

/// Get the ROM name from the exec path (without extension)
pub fn get_rom_name_from_exec(exec: &str) -> String {
    std::path::Path::new(exec)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("game")
        .to_string()
}

/// Searches for files with a given extension within a directory up to a specified depth
/// [UPDATED] 'extension' argument changed to 'extensions' (slice of &str)
pub fn find_files_by_extension<P: AsRef<Path>>(
    dir: P,
    extensions: &[&str],
    max_depth: usize,
    find_first: bool,
) -> Result<Vec<PathBuf>, io::Error> {
    let dir_path = dir.as_ref();

    // Check if initial directory exists and is accessible
    if !dir_path.exists() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!("Directory does not exist: {}", dir_path.display())
        ));
    }

    // Try to read the initial directory to ensure it's accessible
    fs::read_dir(dir_path)?;

    let mut results = Vec::new();
    search_breadth_first(dir_path, extensions, max_depth, find_first, &mut results);
    Ok(results)
}

pub fn get_save_dir_from_drive_name(drive_name: &str) -> String {
    let base_dir = dirs::home_dir().unwrap().join(".local/share/kazeta");
    if drive_name == "internal" || drive_name.is_empty() {
        let save_dir = base_dir.join("saves/default");
        if !save_dir.exists() {
            fs::create_dir_all(&save_dir).unwrap_or_else(|e| {
                eprintln!("Failed to create save directory: {}", e);
            });
        }
        save_dir.to_string_lossy().into_owned()
    } else {
        let base_ext = if Path::new("/media").read_dir().map(|mut d| d.next().is_none()).unwrap_or(true) {
            if Path::new(&format!("/run/media/{}", whoami::username())).exists() {
                format!("/run/media/{}", whoami::username())
            } else {
                "/run/media".to_string()
            }
        } else {
            "/media".to_string()
        };

        let save_dir = Path::new(&base_ext).join(drive_name).join("kazeta/saves");
        if !save_dir.exists() {
            fs::create_dir_all(&save_dir).unwrap_or_else(|e| {
                eprintln!("Failed to create save directory: {}", e);
            });
        }
        save_dir.to_string_lossy().into_owned()
    }
}

pub fn get_cache_dir_from_drive_name(drive_name: &str) -> String {
    let base_dir = dirs::home_dir().unwrap().join(".local/share/kazeta");
    if drive_name == "internal" || drive_name.is_empty() {
        let cache_dir = base_dir.join("cache");
        if !cache_dir.exists() {
            fs::create_dir_all(&cache_dir).unwrap_or_else(|e| {
                eprintln!("Failed to create cache directory: {}", e);
            });
        }
        cache_dir.to_string_lossy().into_owned()
    } else {
        let base_ext = if Path::new("/media").read_dir().map(|mut d| d.next().is_none()).unwrap_or(true) {
            if Path::new(&format!("/run/media/{}", whoami::username())).exists() {
                format!("/run/media/{}", whoami::username())
            } else {
                "/run/media".to_string()
            }
        } else {
            "/media".to_string()
        };

        let cache_dir = Path::new(&base_ext).join(drive_name).join("kazeta/cache");
        if !cache_dir.exists() {
            fs::create_dir_all(&cache_dir).unwrap_or_else(|e| {
                eprintln!("Failed to create cache directory: {}", e);
            });
        }
        cache_dir.to_string_lossy().into_owned()
    }
}

pub fn list_devices() -> io::Result<Vec<(String, u32)>> {
    let mut devices = Vec::new();
    let disks = Disks::new_with_refreshed_list();

    // Add internal drive
    let base_dir = dirs::home_dir().unwrap().join(".local/share/kazeta");
    let base_dir_str = base_dir.to_str().unwrap();

    // Find the disk that contains our base directory
    let internal_disk = disks.iter()
    .find(|disk| {
        let mount_point = disk.mount_point().to_str().unwrap();
        base_dir_str.starts_with(mount_point)
    })
    .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "Could not find internal disk"))?;

    let free_space = (internal_disk.available_space() / 1024 / 1024) as u32; // Convert to MB
    devices.push(("internal".to_string(), free_space));

    // Add external drives
    let base_ext = if Path::new("/media").read_dir().map(|mut d| d.next().is_none()).unwrap_or(true) {
        if Path::new(&format!("/run/media/{}", whoami::username())).exists() {
            format!("/run/media/{}", whoami::username())
        } else {
            "/run/media".to_string()
        }
    } else {
        "/media".to_string()
    };

    // Find all disks mounted under the external base directory
    for disk in disks.iter() {
        let mount_point = disk.mount_point().to_str().unwrap();
        if mount_point.starts_with(&base_ext) {
            let name = mount_point.split('/').last().unwrap().to_string();
            if name == "frzr_efi" {
                // ignore internal frzr partition
                continue;
            }
            let free_space = (disk.available_space() / 1024 / 1024) as u32; // Convert to MB
            devices.push((name, free_space));
        }
    }

    Ok(devices)
}

pub fn has_save_dir(drive_name: &str) -> bool {
    if drive_name == "internal" {
        return true;
    }

    let save_dir = get_save_dir_from_drive_name(drive_name);
    Path::new(&save_dir).exists()
}

// [UPDATED] Logic now checks for both kzi and kzp
pub fn is_cart(drive_name: &str) -> bool {
    if drive_name == "internal" {
        return false;
    }

    let save_dir = get_save_dir_from_drive_name(drive_name);
    let mount_point: String = Path::new(&save_dir).parent().unwrap().parent().unwrap().display().to_string();

    if let Ok(files) = find_files_by_extension(mount_point, &["kzi", "kzp"], 1, true) {
        if files.len() > 0 {
            return true;
        }
    }

    false
}

// [UPDATED] Logic now checks for both kzi and kzp
pub fn is_cart_connected() -> bool {
    // In dev mode, check ~/kazeta-games first
    if DEV_MODE {
        if let Some(dev_games_dir) = dirs::home_dir().map(|h| h.join("kazeta-games")).filter(|p| p.exists()) {
            if let Ok(files) = find_files_by_extension(&dev_games_dir, &["kzi", "kzp"], 2, true) {
                if files.len() > 0 {
                    return true;
                }
            }
        }
    }

    // Check production location
    if let Ok(files) = find_files_by_extension("/run/media", &["kzi", "kzp"], 2, true) {
        if files.len() > 0 {
            return true;
        }
    }

    false
}

pub fn get_save_details(drive_name: &str) -> io::Result<Vec<(String, String, String)>> {
    let save_dir = get_save_dir_from_drive_name(drive_name);
    let cache_dir = get_cache_dir_from_drive_name(drive_name);
    eprintln!("Getting save details from directory: {}", save_dir);
    let mut details = Vec::new();

    for entry in fs::read_dir(save_dir)? {
        let entry = entry?;
        let path = entry.path();
        let file_name = path.file_name()
        .and_then(|n| n.to_str())
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "Invalid filename"))?;

        // Remove .tar extension if present
        let cart_id = if file_name.ends_with(".tar") {
            &file_name[..file_name.len() - 4]
        } else {
            file_name
        };

        let metadata_path = Path::new(&cache_dir).join(cart_id).join("metadata.kzi");
        let name = get_attribute(&metadata_path, "Name").unwrap_or_else(|e| {
            eprintln!("Failed to read metadata for {}: {}", cart_id, e);
            String::new()
        });
        let icon = format!("{}/{}/icon.png", cache_dir, cart_id);

        details.push((cart_id.to_string(), name, icon));
    }

    // Sort details alphabetically by name, fallback to cart_id if name is empty
    details.sort_by(|a, b| {
        let name_a = if a.1.is_empty() { &a.0 } else { &a.1 };
        let name_b = if b.1.is_empty() { &b.0 } else { &b.1 };
        name_a.to_lowercase().cmp(&name_b.to_lowercase())
    });

    eprintln!("Found {} save details", details.len());
    Ok(details)
}

pub fn delete_save(cart_id: &str, from_drive: &str) -> Result<(), SaveError> {
    let from_dir = get_save_dir_from_drive_name(from_drive);
    let from_cache = get_cache_dir_from_drive_name(from_drive);

    // Check if save exists
    let save_path = Path::new(&from_dir).join(cart_id);
    let save_path_tar = Path::new(&from_dir).join(format!("{}.tar", cart_id));
    if !save_path.exists() && !save_path_tar.exists() {
        //return Err(format!("Save file for {} does not exist on '{}' drive", cart_id, from_drive));
        return Err(SaveError::Message(format!("Save file for {} does not exist on '{}' drive", cart_id, from_drive)));
    }

    // Delete save file
    if from_drive == "internal" {
        //fs::remove_dir_all(save_path).map_err(|e| e.to_string())?;
        fs::remove_dir_all(save_path)?;
    } else {
        //fs::remove_file(save_path_tar).map_err(|e| e.to_string())?;
        fs::remove_file(save_path_tar)?;
    }

    // Delete cache
    let cache_path = Path::new(&from_cache).join(cart_id);
    if cache_path.exists() {
        //fs::remove_dir_all(cache_path).map_err(|e| e.to_string())?;
        fs::remove_dir_all(cache_path)?;
    }

    Ok(())
}

pub fn copy_save(cart_id: &str, from_drive: &str, to_drive: &str, progress: Arc<AtomicU16>) -> Result<(), SaveError> {
    let from_dir = get_save_dir_from_drive_name(from_drive);
    let to_dir = get_save_dir_from_drive_name(to_drive);
    let from_cache = get_cache_dir_from_drive_name(from_drive);
    let to_cache = get_cache_dir_from_drive_name(to_drive);

    if from_drive == to_drive {
        //return Err("Cannot copy to same location".to_string());
        return Err(SaveError::Message("Cannot copy to same location".to_string()));
    }

    // Check if source save exists
    let from_path = Path::new(&from_dir).join(cart_id);
    let from_path_tar = Path::new(&from_dir).join(format!("{}.tar", cart_id));
    if !from_path.exists() && !from_path_tar.exists() {
        //return Err(format!("Save file for {} does not exist on '{}' drive", cart_id, from_drive));
        return Err(SaveError::Message(format!("Save file for {} does not exist on '{}' drive", cart_id, from_drive)));
    }

    // Check if destination save already exists
    let to_path = Path::new(&to_dir).join(cart_id);
    let to_path_tar = Path::new(&to_dir).join(format!("{}.tar", cart_id));
    if to_path.exists() || to_path_tar.exists() {
        return Err(SaveError::Message(format!("Save file for {} already exists on '{}'", cart_id, to_drive)));
    }

    // Create destination directories
    fs::create_dir_all(&to_dir)?;
    fs::create_dir_all(&to_cache)?;

    // Copy save data
    let result = if from_drive == "internal" {
        // Internal to external: create tar archive
        eprintln!("Starting internal to external copy for {}", cart_id);
        let file = fs::File::create(&to_path_tar).map_err(|e| format!("Failed to create destination file: {}", e))?;
        let mut builder = Builder::new(file);

        // Calculate total size for progress reporting
        let mut total_size = 0;
        for entry in walkdir::WalkDir::new(&from_path)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| {
                let path = e.path();
                // Skip excluded directories and their contents
                !should_exclude_path(path) &&
                path.is_file()
            }) {
                total_size += entry.metadata().map_err(|e| format!("Failed to get metadata: {}", e))?.len();
            }

            eprintln!("Total size to archive: {} bytes", total_size);
            if total_size == 0 {
                return Err(SaveError::Message("No files found to archive".to_string()));
            }

            // Add the entire directory to the archive, excluding ignored directories
            let mut current_size = 0;
            for entry in walkdir::WalkDir::new(&from_path)
                .into_iter()
                .filter_map(|e| e.ok())
                .filter(|e| {
                    let path = e.path();
                    // Skip excluded directories and their contents
                    !should_exclude_path(path) &&
                    path.is_file()
                }) {
                    let path = entry.path();
                    // Get the relative path from the source directory
                    let name = path.strip_prefix(&from_path)
                    .map_err(|e| format!("Failed to get relative path: {}", e))?
                    .to_str()
                    .ok_or_else(|| "Invalid path encoding".to_string())?;

                    let file_size = entry.metadata().map_err(|e| format!("Failed to get file metadata: {}", e))?.len();
                    eprintln!("Adding file to archive: {} ({} bytes)", name, file_size);

                    let mut file = fs::File::open(path).map_err(|e| format!("Failed to open source file: {}", e))?;

                    // Create a new header with the correct path
                    let mut header = tar::Header::new_gnu();
                    header.set_path(name).map_err(|e| format!("Failed to set path in header: {}", e))?;
                    header.set_size(file_size);
                    header.set_cksum();

                    // Write the header and file contents
                    builder.append(&header, &mut file).map_err(|e| format!("Failed to append file to archive: {}", e))?;
                    sync_to_disk();

                    current_size += file_size;
                    progress.store((current_size * 100 / total_size) as u16, Ordering::SeqCst);
                }

                eprintln!("Finished creating archive, final size: {} bytes", current_size);
                if current_size == 0 {
                    return Err(SaveError::Message("No files were added to the archive".to_string()));
                }

                builder.finish().map_err(|e| format!("Failed to finish archive: {}", e))?;
                sync_to_disk();

                // Verify the archive was created and has content
                let archive_size = fs::metadata(&to_path_tar).map_err(|e| format!("Failed to get archive metadata: {}", e))?.len();
                eprintln!("Archive file size: {} bytes", archive_size);
                if archive_size == 0 {
                    return Err(SaveError::Message("Created archive is empty".to_string()));
                }

                Ok(())
    } else if to_drive == "internal" {
        // External to internal: extract tar archive
        eprintln!("Starting external to internal copy for {}", cart_id);
        fs::create_dir_all(&to_path).map_err(|e| format!("Failed to create destination directory: {}", e))?;

        let file = fs::File::open(&from_path_tar).map_err(|e| format!("Failed to open source archive: {}", e))?;
        let file_size = file.metadata().map_err(|e| format!("Failed to get archive metadata: {}", e))?.len();
        eprintln!("Archive size: {} bytes", file_size);

        let mut archive = Archive::new(file);
        let mut current_size = 0;

        for entry in archive.entries().map_err(|e| format!("Failed to read archive entries: {}", e))? {
            let mut entry = entry.map_err(|e| format!("Failed to read archive entry: {}", e))?;
            let path = entry.path().map_err(|e| format!("Failed to get entry path: {}", e))?;
            let entry_size = entry.header().size().unwrap_or(0);
            eprintln!("Extracting: {} ({} bytes)", path.display(), entry_size);

            // Ensure the parent directory exists
            if let Some(parent) = path.parent() {
                fs::create_dir_all(to_path.join(parent))
                .map_err(|e| format!("Failed to create parent directory: {}", e))?;
            }

            // Extract the file
            entry.unpack_in(&to_path)
            .map_err(|e| format!("Failed to extract file: {}", e))?;

            current_size += entry_size;
            progress.store((current_size * 100 / file_size) as u16, Ordering::SeqCst);
        }

        // Verify extraction
        let mut extracted_size = 0;
        for entry in walkdir::WalkDir::new(&to_path)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.path().is_file()) {
                extracted_size += entry.metadata()
                .map_err(|e| format!("Failed to get extracted file metadata: {}", e))?
                .len();
            }
            eprintln!("Total extracted size: {} bytes", extracted_size);

        if extracted_size == 0 {
            return Err(SaveError::Message("No files were extracted from the archive".to_string()));
        }

        Ok(())
    } else {
        // External to external: direct copy with progress
        let file_size = fs::metadata(&from_path_tar)?.len();
        let mut source = fs::File::open(&from_path_tar)?;
        let mut dest = fs::File::create(&to_path_tar)?;

        let mut buffer = [0; 8192];
        let mut current_size = 0;
        loop {
            let bytes_read = source.read(&mut buffer)?;
            if bytes_read == 0 {
                break;
            }
            dest.write_all(&buffer[..bytes_read])?;
            sync_to_disk();

            current_size += bytes_read as u64;
            progress.store((current_size * 100 / file_size) as u16, Ordering::SeqCst);
        }
        Ok(())
    };

    // If the main copy operation failed, clean up and return error
    if let Err(e) = result {
        // Clean up by removing the top-level directories
        if to_drive == "internal" {
            fs::remove_dir_all(&to_path).ok();
        } else {
            fs::remove_file(&to_path_tar).ok();
        }
        fs::remove_dir_all(Path::new(&to_cache).join(cart_id)).ok();
        return Err(e);
    }

    // Copy cache files
    let to_cache_path = Path::new(&to_cache).join(cart_id);
    fs::remove_dir_all(&to_cache_path).ok(); // Ignore errors if directory doesn't exist
    fs::create_dir_all(&to_cache_path)?;

    // Copy metadata.kzi if it exists
    let from_metadata = Path::new(&from_cache).join(cart_id).join("metadata.kzi");
    let to_metadata = to_cache_path.join("metadata.kzi");
    if from_metadata.exists() {
        fs::copy(&from_metadata, &to_metadata)?;
    }

    // Copy icon.png if it exists
    let from_icon = Path::new(&from_cache).join(cart_id).join("icon.png");
    let to_icon = to_cache_path.join("icon.png");
    if from_icon.exists() {
        fs::copy(&from_icon, &to_icon)?;
    }

    sync_to_disk();
    Ok(())
}

/// Calculate total playtime for a game from its .kazeta/var/playtime.log file
/// Returns playtime in hours with one decimal place
pub fn calculate_playtime(cart_id: &str, drive_name: &str) -> f32 {
    println!("Calculating playtime for {} on {}", cart_id, drive_name);
    let save_dir = get_save_dir_from_drive_name(drive_name);

    // Check if this is a tar file (external drive) or directory (internal drive)
    let tar_path = Path::new(&save_dir).join(format!("{}.tar", cart_id));
    let dir_path = Path::new(&save_dir).join(cart_id);

    if tar_path.exists() {
        // External drive: read from tar archive
        calculate_playtime_from_tar(&tar_path, cart_id)
    } else if dir_path.exists() {
        // Internal drive: read from directory
        calculate_playtime_from_dir(&dir_path, cart_id)
    } else {
        // Neither exists
        0.0
    }
}

/// Calculate save data size for a game (lazy calculation)
/// Returns size in MB with one decimal place
pub fn calculate_save_size(cart_id: &str, drive_name: &str) -> f32 {
    println!("Calculating save size for {} on {}", cart_id, drive_name);
    let save_dir = get_save_dir_from_drive_name(drive_name);

    // Check if this is a tar file (external drive) or directory (internal drive)
    let tar_path = Path::new(&save_dir).join(format!("{}.tar", cart_id));
    let dir_path = Path::new(&save_dir).join(cart_id);

    let size_bytes = if tar_path.exists() {
        // External drive: get tar file size
        calculate_size_from_tar(&tar_path)
    } else if dir_path.exists() {
        // Internal drive: calculate directory size
        calculate_size_from_dir(&dir_path)
    } else {
        // Neither exists
        return 0.0;
    };

    // Convert to MB with one decimal place, rounding up to nearest 0.1 MB if non-zero
    let size_mb = size_bytes as f64 / 1024.0 / 1024.0;
    if size_mb > 0.0 {
        ((size_mb * 10.0).ceil() / 10.0) as f32
    } else {
        0.0
    }
}

/// Setup RetroAchievements for a game launch (called from launch_game_with_options)
fn setup_retroachievements_for_launch(cart_info: &CartInfo, kzi_path: &Path) {
    // Check if kazeta-ra is available
    if Command::new("kazeta-ra").arg("status").output().is_err() {
        return;
    }

    // Check if RA is configured and enabled
    let status_output = match Command::new("kazeta-ra").arg("status").output() {
        Ok(output) => output,
        Err(_) => return,
    };

    let status_str = String::from_utf8_lossy(&status_output.stdout);
    if !status_str.contains("\"enabled\":true") && !status_str.contains("\"enabled\": true") {
        return;
    }

    // Get ROM path - for .kzi files, we need to extract first
    let rom_path = if kzi_path.extension().and_then(|e| e.to_str()) == Some("kzp") {
        // For .kzp, we can't easily get ROM path - skip RA setup
        return;
    } else {
        // For .kzi, extract to get ROM path
        let extract_dir = if DEV_MODE {
            get_user_data_dir().unwrap().join("kzi-cache").join(&cart_info.id)
        } else {
            PathBuf::from("/tmp").join("kazeta-kzi").join(&cart_info.id)
        };

        // Check if already extracted
        let rom_path = extract_dir.join(&cart_info.exec);
        if rom_path.exists() {
            Some(rom_path)
        } else {
            // Try to extract (best effort)
            if let Ok(file) = fs::File::open(kzi_path) {
                let decoder = GzDecoder::new(file);
                let mut archive = Archive::new(decoder);
                
                if fs::create_dir_all(&extract_dir).is_ok() {
                    if archive.unpack(&extract_dir).is_ok() {
                        let rom_path = extract_dir.join(&cart_info.exec);
                        if rom_path.exists() {
                            Some(rom_path)
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                } else {
                    None
                }
            } else {
                None
            }
        }
    };

    if let Some(rom_path) = rom_path {
        if rom_path.exists() {
            println!("[RA] Setting up RetroAchievements for: {}", rom_path.display());

            // Call kazeta-ra game-start (run in background)
            let rom_path_str = rom_path.to_string_lossy().to_string();
            std::thread::spawn(move || {
                let _ = Command::new("kazeta-ra")
                    .arg("game-start")
                    .arg("--path")
                    .arg(&rom_path_str)
                    .arg("")
                    .output();
            });

            // Also send achievement list to overlay (run in background)
            let rom_path_str2 = rom_path.to_string_lossy().to_string();
            std::thread::spawn(move || {
                std::thread::sleep(std::time::Duration::from_millis(500));
                let _ = Command::new("kazeta-ra")
                    .arg("send-achievements-to-overlay")
                    .arg("--path")
                    .arg(&rom_path_str2)
                    .output();
            });
        }
    }
}
