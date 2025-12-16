use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand};
use kazeta_ra::{
    api::RAClient,
    auth::{CredentialManager, Credentials},
    cache::RACache,
    hash::hash_rom,
    types::ConsoleId,
};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "kazeta-ra")]
#[command(about = "RetroAchievements integration for Kazeta+")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Login with RetroAchievements credentials
    Login {
        /// RetroAchievements username
        #[arg(short, long)]
        username: String,
        /// Web API key (from RA website Settings → Keys)
        #[arg(short, long)]
        api_key: String,
    },

    /// Logout and remove stored credentials
    Logout,

    /// Get stored credentials (for runtime wrappers)
    GetCredentials {
        /// Output format: json, env
        #[arg(short, long, default_value = "json")]
        format: String,
    },

    /// Set hardcore mode on/off
    SetHardcore {
        /// Enable hardcore mode
        #[arg(short, long)]
        enabled: bool,
    },

    /// Get user profile/summary
    Profile,

    /// Hash a ROM file for RA identification
    HashRom {
        /// Path to ROM file
        #[arg(short, long)]
        path: PathBuf,
        /// Console type (gba, nes, snes, etc.)
        #[arg(short, long)]
        console: String,
    },

    /// Get game info and achievements for a ROM
    GameInfo {
        /// ROM hash (use hash-rom to get this)
        #[arg(short = 'H', long)]
        hash: Option<String>,
        /// Path to ROM file (alternative to hash)
        #[arg(short, long)]
        path: Option<PathBuf>,
        /// Console type (required with --path)
        #[arg(short, long)]
        console: Option<String>,
    },

    /// Notify that a game has started (sends to overlay)
    GameStart {
        /// ROM hash
        #[arg(short = 'H', long)]
        hash: String,
        /// Console type
        #[arg(short, long)]
        console: String,
        /// Also notify the overlay daemon
        #[arg(long)]
        notify_overlay: bool,
    },

    /// Notify that an achievement was unlocked
    NotifyAchievement {
        /// Achievement ID
        #[arg(short, long)]
        id: u32,
        /// Achievement title (optional, for display)
        #[arg(short, long)]
        title: Option<String>,
    },

    /// Check if RA is configured and enabled
    Status,

    /// Clear local achievement cache
    ClearCache,

    /// Send achievement list to overlay daemon
    SendAchievementsToOverlay {
        /// ROM hash
        #[arg(short = 'H', long)]
        hash: String,
        /// Console type
        #[arg(short, long)]
        console: String,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Login { username, api_key } => cmd_login(username, api_key),
        Commands::Logout => cmd_logout(),
        Commands::GetCredentials { format } => cmd_get_credentials(&format),
        Commands::SetHardcore { enabled } => cmd_set_hardcore(enabled),
        Commands::Profile => cmd_profile(),
        Commands::HashRom { path, console } => cmd_hash_rom(&path, &console),
        Commands::GameInfo { hash, path, console } => cmd_game_info(hash, path, console),
        Commands::GameStart { hash, console, notify_overlay } => {
            cmd_game_start(&hash, &console, notify_overlay)
        }
        Commands::NotifyAchievement { id, title } => cmd_notify_achievement(id, title),
        Commands::Status => cmd_status(),
        Commands::ClearCache => cmd_clear_cache(),
        Commands::SendAchievementsToOverlay { hash, console } => {
            cmd_send_achievements_to_overlay(&hash, &console)
        }
    }
}

fn cmd_login(username: String, api_key: String) -> Result<()> {
    let cred_manager = CredentialManager::new()?;
    let credentials = Credentials::new(username.clone(), api_key);

    // Verify credentials work
    let client = RAClient::new(credentials.clone());
    if !client.verify_credentials()? {
        bail!("Invalid credentials. Please check your username and API key.");
    }

    // Save credentials
    cred_manager.save(&credentials)?;

    println!("✓ Logged in as: {}", username);
    println!("✓ Credentials saved to: {}", cred_manager.credentials_path().display());
    Ok(())
}

