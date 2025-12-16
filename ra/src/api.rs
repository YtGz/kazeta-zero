use anyhow::{bail, Context, Result};
use crate::auth::Credentials;
use crate::types::*;
use serde::Deserialize;

const RA_API_BASE: &str = "https://retroachievements.org/API";

/// RetroAchievements API client
pub struct RAClient {
    client: reqwest::blocking::Client,
    credentials: Credentials,
}

impl RAClient {
    pub fn new(credentials: Credentials) -> Self {
        let client = reqwest::blocking::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .expect("Failed to create HTTP client");

        Self { client, credentials }
    }

    /// Get user summary (profile info)
    pub fn get_user_summary(&self) -> Result<UserSummary> {
        let url = format!(
            "{}/API_GetUserSummary.php?u={}&y={}&g=5&a=5",
            RA_API_BASE, self.credentials.username, self.credentials.api_key
        );

        let response = self.client.get(&url)
            .send()
            .context("Failed to send request to RA API")?;

        if !response.status().is_success() {
            bail!("RA API returned error: {}", response.status());
        }

        let summary: UserSummary = response.json()
            .context("Failed to parse user summary")?;

        Ok(summary)
    }

    /// Get game ID from ROM hash
    pub fn get_game_id(&self, hash: &str, console_id: ConsoleId) -> Result<Option<u32>> {
        let url = format!(
            "{}/API_GetGameInfoExtended.php?m={}&y={}",
            RA_API_BASE, hash, self.credentials.api_key
        );

        let response = self.client.get(&url)
            .send()
            .context("Failed to send request to RA API")?;

        if !response.status().is_success() {
            // May return 404 for unknown games
            if response.status() == reqwest::StatusCode::NOT_FOUND {
                return Ok(None);
            }
            bail!("RA API returned error: {}", response.status());
        }

        let text = response.text()?;
        
        // RA API returns empty object {} or error for unknown hash
        if text == "{}" || text.is_empty() || text.contains("\"ID\":0") {
            return Ok(None);
        }

        let lookup: GameInfoAndProgress = serde_json::from_str(&text)
            .context("Failed to parse game lookup response")?;

        Ok(Some(lookup.id))
    }

    /// Get game info and user's achievement progress
    pub fn get_game_info_and_progress(&self, game_id: u32) -> Result<GameInfoAndProgress> {
        let url = format!(
            "{}/API_GetGameInfoAndUserProgress.php?g={}&u={}&y={}",
            RA_API_BASE, game_id, self.credentials.username, self.credentials.api_key
        );

        let response = self.client.get(&url)
            .send()
            .context("Failed to send request to RA API")?;

        if !response.status().is_success() {
            bail!("RA API returned error: {}", response.status());
        }

        let info: GameInfoAndProgress = response.json()
            .context("Failed to parse game info")?;

        Ok(info)
    }

    /// Award an achievement (unlock)
    /// Note: This requires a session token, not the web API key
    pub fn award_achievement(&self, achievement_id: u32, hardcore: bool) -> Result<AwardAchievementResponse> {
        let token = self.credentials.token.as_ref()
            .context("No session token available. Login required.")?;

        let url = format!(
            "{}/API_AwardAchievement.php?u={}&t={}&a={}&h={}",
            RA_API_BASE,
            self.credentials.username,
            token,
            achievement_id,
            if hardcore { 1 } else { 0 }
        );

        let response = self.client.post(&url)
            .send()
            .context("Failed to send award request to RA API")?;

        if !response.status().is_success() {
            bail!("RA API returned error: {}", response.status());
        }

        let result: AwardAchievementResponse = response.json()
            .context("Failed to parse award response")?;

        Ok(result)
    }

    /// Login to get a session token (required for awarding achievements)
    /// Note: This uses the user's password, not API key
    pub fn login(&self, password: &str) -> Result<String> {
        let url = format!(
            "{}/API_Login.php?u={}&p={}",
            RA_API_BASE, self.credentials.username, password
        );

        let response = self.client.post(&url)
            .send()
            .context("Failed to send login request")?;

        if !response.status().is_success() {
            bail!("Login failed: {}", response.status());
        }

        #[derive(Deserialize)]
        struct LoginResponse {
            #[serde(rename = "Success")]
            success: bool,
            #[serde(rename = "Token")]
            token: Option<String>,
            #[serde(rename = "Error")]
            error: Option<String>,
        }

        let login: LoginResponse = response.json()
            .context("Failed to parse login response")?;

        if !login.success {
            bail!("Login failed: {}", login.error.unwrap_or_default());
        }

        login.token.context("No token in login response")
    }

    /// Get list of games for a console
    pub fn get_game_list(&self, console_id: ConsoleId) -> Result<Vec<GameListEntry>> {
        let url = format!(
            "{}/API_GetGameList.php?c={}&y={}",
            RA_API_BASE, console_id.as_u32(), self.credentials.api_key
        );

        let response = self.client.get(&url)
            .send()
            .context("Failed to send request to RA API")?;

        if !response.status().is_success() {
            bail!("RA API returned error: {}", response.status());
        }

        let games: Vec<GameListEntry> = response.json()
            .context("Failed to parse game list")?;

        Ok(games)
    }

    /// Verify credentials are valid
    pub fn verify_credentials(&self) -> Result<bool> {
        match self.get_user_summary() {
            Ok(_) => Ok(true),
            Err(_) => Ok(false),
        }
    }

    /// Get the username
    pub fn username(&self) -> &str {
        &self.credentials.username
    }

