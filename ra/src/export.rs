use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand};
use std::path::PathBuf;

use kazeta_ra::api::RAClient;
use kazeta_ra::auth::Credentials;
use kazeta_ra::local_definitions::{LocalAchievement, LocalDefinitions};
use kazeta_ra::types::GameInfoAndProgress;

/// Fetch RetroAchievements definitions and export them to local files for
/// offline use. This is the only component that touches the internet or an
/// RA account. Run on the prep machine, then copy the output to the SD card.
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

        /// RetroAchievements web API key
        #[arg(long)]
        api_key: String,

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
            api_key,
            game_id,
            output_dir,
            download_badges,
        } => {
            let creds = Credentials::new(username, api_key);
            let client = RAClient::new(creds);

            println!("Fetching game info for game ID {}...", game_id);
            let info = client
                .get_game_info_extended(game_id)
                .context("Failed to fetch game info from RA API")?;

            if info.id == 0 {
                bail!("Game ID {} not found on RetroAchievements", game_id);
            }

            println!("Game: {} (console: {})", info.title, info.console_name);

            let defs = convert_to_local_definitions(&info);

            std::fs::create_dir_all(&output_dir).context("Failed to create output directory")?;

            let definitions_path = output_dir.join("achievements.json");
            defs.save(&definitions_path)?;
            println!("Wrote definitions to {:?}", definitions_path);
            println!("  {} achievements", defs.achievements.len());

            if download_badges {
                let badges_dir = output_dir.join("badges");
                std::fs::create_dir_all(&badges_dir)
                    .context("Failed to create badges directory")?;
                download_all_badges(&client, &defs, &badges_dir)?;
            }

            println!("Done!");
        }
        Commands::ListGames { username, api_key } => {
            let creds = Credentials::new(username, api_key);
            let client = RAClient::new(creds);

            use kazeta_ra::types::ConsoleId;
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

/// Convert the RA API response into the local definitions format.
fn convert_to_local_definitions(info: &GameInfoAndProgress) -> LocalDefinitions {
    let achievements = if let Some(ref achs) = info.achievements {
        achs.values()
            .map(|a| LocalAchievement {
                id: a.id,
                title: a.title.clone(),
                description: a.description.clone(),
                points: a.points,
                badge_name: a.badge_name.clone(),
                mem_addr: a.mem_addr.clone().unwrap_or_default(),
                achievement_type: a.achievement_type.clone(),
                display_order: a.display_order,
            })
            .collect()
    } else {
        vec![]
    };

    LocalDefinitions {
        game_id: info.id,
        game_title: info.title.clone(),
        console_id: info.console_id,
        console_name: info.console_name.clone(),
        icon_url: Some(info.image_icon.clone()),
        rich_presence_patch: String::new(),
        achievements,
        leaderboards: vec![],
    }
}

/// Download badge images for all achievements in the definitions set.
fn download_all_badges(
    _client: &RAClient,
    defs: &LocalDefinitions,
    badges_dir: &std::path::Path,
) -> Result<()> {
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

    // Download the game icon too
    if let Some(ref icon_url) = defs.icon_url {
        if !icon_url.is_empty() {
            let full_url = if icon_url.starts_with("http") {
                icon_url.clone()
            } else {
                format!("https://media.retroachievements.org{}", icon_url)
            };
            let icon_path = badges_dir.join("game_icon.png");
            if let Ok(resp) = http_client.get(&full_url).send() {
                if resp.status().is_success() {
                    if let Ok(bytes) = resp.bytes() {
                        let _ = std::fs::write(&icon_path, &bytes);
                    }
                }
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