fn cmd_logout() -> Result<()> {
    let cred_manager = CredentialManager::new()?;
    cred_manager.delete()?;
    println!("✓ Logged out. Credentials removed.");
    Ok(())
}

fn cmd_get_credentials(format: &str) -> Result<()> {
    let cred_manager = CredentialManager::new()?;
    let credentials = cred_manager.load()?
        .context("No credentials stored. Run 'kazeta-ra login' first.")?;

    match format {
        "json" => {
            println!("{}", serde_json::to_string(&credentials)?);
        }
        "env" => {
            println!("RA_USERNAME={}", credentials.username);
            println!("RA_API_KEY={}", credentials.api_key);
            if let Some(token) = &credentials.token {
                println!("RA_TOKEN={}", token);
            }
            println!("RA_HARDCORE={}", if credentials.hardcore { "1" } else { "0" });
        }
        _ => {
            bail!("Unknown format: {}. Use 'json' or 'env'.", format);
        }
    }

    Ok(())
}

fn cmd_set_hardcore(enabled: bool) -> Result<()> {
    let cred_manager = CredentialManager::new()?;
    cred_manager.set_hardcore(enabled)?;
    println!("✓ Hardcore mode: {}", if enabled { "enabled" } else { "disabled" });
    Ok(())
}

fn cmd_profile() -> Result<()> {
    let cred_manager = CredentialManager::new()?;
    let credentials = cred_manager.load()?
        .context("No credentials stored. Run 'kazeta-ra login' first.")?;

    let client = RAClient::new(credentials);
    let summary = client.get_user_summary()?;

    println!("╔════════════════════════════════════════╗");
    println!("║  RetroAchievements Profile             ║");
    println!("╠════════════════════════════════════════╣");
    println!("║  User: {:<30} ║", summary.user);
    println!("║  Points: {:<28} ║", summary.total_points);
    println!("║  Softcore Points: {:<19} ║", summary.total_softcore_points);
    println!("║  True Points: {:<23} ║", summary.total_true_points);
    if let Some(rank) = summary.rank {
        println!("║  Rank: #{:<28} ║", rank);
    }
    println!("╚════════════════════════════════════════╝");

    if let Some(recent) = summary.recently_played {
        if !recent.is_empty() {
            println!("\nRecently Played:");
            for game in recent.iter().take(5) {
                println!("  • {} ({})", game.title, game.console_name);
            }
        }
    }

    Ok(())
}

fn cmd_hash_rom(path: &PathBuf, console: &str) -> Result<()> {
    let console_id = ConsoleId::from_str(console)
        .context(format!("Unknown console: {}", console))?;

    let hash = hash_rom(path, console_id)?;
    println!("{}", hash);
    Ok(())
}