    /// Check if hardcore mode is enabled
    pub fn is_hardcore(&self) -> bool {
        self.credentials.hardcore
    }
}

/// Async RetroAchievements API client (non-blocking)
pub struct AsyncRAClient {
    client: reqwest::Client,
    credentials: Credentials,
}

impl AsyncRAClient {
    pub fn new(credentials: Credentials) -> Self {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .expect("Failed to create HTTP client");

        Self { client, credentials }
    }

    /// Get user summary (profile info)
    pub async fn get_user_summary(&self) -> Result<UserSummary> {
        let url = format!(
            "{}/API_GetUserSummary.php?u={}&y={}&g=5&a=5",
            RA_API_BASE, self.credentials.username, self.credentials.api_key
        );

        let response = self.client.get(&url)
            .send()
            .await
            .context("Failed to send request to RA API")?;

        if !response.status().is_success() {
            bail!("RA API returned error: {}", response.status());
        }

        let summary: UserSummary = response.json()
            .await
            .context("Failed to parse user summary")?;

        Ok(summary)
    }

    /// Get game ID from ROM hash
    pub async fn get_game_id(&self, hash: &str, _console_id: ConsoleId) -> Result<Option<u32>> {
        let url = format!(
            "{}/API_GetGameInfoExtended.php?m={}&y={}",
            RA_API_BASE, hash, self.credentials.api_key
        );

        let response = self.client.get(&url)
            .send()
            .await
            .context("Failed to send request to RA API")?;

        if !response.status().is_success() {
            // May return 404 for unknown games
            if response.status() == reqwest::StatusCode::NOT_FOUND {
                return Ok(None);
            }
            bail!("RA API returned error: {}", response.status());
        }

        let text = response.text().await?;

        // RA API returns empty object {} or error for unknown hash
        if text == "{}" || text.is_empty() || text.contains("\"ID\":0") {
            return Ok(None);
        }

        let lookup: GameInfoAndProgress = serde_json::from_str(&text)
            .context("Failed to parse game lookup response")?;

        Ok(Some(lookup.id))
    }

    /// Get game info and user's achievement progress
    pub async fn get_game_info_and_progress(&self, game_id: u32) -> Result<GameInfoAndProgress> {
        let url = format!(
            "{}/API_GetGameInfoAndUserProgress.php?g={}&u={}&y={}",
            RA_API_BASE, game_id, self.credentials.username, self.credentials.api_key
        );

        let response = self.client.get(&url)
            .send()
            .await
            .context("Failed to send request to RA API")?;

        if !response.status().is_success() {
            bail!("RA API returned error: {}", response.status());
        }

        let info: GameInfoAndProgress = response.json()
            .await
            .context("Failed to parse game info")?;

        Ok(info)
    }

    /// Award an achievement (unlock)
    /// Note: This requires a session token, not the web API key
    pub async fn award_achievement(&self, achievement_id: u32, hardcore: bool) -> Result<AwardAchievementResponse> {
        let token = self.credentials.token.as_ref()
            .context("No session token available. Login required.")?;

        let url = format!(
            "{}/API_AwardAchievement.php?u={}&t={}&a={}&h={}",
            RA_API_BASE,
            self.credentials.username,
            token,
            achievement_id,
            if hardcore { 1 } else { 0 }
        );

        let response = self.client.post(&url)
            .send()
            .await
            .context("Failed to send award request to RA API")?;

        if !response.status().is_success() {
            bail!("RA API returned error: {}", response.status());
        }

        let result: AwardAchievementResponse = response.json()
            .await
            .context("Failed to parse award response")?;

        Ok(result)
    }

    /// Login to get a session token (required for awarding achievements)
    /// Note: This uses the user's password, not API key
    pub async fn login(&self, password: &str) -> Result<String> {
        let url = format!(
            "{}/API_Login.php?u={}&p={}",
            RA_API_BASE, self.credentials.username, password
        );

        let response = self.client.post(&url)
            .send()
            .await
            .context("Failed to send login request")?;

        if !response.status().is_success() {
            bail!("Login failed: {}", response.status());
        }

        #[derive(Deserialize)]
        struct LoginResponse {
            #[serde(rename = "Success")]
            success: bool,
            #[serde(rename = "Token")]
            token: Option<String>,
            #[serde(rename = "Error")]
            error: Option<String>,
        }

        let login: LoginResponse = response.json()
            .await
            .context("Failed to parse login response")?;

        if !login.success {
            bail!("Login failed: {}", login.error.unwrap_or_default());
        }

        login.token.context("No token in login response")
    }

    /// Get list of games for a console
    pub async fn get_game_list(&self, console_id: ConsoleId) -> Result<Vec<GameListEntry>> {
        let url = format!(
            "{}/API_GetGameList.php?c={}&y={}",
            RA_API_BASE, console_id.as_u32(), self.credentials.api_key
        );

        let response = self.client.get(&url)
            .send()
            .await
            .context("Failed to send request to RA API")?;

        if !response.status().is_success() {
            bail!("RA API returned error: {}", response.status());
        }

        let games: Vec<GameListEntry> = response.json()
            .await
            .context("Failed to parse game list")?;

        Ok(games)
    }

    /// Verify credentials are valid
    pub async fn verify_credentials(&self) -> Result<bool> {
        match self.get_user_summary().await {
            Ok(_) => Ok(true),
            Err(_) => Ok(false),
        }
    }

    /// Get the username
    pub fn username(&self) -> &str {
        &self.credentials.username
    }

    /// Check if hardcore mode is enabled
    pub fn is_hardcore(&self) -> bool {
        self.credentials.hardcore
    }
}

