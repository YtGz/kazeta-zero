use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand};
use std::path::PathBuf;

use kazeta_ra::local_definitions::{LocalAchievement, LocalDefinitions};

/// Fetch RetroAchievements definitions and export them to local files for
/// offline use. This is the only component that touches the internet or an
/// RA account. Run on the prep machine, then copy the output to the SD card.
///
/// The Connect API requires your RA password (not the web API key) to fetch
/// achievement condition strings. Your password is sent over HTTPS to
/// retroachievements.org for a one-time login to obtain a session token.
#[derive(Parser)]
#[command(name = "kazeta-ra-export", version, about)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Fetch achievement definitions and badges for a game
    Fetch {
        /// RetroAchievements username
        #[arg(long)]
        username: String,

        /// RetroAchievements password (required for Connect API to fetch condition strings)
        #[arg(long)]
        password: String,

        /// RetroAchievements game ID (e.g. 7693 for Mario Kart: Double Dash)
        #[arg(long)]
        game_id: u32,

        /// Output directory (will be created if it doesn't exist)
        #[arg(long)]
        output_dir: PathBuf,

        /// Also download badge images to <output_dir>/badges/
        #[arg(long, default_value_t = true)]
        download_badges: bool,
    },

    /// List all GameCube games with achievement sets on RetroAchievements
    ListGames {
        /// RetroAchievements username
        #[arg(long)]
        username: String,

        /// RetroAchievements web API key
        #[arg(long)]
        api_key: String,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Fetch {
            username,
            password,
            game_id,
            output_dir,
            download_badges,
        } => {
            println!("Fetching game info for game ID {}...", game_id);

            let http_client = reqwest::blocking::Client::builder()
                .timeout(std::time::Duration::from_secs(30))
                .build()
                .context("Failed to create HTTP client")?;

            let connect_url = "https://retroachievements.org/dorequest.php";
            let user_agent = "kazeta-ra-export/1.0 (Linux) rcheevos/11.5";

            // Step 1: Login via Connect API to get a session token
            println!("  Logging in to Connect API...");
            let login_data = format!("r=login2&u={}&p={}", username, password);

            let login_resp = http_client
                .post(connect_url)
                .header("Content-Type", "application/x-www-form-urlencoded")
                .header("User-Agent", user_agent)
                .body(login_data)
                .send()
                .context("Failed to login to Connect API")?;

            if !login_resp.status().is_success() {
                bail!("Connect API login failed: {}", login_resp.status());
            }

            let login_json: serde_json::Value = login_resp
                .json()
                .context("Failed to parse login response")?;

            let success = login_json["Success"].as_bool().unwrap_or(false);
            if !success {
                let error = login_json["Error"].as_str().unwrap_or("unknown error");
                bail!("Connect API login failed: {}", error);
            }

            let token = login_json["Token"]
                .as_str()
                .context("No session token in login response")?;
            println!("  Logged in, session token obtained");

            // Step 2: Fetch game data (achievement definitions with condition strings)
            println!("  Fetching achievement definitions...");
            let fetch_data = format!("r=patch&u={}&t={}&g={}", username, token, game_id);

            let fetch_resp = http_client
                .post(connect_url)
                .header("Content-Type", "application/x-www-form-urlencoded")
                .header("User-Agent", user_agent)
                .body(fetch_data)
                .send()
                .context("Failed to fetch game data")?;

            if !fetch_resp.status().is_success() {
                bail!("Failed to fetch game data: {}", fetch_resp.status());
            }

            let json: serde_json::Value = fetch_resp
                .json()
                .context("Failed to parse game data response")?;

            let patch_data = &json["PatchData"];
            if patch_data.is_null() {
                bail!("No PatchData in response");
            }

            let ra_game_id = patch_data["ID"].as_u64().unwrap_or(0) as u32;
            let console_id = patch_data["ConsoleID"].as_u64().unwrap_or(0) as u32;
            let game_title = patch_data["Title"]
                .as_str()
                .unwrap_or("Unknown")
                .to_string();

            println!("Game: {} (console ID: {})", game_title, console_id);

            if ra_game_id == 0 {
                bail!("Game ID {} not found on RetroAchievements", game_id);
            }

            let achievements = patch_data["Achievements"]
                .as_array()
                .context("No Achievements array in response")?;

            let local_achievements: Vec<LocalAchievement> = achievements
                .iter()
                .enumerate()
                .filter(|(_, a)| {
                    // Skip the "Warning: Unknown Emulator" placeholder achievement
                    a["MemAddr"]
                        .as_str()
                        .map(|m| m != "1=1.300.")
                        .unwrap_or(true)
                })
                .map(|(i, a)| LocalAchievement {
                    id: a["ID"].as_u64().unwrap_or(0) as u32,
                    title: a["Title"].as_str().unwrap_or("").to_string(),
                    description: a["Description"].as_str().unwrap_or("").to_string(),
                    points: a["Points"].as_u64().unwrap_or(0) as u32,
                    badge_name: a["BadgeName"].as_str().unwrap_or("").to_string(),
                    mem_addr: a["MemAddr"].as_str().unwrap_or("").to_string(),
                    achievement_type: match a["Type"].as_str() {
                        Some("progression") => "progression".to_string(),
                        Some("missable") => "missable".to_string(),
                        Some("win") => "win".to_string(),
                        _ => "standard".to_string(),
                    },
                    display_order: (i + 1) as u32,
                })
                .collect();

            let defs = LocalDefinitions {
                game_id: ra_game_id,
                game_title,
                console_id,
                console_name: "Nintendo GameCube".to_string(),
                icon_url: None,
                rich_presence_patch: patch_data["RichPresencePatch"]
                    .as_str()
                    .unwrap_or("")
                    .to_string(),
                achievements: local_achievements,
                leaderboards: vec![],
            };

            std::fs::create_dir_all(&output_dir).context("Failed to create output directory")?;

            let definitions_path = output_dir.join("achievements.json");
            defs.save(&definitions_path)?;
            println!("Wrote definitions to {:?}", definitions_path);
            println!("  {} achievements", defs.achievements.len());

            if download_badges {
                let badges_dir = output_dir.join("badges");
                std::fs::create_dir_all(&badges_dir)
                    .context("Failed to create badges directory")?;
                download_all_badges(&defs, &badges_dir)?;
            }

            println!("Done!");
        }
        Commands::ListGames { username, api_key } => {
            use kazeta_ra::api::RAClient;
            use kazeta_ra::auth::Credentials;
            use kazeta_ra::types::ConsoleId;

            let creds = Credentials::new(username, api_key);
            let client = RAClient::new(creds);

            println!("Fetching GameCube game list...");
            let games = client
                .get_game_list(ConsoleId::GameCube)
                .context("Failed to fetch GameCube game list")?;

            if games.is_empty() {
                println!("No GameCube games found.");
            } else {
                println!(
                    "{:<10} {:<60} {:>5} {:>6}",
                    "Game ID", "Title", "Ach.", "Points"
                );
                println!("{}", "-".repeat(83));
                for game in games {
                    println!(
                        "{:<10} {:<60} {:>5} {:>6}",
                        game.id,
                        truncate(&game.title, 60),
                        game.num_achievements,
                        game.points,
                    );
                }
            }
        }
    }

    Ok(())
}