fn cmd_game_info(hash: Option<String>, path: Option<PathBuf>, console: Option<String>) -> Result<()> {
    let cred_manager = CredentialManager::new()?;
    let credentials = cred_manager.load()?
        .context("No credentials stored. Run 'kazeta-ra login' first.")?;

    // Get hash either directly or by hashing the ROM
    let rom_hash = if let Some(h) = hash {
        h
    } else if let (Some(p), Some(c)) = (path, console) {
        let console_id = ConsoleId::from_str(&c)
            .context(format!("Unknown console: {}", c))?;
        hash_rom(&p, console_id)?
    } else {
        bail!("Either --hash or both --path and --console are required");
    };

    let client = RAClient::new(credentials);
    let cache = RACache::new()?;

    // Try to get game ID from hash
    let game_id = match client.get_game_id(&rom_hash, ConsoleId::GameBoyAdvance)? {
        Some(id) => id,
        None => {
            println!("No RetroAchievements found for this ROM.");
            println!("Hash: {}", rom_hash);
            return Ok(());
        }
    };

    // Get full game info
    let info = client.get_game_info_and_progress(game_id)?;

    // Cache it
    cache.cache_game(&rom_hash, &info)?;

    // Display
    println!("╔════════════════════════════════════════════════════════╗");
    println!("║  {} ", info.title);
    println!("╠════════════════════════════════════════════════════════╣");
    println!("║  Console: {}", info.console_name);
    println!("║  Game ID: {}", info.id);
    println!("║  Hash: {}", rom_hash);
    println!("║  Achievements: {}", info.num_achievements);
    
    if let Some(earned) = info.num_awarded_to_user {
        let total = info.num_achievements;
        let pct = if total > 0 { earned * 100 / total } else { 0 };
        println!("║  Progress: {}/{} ({}%)", earned, total, pct);
    }
    
    println!("╚════════════════════════════════════════════════════════╝");

    // List achievements
    if let Some(ref achievements) = info.achievements {
        println!("\nAchievements:");
        let mut sorted: Vec<_> = achievements.values().collect();
        sorted.sort_by_key(|a| a.display_order);

        for achievement in sorted {
            let status = if achievement.is_earned_hardcore() {
                "★"
            } else if achievement.is_earned() {
                "✓"
            } else {
                " "
            };
            println!(
                "  [{}] {} ({} pts) - {}",
                status, achievement.title, achievement.points, achievement.description
            );
        }
    }

    Ok(())
}

fn cmd_game_start(hash: &str, console: &str, notify_overlay: bool) -> Result<()> {
    let cred_manager = CredentialManager::new()?;
    let credentials = cred_manager.load()?
        .context("No credentials stored. Run 'kazeta-ra login' first.")?;

    let console_id = ConsoleId::from_str(console)
        .context(format!("Unknown console: {}", console))?;

    let client = RAClient::new(credentials);
    let cache = RACache::new()?;

    // Get game info
    let game_id = match client.get_game_id(hash, console_id)? {
        Some(id) => id,
        None => {
            println!("{{\"success\": false, \"error\": \"Game not found in RetroAchievements\"}}");
            return Ok(());
        }
    };

    let info = client.get_game_info_and_progress(game_id)?;
    cache.cache_game(hash, &info)?;

    let earned = info.num_awarded_to_user.unwrap_or(0);
    let total = info.num_achievements;

    // Output game info as JSON for runtime wrapper
    let output = serde_json::json!({
        "success": true,
        "game_id": info.id,
        "title": info.title,
        "console": info.console_name,
        "achievements_total": total,
        "achievements_earned": earned,
        "icon_url": info.image_icon,
    });
    println!("{}", serde_json::to_string(&output)?);

    // Notify overlay if requested
    if notify_overlay {
        notify_overlay_game_start(&info.title, earned, total)?;
    }

    Ok(())
}

fn cmd_notify_achievement(id: u32, title: Option<String>) -> Result<()> {
    let cache = RACache::new()?;

    // Try to get achievement info from cache
    let achievement_title = title.unwrap_or_else(|| format!("Achievement #{}", id));

    // Notify overlay
    notify_overlay_achievement(&achievement_title)?;

    // Mark in cache
    let _ = cache.mark_earned(id, false);

    println!("{{\"success\": true, \"achievement_id\": {}}}", id);
    Ok(())
}

fn cmd_status() -> Result<()> {
    let cred_manager = CredentialManager::new()?;

    if !cred_manager.has_credentials() {
        println!("{{\"enabled\": false, \"reason\": \"Not logged in\"}}");
        return Ok(());
    }

    let credentials = cred_manager.load()?
        .context("Failed to load credentials")?;

    let client = RAClient::new(credentials.clone());
    let valid = client.verify_credentials().unwrap_or(false);

    let output = serde_json::json!({
        "enabled": valid,
        "username": credentials.username,
        "hardcore": credentials.hardcore,
        "valid_credentials": valid,
    });
    println!("{}", serde_json::to_string(&output)?);

    Ok(())
}

