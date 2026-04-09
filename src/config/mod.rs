use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub auto_update: bool,
    pub github_token: Option<String>,
    pub last_sync: Option<String>,
    /// GitHub OAuth App Client ID.
    /// Create one at: https://github.com/settings/developers
    /// → "New OAuth App" → enable "Device Flow"
    pub client_id: Option<String>,
    /// OpenAI-compatible API key for AI search
    pub openai_api_key: Option<String>,
    /// OpenAI-compatible base URL (default: https://api.openai.com/v1)
    pub openai_base_url: Option<String>,
    /// Model to use for AI search (default: gpt-4o-mini)
    pub openai_model: Option<String>,
    /// When true, use GitHub Copilot for AI search instead of a plain API key.
    /// The existing GitHub token is exchanged for a Copilot token automatically.
    pub use_copilot: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            auto_update: true,
            github_token: None,
            last_sync: None,
            client_id: None,
            openai_api_key: None,
            openai_base_url: None,
            openai_model: None,
            use_copilot: false,
        }
    }
}

impl Config {
    pub fn config_dir() -> Result<PathBuf> {
        let dir = dirs::config_dir()
            .context("Cannot find config directory")?
            .join("github-stars-pocket");
        std::fs::create_dir_all(&dir)?;
        Ok(dir)
    }

    pub fn config_path() -> Result<PathBuf> {
        Ok(Self::config_dir()?.join("config.toml"))
    }

    pub fn db_path() -> Result<PathBuf> {
        Ok(Self::config_dir()?.join("stars.db"))
    }

    pub fn load() -> Result<Self> {
        let path = Self::config_path()?;
        if !path.exists() {
            let config = Self::default();
            config.save()?;
            return Ok(config);
        }
        let content = std::fs::read_to_string(&path)
            .with_context(|| format!("Failed to read config from {:?}", path))?;
        let config: Self = toml::from_str(&content).context("Failed to parse config.toml")?;
        Ok(config)
    }

    pub fn save(&self) -> Result<()> {
        let path = Self::config_path()?;
        let content = toml::to_string_pretty(self).context("Failed to serialize config")?;
        std::fs::write(&path, content)
            .with_context(|| format!("Failed to write config to {:?}", path))?;
        Ok(())
    }

    pub fn is_authenticated(&self) -> bool {
        self.github_token
            .as_ref()
            .map(|t| !t.is_empty())
            .unwrap_or(false)
    }

    pub fn client_id(&self) -> Option<&str> {
        self.client_id.as_deref().filter(|s| !s.is_empty())
    }
}