/// Download badge images for all achievements in the definitions set.
fn download_all_badges(defs: &LocalDefinitions, badges_dir: &std::path::Path) -> Result<()> {
    let http_client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .context("Failed to create HTTP client for badge downloads")?;

    let mut downloaded = 0;
    let mut failed = 0;

    for ach in &defs.achievements {
        let badge_url = format!(
            "https://media.retroachievements.org/Badge/{}.png",
            ach.badge_name
        );
        let badge_path = badges_dir.join(format!("{}.png", ach.badge_name));

        if badge_path.exists() {
            downloaded += 1;
            continue;
        }

        match http_client.get(&badge_url).send() {
            Ok(resp) if resp.status().is_success() => {
                let bytes = resp.bytes().context("Failed to read badge image")?;
                std::fs::write(&badge_path, &bytes).context("Failed to write badge image")?;
                downloaded += 1;
            }
            Ok(resp) => {
                eprintln!(
                    "  Warning: badge {} returned HTTP {}",
                    ach.badge_name,
                    resp.status()
                );
                failed += 1;
            }
            Err(e) => {
                eprintln!("  Warning: badge {} download failed: {}", ach.badge_name, e);
                failed += 1;
            }
        }
    }

    println!(
        "Badges: {} downloaded, {} failed, {} skipped",
        downloaded - failed,
        failed,
        defs.achievements.len() - downloaded
    );

    Ok(())
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        format!("{}...", &s[..max - 3])
    }
}