fn cmd_clear_cache() -> Result<()> {
    let cache = RACache::new()?;
    cache.clear()?;
    println!("✓ Achievement cache cleared.");
    Ok(())
}

// Overlay notification helpers

fn notify_overlay_game_start(title: &str, earned: u32, total: u32) -> Result<()> {
    use std::io::Write;
    use std::os::unix::net::UnixStream;

    let socket_path = "/tmp/kazeta-overlay.sock";
    if !std::path::Path::new(socket_path).exists() {
        return Ok(()); // Overlay not running, skip
    }

    let message = serde_json::json!({
        "type": "ra_game_start",
        "game_title": title,
        "total_achievements": total,
        "earned_achievements": earned,
    });

    if let Ok(mut stream) = UnixStream::connect(socket_path) {
        let _ = writeln!(stream, "{}", message);
    }

    Ok(())
}

fn notify_overlay_achievement(title: &str) -> Result<()> {
    use std::io::Write;
    use std::os::unix::net::UnixStream;

    let socket_path = "/tmp/kazeta-overlay.sock";
    if !std::path::Path::new(socket_path).exists() {
        return Ok(()); // Overlay not running, skip
    }

    let message = serde_json::json!({
        "type": "show_toast",
        "message": format!("🏆 Achievement Unlocked: {}", title),
        "style": "success",
        "duration_ms": 5000,
    });

    if let Ok(mut stream) = UnixStream::connect(socket_path) {
        let _ = writeln!(stream, "{}", message);
    }

    Ok(())
}

fn cmd_send_achievements_to_overlay(hash: &str, console: &str) -> Result<()> {
    use std::io::Write;
    use std::os::unix::net::UnixStream;

    let cred_manager = CredentialManager::new()?;
    let credentials = cred_manager.load()?
        .context("No credentials stored. Run 'kazeta-ra login' first.")?;

    let console_id = ConsoleId::from_str(console)
        .context(format!("Unknown console: {}", console))?;

    let client = RAClient::new(credentials);
    let cache = RACache::new()?;

    // Get game ID from hash
    let game_id = match client.get_game_id(hash, console_id)? {
        Some(id) => id,
        None => {
            println!("{{\"success\": false, \"error\": \"Game not found\"}}");
            return Ok(());
        }
    };

    // Get full game info with achievements
    let info = client.get_game_info_and_progress(game_id)?;
    
    // Cache it
    cache.cache_game(hash, &info)?;

    // Build achievement list for overlay
    let achievements: Vec<serde_json::Value> = info.achievements
        .as_ref()
        .map(|achs| {
            let mut list: Vec<_> = achs.values()
                .map(|a| serde_json::json!({
                    "id": a.id,
                    "title": a.title,
                    "description": a.description,
                    "points": a.points,
                    "earned": a.date_earned.is_some() || a.date_earned_hardcore.is_some(),
                    "earned_hardcore": a.date_earned_hardcore.is_some(),
                }))
                .collect();
            // Sort by display order (using id as fallback)
            list.sort_by(|a, b| {
                let a_id = a["id"].as_u64().unwrap_or(0);
                let b_id = b["id"].as_u64().unwrap_or(0);
                a_id.cmp(&b_id)
            });
            list
        })
        .unwrap_or_default();

    // Send to overlay
    let socket_path = "/tmp/kazeta-overlay.sock";
    if !std::path::Path::new(socket_path).exists() {
        println!("{{\"success\": false, \"error\": \"Overlay not running\"}}");
        return Ok(());
    }

    let message = serde_json::json!({
        "type": "ra_achievement_list",
        "game_title": info.title,
        "game_hash": hash,
        "achievements": achievements,
    });

    if let Ok(mut stream) = UnixStream::connect(socket_path) {
        let _ = writeln!(stream, "{}", message);
        println!("{{\"success\": true, \"achievements_sent\": {}}}", achievements.len());
    } else {
        println!("{{\"success\": false, \"error\": \"Failed to connect to overlay\"}}");
    }

    Ok(())
}

