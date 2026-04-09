use anyhow::Result;
use chrono::Utc;
use crossterm::event::KeyCode;
use ratatui::{style::Color, widgets::ListState};
use std::time::{Duration, Instant};
use tokio::sync::{oneshot, watch};

use crate::{
    api::{ApiClient, StarredRepo},
    auth::{AuthClient, DeviceCodeResponse, PollResult},
    classifier::Classifier,
    config::Config,
    storage::{CategoryRow, Database, RepoRow},
    tui::events::{AppEvent, is_quit, poll_event},
};

// ── Sync log ──────────────────────────────────────────────────────────────────

#[derive(Clone)]
pub struct LogEntry {
    pub color: Color,
    pub message: String,
}

impl LogEntry {
    pub fn info(msg: impl Into<String>) -> Self {
        Self { color: Color::White, message: msg.into() }
    }
    pub fn ok(msg: impl Into<String>) -> Self {
        Self { color: Color::Green, message: msg.into() }
    }
    pub fn warn(msg: impl Into<String>) -> Self {
        Self { color: Color::Yellow, message: msg.into() }
    }
    pub fn err(msg: impl Into<String>) -> Self {
        Self { color: Color::Red, message: msg.into() }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Screen {
    Setup,
    Login,
    Home,
    Browse,
    Search,
    Settings,
    Syncing,
}

#[derive(Debug, Clone)]
pub enum SyncStatus {
    Idle,
    FetchingStars(usize),
    Classifying,
    Done(usize),
    Error(String),
}

#[derive(Debug, Clone, PartialEq)]
pub enum BrowsePane {
    Categories,
    Repos,
}

// ── App ───────────────────────────────────────────────────────────────────────

pub struct App {
    pub screen: Screen,
    pub config: Config,
    pub sync_status: SyncStatus,

    // Auth state
    pub device_user_code: Option<String>,
    pub device_verification_uri: Option<String>,
    device_code: Option<String>,
    /// Minimum seconds between polls (from GitHub, starts at 5, increases on slow_down)
    device_poll_interval_secs: u64,
    /// When we last polled GitHub for the token
    last_poll_at: Option<Instant>,

    // Setup (client_id entry)
    pub setup_input: String,

    // Browse state
    pub categories: Vec<CategoryRow>,
    pub displayed_repos: Vec<RepoRow>,
    pub selected_category: Option<usize>,
    pub selected_repo: Option<usize>,
    pub browse_pane: BrowsePane,
    pub category_list_state: ListState,
    pub repo_list_state: ListState,

    // Search state
    pub search_query: String,
    pub search_results: Vec<RepoRow>,

    // Stats
    pub total_repos: i64,
    pub total_categories: i64,

    // Misc
    pub tick_count: usize,
    pub should_quit: bool,

    /// Receives final fetch result (Ok/Err) from background task
    fetch_done_rx: Option<oneshot::Receiver<Result<Vec<StarredRepo>, String>>>,
    /// Receives live page count from background task (latest value, non-blocking)
    fetch_progress_rx: Option<watch::Receiver<usize>>,
    /// True when auto-update is running silently in the background (shown in Home footer)
    pub bg_syncing: bool,
    /// Accumulated log lines shown during sync
    pub sync_log: Vec<LogEntry>,
}

impl App {
    pub fn new(config: Config) -> Self {
        Self {
            screen: Screen::Home,
            config,
            sync_status: SyncStatus::Idle,
            device_user_code: None,
            device_verification_uri: None,
            device_code: None,
            device_poll_interval_secs: 5,
            last_poll_at: None,
            setup_input: String::new(),
            categories: Vec::new(),
            displayed_repos: Vec::new(),
            selected_category: None,
            selected_repo: None,
            browse_pane: BrowsePane::Categories,
            category_list_state: ListState::default(),
            repo_list_state: ListState::default(),
            search_query: String::new(),
            search_results: Vec::new(),
            total_repos: 0,
            total_categories: 0,
            tick_count: 0,
            should_quit: false,
            fetch_done_rx: None,
            fetch_progress_rx: None,
            bg_syncing: false,
            sync_log: Vec::new(),
        }
    }

    pub fn load_stats(&mut self, db: &Database) {
        self.total_repos = db.count_repos().unwrap_or(0);
        self.total_categories = db.count_categories().unwrap_or(0);
    }

    pub fn load_categories(&mut self, db: &Database) {
        self.categories = db.get_categories().unwrap_or_default();
        if !self.categories.is_empty() && self.selected_category.is_none() {
            self.selected_category = Some(0);
            self.load_repos_for_selected(db);
        }
    }

    pub fn load_repos_for_selected(&mut self, db: &Database) {
        if let Some(idx) = self.selected_category {
            if let Some(cat) = self.categories.get(idx) {
                self.displayed_repos = db.get_repos_by_category(cat.id).unwrap_or_default();
                self.selected_repo = if self.displayed_repos.is_empty() {
                    None
                } else {
                    Some(0)
                };
            }
        }
    }

    pub fn run_search(&mut self, db: &Database) {
        if self.search_query.is_empty() {
            self.search_results.clear();
            self.selected_repo = None;
        } else {
            self.search_results = db.search_repos(&self.search_query).unwrap_or_default();
            self.selected_repo = if self.search_results.is_empty() {
                None
            } else {
                Some(0)
            };
        }
    }

    // ── Key handlers ─────────────────────────────────────────────────────────

    pub fn handle_key(
        &mut self,
        key: crossterm::event::KeyEvent,
        db: &Database,
    ) -> Result<()> {
        match self.screen {
            Screen::Setup => self.handle_setup_key(key),
            Screen::Login => self.handle_login_key(key),
            Screen::Home => self.handle_home_key(key),
            Screen::Browse => self.handle_browse_key(key, db),
            Screen::Search => self.handle_search_key(key, db),
            Screen::Settings => self.handle_settings_key(key),
            Screen::Syncing => {
                if matches!(
                    &self.sync_status,
                    SyncStatus::Done(_) | SyncStatus::Error(_)
                ) {
                    self.sync_status = SyncStatus::Idle;
                    self.screen = Screen::Home;
                }
            }
        }
        Ok(())
    }

    fn handle_setup_key(&mut self, key: crossterm::event::KeyEvent) {
        match key.code {
            KeyCode::Char('q') if crossterm::event::KeyModifiers::CONTROL == key.modifiers => {
                self.should_quit = true;
            }
            KeyCode::Char(c) => self.setup_input.push(c),
            KeyCode::Backspace => { self.setup_input.pop(); }
            KeyCode::Enter => {
                let id = self.setup_input.trim().to_string();
                if !id.is_empty() {
                    self.config.client_id = Some(id);
                    let _ = self.config.save();
                    self.setup_input.clear();
                    // Trigger device flow next tick
                }
            }
            KeyCode::Esc => self.should_quit = true,
            _ => {}
        }
    }

    fn handle_login_key(&mut self, key: crossterm::event::KeyEvent) {
        if is_quit(&key) {
            self.should_quit = true;
        }
    }

    fn handle_home_key(&mut self, key: crossterm::event::KeyEvent) {
        match key.code {
            KeyCode::Char('q') | KeyCode::Esc => self.should_quit = true,
            KeyCode::Char('b') => self.screen = Screen::Browse,
            KeyCode::Char('/') => {
                self.search_query.clear();
                self.search_results.clear();
                self.selected_repo = None;
                self.screen = Screen::Search;
            }
            KeyCode::Char('s') => self.screen = Screen::Settings,
            KeyCode::Char('u') => self.screen = Screen::Syncing,
            _ => {}
        }
    }

    fn handle_browse_key(&mut self, key: crossterm::event::KeyEvent, db: &Database) {
        match key.code {
            KeyCode::Char('q') | KeyCode::Esc => self.screen = Screen::Home,
            KeyCode::Left => self.browse_pane = BrowsePane::Categories,
            KeyCode::Right | KeyCode::Tab => self.browse_pane = BrowsePane::Repos,
            KeyCode::Up => match self.browse_pane {
                BrowsePane::Categories => {
                    if let Some(i) = self.selected_category {
                        let new = i.saturating_sub(1);
                        self.selected_category = Some(new);
                        self.load_repos_for_selected(db);
                    }
                }
                BrowsePane::Repos => {
                    if let Some(i) = self.selected_repo {
                        self.selected_repo = Some(i.saturating_sub(1));
                    }
                }
            },
            KeyCode::Down => match self.browse_pane {
                BrowsePane::Categories => {
                    let max = self.categories.len().saturating_sub(1);
                    let new = self.selected_category.map(|i| (i + 1).min(max)).unwrap_or(0);
                    self.selected_category = Some(new);
                    self.load_repos_for_selected(db);
                }
                BrowsePane::Repos => {
                    let max = self.displayed_repos.len().saturating_sub(1);
                    let new = self.selected_repo.map(|i| (i + 1).min(max)).unwrap_or(0);
                    self.selected_repo = Some(new);
                }
            },
            KeyCode::Enter => {
                if self.browse_pane == BrowsePane::Repos {
                    self.open_selected_repo_url(&self.displayed_repos.clone());
                }
            }
            _ => {}
        }
    }

    fn handle_search_key(&mut self, key: crossterm::event::KeyEvent, db: &Database) {
        match key.code {
            KeyCode::Esc => self.screen = Screen::Home,
            KeyCode::Backspace => {
                self.search_query.pop();
                self.run_search(db);
            }
            KeyCode::Char(c) => {
                self.search_query.push(c);
                self.run_search(db);
            }
            KeyCode::Up => {
                if let Some(i) = self.selected_repo {
                    self.selected_repo = Some(i.saturating_sub(1));
                }
            }
            KeyCode::Down => {
                let max = self.search_results.len().saturating_sub(1);
                let new = self.selected_repo.map(|i| (i + 1).min(max)).unwrap_or(0);
                if !self.search_results.is_empty() {
                    self.selected_repo = Some(new);
                }
            }
            KeyCode::Enter => {
                self.open_selected_repo_url(&self.search_results.clone());
            }
            _ => {}
        }
    }

    fn handle_settings_key(&mut self, key: crossterm::event::KeyEvent) {
        match key.code {
            KeyCode::Char('q') | KeyCode::Esc => self.screen = Screen::Home,
            KeyCode::Char('a') => {
                self.config.auto_update = !self.config.auto_update;
                let _ = self.config.save();
            }
            KeyCode::Char('l') => {
                self.config.github_token = None;
                let _ = self.config.save();
                self.screen = Screen::Login;
            }
            _ => {}
        }
    }

    fn open_selected_repo_url(&self, repos: &[RepoRow]) {
        if let Some(idx) = self.selected_repo {
            if let Some(repo) = repos.get(idx) {
                let _ = open::that(&repo.url);
            }
        }
    }

    // ── Auth polling ─────────────────────────────────────────────────────────

    pub fn start_device_flow(&mut self, device: DeviceCodeResponse) {
        self.device_user_code = Some(device.user_code);
        self.device_verification_uri = Some(device.verification_uri);
        self.device_code = Some(device.device_code);
        self.device_poll_interval_secs = device.interval;
        self.last_poll_at = None; // poll immediately on first tick
        self.screen = Screen::Login;
    }

    /// Returns true if enough time has elapsed since last poll.
    pub fn should_poll_now(&mut self) -> bool {
        match self.last_poll_at {
            None => true,
            Some(t) => t.elapsed().as_secs() >= self.device_poll_interval_secs,
        }
    }

    /// Call after a successful poll attempt (regardless of result).
    pub fn mark_polled(&mut self) {
        self.last_poll_at = Some(Instant::now());
    }

    /// Call when GitHub returns slow_down — add 5s to the interval.
    pub fn slow_down(&mut self) {
        self.device_poll_interval_secs += 5;
    }

    pub fn device_code(&self) -> Option<&str> {
        self.device_code.as_deref()
    }
}

// ── Run loop ──────────────────────────────────────────────────────────────────

pub async fn run_app(
    terminal: &mut ratatui::Terminal<ratatui::backend::CrosstermBackend<std::io::Stdout>>,
    app: &mut App,
    db: &Database,
) -> Result<()> {
    let tick_rate = Duration::from_millis(200);
    let auth_client = AuthClient::new();

    // Determine initial screen
    if app.config.is_authenticated() {
        app.screen = Screen::Home;
        app.load_stats(db);
    } else if app.config.client_id().is_none() {
        // First run: user must provide their OAuth App Client ID
        app.screen = Screen::Setup;
    } else {
        // Has client_id but no token → start device flow immediately
        let client_id = app.config.client_id().unwrap().to_owned();
        match auth_client.request_device_code(&client_id).await {
            Ok(device) => app.start_device_flow(device),
            Err(e) => {
                app.sync_status = SyncStatus::Error(e.to_string());
                app.screen = Screen::Login;
            }
        }
    }

    loop {
        // Draw current screen
        terminal.draw(|f| crate::tui::ui::draw(f, app))?;

        // Handle events
        if let Some(event) = poll_event(tick_rate)? {
            match event {
                AppEvent::Key(key) => {
                    app.handle_key(key, db)?;
                }
                AppEvent::Tick => {
                    app.tick_count = app.tick_count.wrapping_add(1);

                    // After user entered client_id in Setup screen, start device flow
                    if app.screen == Screen::Setup
                        && app.config.client_id().is_some()
                        && app.device_code.is_none()
                    {
                        let client_id = app.config.client_id().unwrap().to_owned();
                        match auth_client.request_device_code(&client_id).await {
                            Ok(device) => app.start_device_flow(device),
                            Err(e) => {
                                app.sync_status = SyncStatus::Error(e.to_string());
                                app.screen = Screen::Login;
                            }
                        }
                    }

                    // Poll for OAuth token
                    if app.screen == Screen::Login && app.should_poll_now() {
                        if let (Some(code), Some(client_id)) = (
                            app.device_code().map(str::to_owned),
                            app.config.client_id().map(str::to_owned),
                        ) {
                            app.mark_polled();
                            match auth_client.poll_for_token(&client_id, &code).await {
                                Ok(PollResult::Token(token)) => {
                                    app.config.github_token = Some(token);
                                    let _ = app.config.save();
                                    app.screen = Screen::Syncing;
                                }
                                Ok(PollResult::SlowDown) => {
                                    app.slow_down();
                                }
                                Ok(PollResult::Pending) => {}
                                Err(e) => {
                                    app.sync_status = SyncStatus::Error(e.to_string());
                                }
                            }
                        }
                    }

                    // ── Auto-update: kick off silently on first Home tick ──
                    if app.screen == Screen::Home
                        && app.config.auto_update
                        && app.fetch_done_rx.is_none()
                        && !app.bg_syncing
                        && app.tick_count == 1   // first tick after startup
                    {
                        if let Some(token) = app.config.github_token.clone() {
                            let (done_tx, done_rx) = oneshot::channel::<Result<Vec<StarredRepo>, String>>();
                            let (prog_tx, _prog_rx) = watch::channel::<usize>(0);
                            app.fetch_done_rx = Some(done_rx);
                            app.bg_syncing = true;
                            tokio::spawn(async move {
                                let api = ApiClient::new(&token);
                                match api.fetch_all_starred(Some(prog_tx)).await {
                                    Ok(repos) => { let _ = done_tx.send(Ok(repos)); }
                                    Err(e)    => { let _ = done_tx.send(Err(e.to_string())); }
                                }
                            });
                        }
                    }

                    // ── Background fetch result (Home screen) ──
                    if app.bg_syncing {
                        let fetch_result: Option<Result<Vec<StarredRepo>, String>> = {
                            if let Some(rx) = &mut app.fetch_done_rx {
                                match rx.try_recv() {
                                    Ok(result) => Some(result),
                                    Err(oneshot::error::TryRecvError::Empty) => None,
                                    Err(oneshot::error::TryRecvError::Closed) => {
                                        Some(Err("Fetch task died".into()))
                                    }
                                }
                            } else { None }
                        };
                        if let Some(result) = fetch_result {
                            app.fetch_done_rx = None;
                            app.bg_syncing = false;
                            if let Ok(repos) = result {
                                for repo in &repos {
                                    if let Ok(row_id) = db.upsert_repo(repo) {
                                        let _ = Classifier::classify_and_store(db, &[(row_id, repo)]);
                                    }
                                }
                                let now = Utc::now().format("%Y-%m-%d %H:%M UTC").to_string();
                                app.config.last_sync = Some(now);
                                let _ = app.config.save();
                                app.load_stats(db);
                                app.load_categories(db);
                            }
                            // errors are silently ignored for background sync
                        }
                    }

                    // Kick off background fetch when entering Syncing
                    if app.screen == Screen::Syncing
                        && app.fetch_done_rx.is_none()
                        && matches!(app.sync_status, SyncStatus::Idle)
                    {
                        if let Some(token) = app.config.github_token.clone() {
                            let (done_tx, done_rx) = oneshot::channel::<Result<Vec<StarredRepo>, String>>();
                            let (prog_tx, prog_rx) = watch::channel::<usize>(0);
                            app.fetch_done_rx = Some(done_rx);
                            app.fetch_progress_rx = Some(prog_rx);
                            app.sync_log.clear();
                            app.sync_status = SyncStatus::FetchingStars(0);
                            app.sync_log.push(LogEntry::info("▶ Starting sync…"));
                            app.sync_log.push(LogEntry::info("  Connecting to GitHub API…"));
                            tokio::spawn(async move {
                                let api = ApiClient::new(&token);
                                match api.fetch_all_starred(Some(prog_tx)).await {
                                    Ok(repos) => { let _ = done_tx.send(Ok(repos)); }
                                    Err(e)    => { let _ = done_tx.send(Err(e.to_string())); }
                                }
                            });
                        }
                    }

                    // Poll live progress (watch — never blocks)
                    if let Some(rx) = &app.fetch_progress_rx {
                        let n = *rx.borrow();
                        if n > 0 {
                            let msg = format!("  Fetching page… ({} repos so far)", n);
                            if let Some(last) = app.sync_log.last_mut() {
                                if last.message.starts_with("  Fetching") {
                                    last.message = msg;
                                } else {
                                    app.sync_log.push(LogEntry::info(msg));
                                }
                            } else {
                                app.sync_log.push(LogEntry::info(msg));
                            }
                            app.sync_status = SyncStatus::FetchingStars(n);
                        }
                    }

                    // Check oneshot for completion
                    let fetch_result: Option<Result<Vec<StarredRepo>, String>> = {
                        if let Some(rx) = &mut app.fetch_done_rx {
                            match rx.try_recv() {
                                Ok(result) => Some(result),
                                Err(oneshot::error::TryRecvError::Empty) => None,
                                Err(oneshot::error::TryRecvError::Closed) => {
                                    Some(Err("Fetch task died unexpectedly".into()))
                                }
                            }
                        } else {
                            None
                        }
                    };

                    if let Some(result) = fetch_result {
                        app.fetch_done_rx = None;
                        app.fetch_progress_rx = None;
                        match result {
                            Ok(repos) => {
                                let total = repos.len();
                                app.sync_log.push(LogEntry::ok(format!(
                                    "  ✓ Fetched {} repos", total
                                )));
                                app.sync_log.push(LogEntry::warn(
                                    "  Classifying by language & topics…",
                                ));
                                app.sync_status = SyncStatus::Classifying;

                                let mut errors = 0usize;
                                for repo in &repos {
                                    match db.upsert_repo(repo) {
                                        Ok(row_id) => {
                                            if let Err(e) = Classifier::classify_and_store(
                                                db, &[(row_id, repo)],
                                            ) {
                                                errors += 1;
                                                app.sync_log.push(LogEntry::err(format!(
                                                    "  ✗ classify {}: {}", repo.name, e
                                                )));
                                            }
                                        }
                                        Err(e) => {
                                            errors += 1;
                                            app.sync_log.push(LogEntry::err(format!(
                                                "  ✗ store {}: {}", repo.name, e
                                            )));
                                        }
                                    }
                                }

                                if errors == 0 {
                                    app.sync_log.push(LogEntry::ok(format!(
                                        "  ✓ Classified {} repos into {} categories",
                                        total,
                                        db.count_categories().unwrap_or(0)
                                    )));
                                } else {
                                    app.sync_log.push(LogEntry::warn(format!(
                                        "  ⚠ Done with {} errors", errors
                                    )));
                                }

                                let now = Utc::now().format("%Y-%m-%d %H:%M UTC").to_string();
                                app.config.last_sync = Some(now.clone());
                                let _ = app.config.save();
                                app.load_stats(db);
                                app.load_categories(db);
                                app.sync_log.push(LogEntry::ok(format!(
                                    "  ✓ Sync complete at {}", now
                                )));
                                app.sync_log.push(LogEntry::info("  Press any key to continue…"));
                                app.sync_status = SyncStatus::Done(total);
                            }
                            Err(e) => {
                                app.sync_log.push(LogEntry::err(format!("  ✗ Error: {}", e)));
                                app.sync_status = SyncStatus::Error(e);
                            }
                        }
                    }

                    // Auto-advance after done/error
                    if matches!(&app.sync_status, SyncStatus::Done(_) | SyncStatus::Error(_))
                        && app.fetch_done_rx.is_none()
                    {
                        if app.tick_count % 15 == 0 {
                            app.sync_status = SyncStatus::Idle;
                            app.screen = Screen::Home;
                        }
                    }
                }
            }
        }

        if app.should_quit {
            break;
        }
    }

    Ok(())
}

