use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

/// RetroAchievements credentials
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Credentials {
    pub username: String,
    pub api_key: String,
    #[serde(default)]
    pub token: Option<String>,
    #[serde(default)]
    pub hardcore: bool,
    #[serde(default)]
    pub last_sync: Option<String>,
}

impl Credentials {
    pub fn new(username: String, api_key: String) -> Self {
        Self {
            username,
            api_key,
            token: None,
            hardcore: false,
            last_sync: None,
        }
    }
}

/// Manages RetroAchievements credentials storage
pub struct CredentialManager {
    credentials_path: PathBuf,
}

impl CredentialManager {
    pub fn new() -> Result<Self> {
        let data_dir = dirs::home_dir()
            .context("Could not find home directory")?
            .join(".local/share/kazeta-plus");

        fs::create_dir_all(&data_dir)
            .context("Failed to create kazeta data directory")?;

        let credentials_path = data_dir.join("ra_credentials.json");

        Ok(Self { credentials_path })
    }

    /// Check if credentials are stored
    pub fn has_credentials(&self) -> bool {
        self.credentials_path.exists()
    }

    /// Load stored credentials
    pub fn load(&self) -> Result<Option<Credentials>> {
        if !self.credentials_path.exists() {
            return Ok(None);
        }

        let content = fs::read_to_string(&self.credentials_path)
            .context("Failed to read credentials file")?;

        let creds: Credentials = serde_json::from_str(&content)
            .context("Failed to parse credentials")?;

        Ok(Some(creds))
    }

    /// Save credentials to storage
    pub fn save(&self, credentials: &Credentials) -> Result<()> {
        let json = serde_json::to_string_pretty(credentials)
            .context("Failed to serialize credentials")?;

        fs::write(&self.credentials_path, json)
            .context("Failed to write credentials file")?;

        // Set restrictive permissions (Unix only)
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = fs::metadata(&self.credentials_path)?.permissions();
            perms.set_mode(0o600);
            fs::set_permissions(&self.credentials_path, perms)?;
        }

        Ok(())
    }

    /// Delete stored credentials
    pub fn delete(&self) -> Result<()> {
        if self.credentials_path.exists() {
            fs::remove_file(&self.credentials_path)
                .context("Failed to delete credentials file")?;
        }
        Ok(())
    }

    /// Update the token in stored credentials
    pub fn update_token(&self, token: String) -> Result<()> {
        if let Some(mut creds) = self.load()? {
            creds.token = Some(token);
            creds.last_sync = Some(chrono::Utc::now().to_rfc3339());
            self.save(&creds)?;
        }
        Ok(())
    }

    /// Update hardcore mode setting
    pub fn set_hardcore(&self, hardcore: bool) -> Result<()> {
        if let Some(mut creds) = self.load()? {
            creds.hardcore = hardcore;
            self.save(&creds)?;
        }
        Ok(())
    }

    /// Get the path to the credentials file
    pub fn credentials_path(&self) -> &PathBuf {
        &self.credentials_path
    }
}

impl Default for CredentialManager {
    fn default() -> Self {
        Self::new().expect("Failed to create credential manager")
    }
}

