use anyhow::{Context, Result};
use rusqlite::{Connection, params};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    #[serde(default = "default_auto_update")]
    pub auto_update: bool,
    pub github_token: Option<String>,
    pub last_sync: Option<String>,
    /// GitHub OAuth App Client ID.
    /// Create one at: https://github.com/settings/developers
    /// → "New OAuth App" → enable "Device Flow"
    pub client_id: Option<String>,
    /// OpenAI-compatible API key for AI search
    #[serde(default)]
    pub openai_api_key: Option<String>,
    /// OpenAI-compatible base URL (default: https://api.openai.com/v1)
    #[serde(default)]
    pub openai_base_url: Option<String>,
    /// Model to use for AI search (default: gpt-4o-mini)
    #[serde(default)]
    pub openai_model: Option<String>,
    /// When true, use GitHub Copilot for AI search instead of a plain API key.
    /// The existing GitHub token is exchanged for a Copilot token automatically.
    #[serde(default)]
    pub use_copilot: bool,
    /// A GitHub token used to obtain a short-lived Copilot session token.
    /// Must come from an official GitHub app (e.g. `gh auth token`).
    /// If unset and use_copilot is true, the app will prompt for one.
    #[serde(default)]
    pub copilot_github_token: Option<String>,
}

fn default_auto_update() -> bool {
    true
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
            copilot_github_token: None,
        }
    }
}

impl Config {
    pub fn config_dir() -> Result<PathBuf> {
        let dir = dirs::config_dir()
            .context("Cannot find config directory")?
            .join("my-gh-stars");
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
        let db_path = Self::db_path()?;
        let conn = Connection::open(&db_path)
            .with_context(|| format!("Failed to open database at {:?}", db_path))?;
        conn.execute_batch(
            "PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;
             CREATE TABLE IF NOT EXISTS settings (key TEXT PRIMARY KEY, value TEXT);",
        )?;

        // Migration: if the settings table is empty and a legacy config.toml exists,
        // read it and persist its values into the DB.
        let has_rows: i64 = conn
            .query_row("SELECT COUNT(*) FROM settings", [], |r| r.get(0))
            .unwrap_or(0);
        if has_rows == 0 {
            let toml_path = Self::config_path()?;
            if toml_path.exists() {
                if let Ok(content) = std::fs::read_to_string(&toml_path) {
                    if let Ok(config) = toml::from_str::<Self>(&content) {
                        Self::write_to_conn(&conn, &config)?;
                        return Ok(config);
                    }
                }
            }
            // No prior settings anywhere — persist and return defaults.
            let config = Self::default();
            Self::write_to_conn(&conn, &config)?;
            return Ok(config);
        }

        Self::read_from_conn(&conn)
    }

    fn read_from_conn(conn: &Connection) -> Result<Self> {
        let mut config = Self::default();
        let mut stmt = conn.prepare("SELECT key, value FROM settings")?;
        let rows = stmt
            .query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, Option<String>>(1)?))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()
            .context("Failed to read settings rows")?;
        for (key, value) in rows {
            match key.as_str() {
                "auto_update" => config.auto_update = value.as_deref() == Some("true"),
                "github_token" => config.github_token = value,
                "last_sync" => config.last_sync = value,
                "client_id" => config.client_id = value,
                "openai_api_key" => config.openai_api_key = value,
                "openai_base_url" => config.openai_base_url = value,
                "openai_model" => config.openai_model = value,
                "use_copilot" => config.use_copilot = value.as_deref() == Some("true"),
                "copilot_github_token" => config.copilot_github_token = value,
                _ => {}
            }
        }
        Ok(config)
    }

    pub fn save(&self) -> Result<()> {
        let db_path = Self::db_path()?;
        let conn = Connection::open(&db_path)
            .with_context(|| format!("Failed to open database at {:?}", db_path))?;
        conn.execute_batch(
            "PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;
             CREATE TABLE IF NOT EXISTS settings (key TEXT PRIMARY KEY, value TEXT);",
        )?;
        Self::write_to_conn(&conn, self)
    }

    fn write_to_conn(conn: &Connection, config: &Self) -> Result<()> {
        let pairs: &[(&str, Option<&str>)] = &[
            (
                "auto_update",
                Some(if config.auto_update { "true" } else { "false" }),
            ),
            ("github_token", config.github_token.as_deref()),
            ("last_sync", config.last_sync.as_deref()),
            ("client_id", config.client_id.as_deref()),
            ("openai_api_key", config.openai_api_key.as_deref()),
            ("openai_base_url", config.openai_base_url.as_deref()),
            ("openai_model", config.openai_model.as_deref()),
            (
                "use_copilot",
                Some(if config.use_copilot { "true" } else { "false" }),
            ),
            (
                "copilot_github_token",
                config.copilot_github_token.as_deref(),
            ),
        ];
        for (key, value) in pairs {
            conn.execute(
                "INSERT INTO settings (key, value) VALUES (?1, ?2)
                 ON CONFLICT(key) DO UPDATE SET value = excluded.value",
                params![key, value],
            )?;
        }
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
