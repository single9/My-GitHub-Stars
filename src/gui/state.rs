use std::path::PathBuf;

use crate::config::Config;
use crate::storage::{CategoryRow, RepoRow};

// ── Screen ────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub enum GuiScreen {
    Setup,
    Login,
    Home,
    Browse,
    Search,
    AiSearch,
    Settings,
    Syncing,
}

// ── SyncStatus ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub enum SyncStatus {
    Idle,
    Running,
    Done,
    Failed,
}

// ── LogEntry ──────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct LogEntry {
    pub text: String,
    /// CSS color string (must be a static literal, e.g. "#3fb950")
    pub color: &'static str,
}

// ── GuiAppState ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct GuiAppState {
    pub screen: GuiScreen,
    pub config: Config,
    pub db_path: PathBuf,

    // Stats
    pub total_repos: i64,
    pub total_categories: i64,

    // Browse
    pub categories: Vec<CategoryRow>,
    pub displayed_repos: Vec<RepoRow>,
    pub selected_category: Option<usize>,
    pub selected_repo: Option<usize>,
    pub category_filter: String,

    // Search
    pub search_query: String,
    pub search_results: Vec<RepoRow>,

    // AI Search
    pub ai_query: String,
    pub ai_results: Vec<RepoRow>,
    pub ai_loading: bool,
    pub ai_error: Option<String>,

    // Auth
    pub device_user_code: Option<String>,
    pub device_verification_uri: Option<String>,
    pub device_code: Option<String>,
    pub device_poll_interval_secs: u64,
    pub auth_error: Option<String>,

    // Sync
    pub sync_log: Vec<LogEntry>,
    pub sync_status: SyncStatus,
    pub bg_syncing: bool,

    // Setup
    pub setup_input: String,

    // Settings
    pub settings_editing_key: bool,
    pub settings_key_input: String,
    pub settings_editing_field: String,
    pub settings_model_picking: bool,
    pub settings_model_cursor: usize,
}

impl GuiAppState {
    pub fn new(config: Config, db_path: PathBuf) -> Self {
        let screen = if config.is_authenticated() {
            GuiScreen::Home
        } else if config.client_id().is_none() {
            GuiScreen::Setup
        } else {
            GuiScreen::Login
        };

        Self {
            screen,
            config,
            db_path,
            total_repos: 0,
            total_categories: 0,
            categories: Vec::new(),
            displayed_repos: Vec::new(),
            selected_category: None,
            selected_repo: None,
            category_filter: String::new(),
            search_query: String::new(),
            search_results: Vec::new(),
            ai_query: String::new(),
            ai_results: Vec::new(),
            ai_loading: false,
            ai_error: None,
            device_user_code: None,
            device_verification_uri: None,
            device_code: None,
            device_poll_interval_secs: 5,
            auth_error: None,
            sync_log: Vec::new(),
            sync_status: SyncStatus::Idle,
            bg_syncing: false,
            setup_input: String::new(),
            settings_editing_key: false,
            settings_key_input: String::new(),
            settings_editing_field: String::new(),
            settings_model_picking: false,
            settings_model_cursor: 0,
        }
    }
}
