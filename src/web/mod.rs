use std::convert::Infallible;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use axum::{
    extract::{Query, State},
    http::{header, StatusCode},
    response::{
        sse::{Event, KeepAlive, Sse},
        IntoResponse,
    },
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use tokio::sync::broadcast;
use tokio_stream::wrappers::BroadcastStream;
use tokio_stream::StreamExt;

use crate::auth::{AuthClient, PollResult};
use crate::config::Config;
use crate::storage::Database;

// ── Embedded frontend ─────────────────────────────────────────────────────────

const FRONTEND: &str = include_str!("frontend.html");

// ── Sync event ────────────────────────────────────────────────────────────────

#[derive(Clone, Serialize)]
struct SyncEvent {
    #[serde(rename = "type")]
    kind: String,
    msg: String,
}

// ── Shared state ──────────────────────────────────────────────────────────────

struct WebState {
    db: Mutex<Database>,
    config: Mutex<Config>,
    sync_running: AtomicBool,
    /// Broadcast channel for live sync events (all SSE subscribers receive them).
    sync_tx: broadcast::Sender<SyncEvent>,
    /// Buffered log of the most recent sync run (cleared at start of each sync).
    sync_log: Mutex<Vec<SyncEvent>>,
}

type Shared = Arc<WebState>;

// ── Entry point ───────────────────────────────────────────────────────────────

pub async fn run() -> anyhow::Result<()> {
    let config = Config::load()?;
    let db_path = Config::db_path()?;
    let db = Database::open(&db_path)?;
    let (sync_tx, _) = broadcast::channel(256);
    let state: Shared = Arc::new(WebState {
        db: Mutex::new(db),
        config: Mutex::new(config),
        sync_running: AtomicBool::new(false),
        sync_tx,
        sync_log: Mutex::new(Vec::new()),
    });

    // Background auto-sync check (runs once after server starts)
    tokio::spawn(auto_sync_check(Arc::clone(&state)));

    let app = Router::new()
        .route("/", get(frontend))
        // Read
        .route("/api/stats", get(api_stats))
        .route("/api/categories", get(api_categories))
        .route("/api/repos", get(api_repos))
        .route("/api/search", get(api_search))
        // Auth
        .route("/api/auth/status", get(api_auth_status))
        .route("/api/auth/setup", post(api_auth_setup))
        .route("/api/auth/start", post(api_auth_start))
        .route("/api/auth/poll", post(api_auth_poll))
        .route("/api/auth/logout", post(api_auth_logout))
        // Sync: GET = SSE subscriber, POST = manual trigger
        .route("/api/sync", get(api_sync_sse).post(api_sync_trigger))
        .route("/api/sync/status", get(api_sync_status))
        // Settings
        .route("/api/settings", get(api_settings_get).patch(api_settings_update))
        // AI Search
        .route("/api/ai-search", post(api_ai_search))
        .with_state(state);

    let addr = "127.0.0.1:3000";
    let listener = tokio::net::TcpListener::bind(addr).await?;

    println!("⭐  My GitHub Stars — web UI at http://{addr}");
    let _ = open::that(format!("http://{addr}"));

    axum::serve(listener, app).await?;
    Ok(())
}

// ── Frontend ──────────────────────────────────────────────────────────────────

async fn frontend() -> impl IntoResponse {
    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, "text/html; charset=utf-8")],
        FRONTEND,
    )
}

// ── Read-only ─────────────────────────────────────────────────────────────────

#[derive(Serialize)]
struct Stats {
    total_repos: i64,
    total_categories: i64,
    last_sync: Option<String>,
}

async fn api_stats(State(s): State<Shared>) -> impl IntoResponse {
    let (total_repos, total_categories) = {
        let db = s.db.lock().unwrap();
        (db.count_repos().unwrap_or(0), db.count_categories().unwrap_or(0))
    };
    let last_sync = s.config.lock().unwrap().last_sync.clone();
    Json(Stats { total_repos, total_categories, last_sync })
}

async fn api_categories(State(s): State<Shared>) -> impl IntoResponse {
    let db = s.db.lock().unwrap();
    Json(db.get_categories().unwrap_or_default())
}

#[derive(Deserialize)]
struct ReposQuery {
    category_id: Option<i64>,
}

async fn api_repos(State(s): State<Shared>, Query(q): Query<ReposQuery>) -> impl IntoResponse {
    let db = s.db.lock().unwrap();
    let repos = match q.category_id {
        Some(id) => db.get_repos_by_category(id).unwrap_or_default(),
        None => db.get_all_repos().unwrap_or_default(),
    };
    Json(repos)
}

#[derive(Deserialize)]
struct SearchQuery {
    q: String,
}

async fn api_search(State(s): State<Shared>, Query(q): Query<SearchQuery>) -> impl IntoResponse {
    let db = s.db.lock().unwrap();
    Json(db.search_repos(&q.q).unwrap_or_default())
}

// ── Auth ──────────────────────────────────────────────────────────────────────

#[derive(Serialize)]
struct AuthStatus {
    authenticated: bool,
    has_client_id: bool,
}

async fn api_auth_status(State(s): State<Shared>) -> impl IntoResponse {
    let config = s.config.lock().unwrap();
    Json(AuthStatus {
        authenticated: config.is_authenticated(),
        has_client_id: config.client_id().is_some(),
    })
}

#[derive(Deserialize)]
struct SetupBody {
    client_id: String,
}

async fn api_auth_setup(
    State(s): State<Shared>,
    Json(body): Json<SetupBody>,
) -> impl IntoResponse {
    let mut config = s.config.lock().unwrap();
    config.client_id = if body.client_id.is_empty() { None } else { Some(body.client_id) };
    match config.save() {
        Ok(_) => StatusCode::OK.into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

async fn api_auth_start(State(s): State<Shared>) -> axum::response::Response {
    let client_id = {
        let config = s.config.lock().unwrap();
        match config.client_id().map(|s| s.to_string()) {
            Some(id) => id,
            None => return (StatusCode::BAD_REQUEST, "No client_id configured").into_response(),
        }
    };
    match AuthClient::new().request_device_code(&client_id).await {
        Ok(r) => Json(serde_json::json!({
            "user_code": r.user_code,
            "verification_uri": r.verification_uri,
            "device_code": r.device_code,
            "interval": r.interval,
        }))
        .into_response(),
        Err(e) => (StatusCode::BAD_REQUEST, e.to_string()).into_response(),
    }
}

#[derive(Deserialize)]
struct PollBody {
    device_code: String,
}

async fn api_auth_poll(
    State(s): State<Shared>,
    Json(body): Json<PollBody>,
) -> axum::response::Response {
    let client_id = {
        let config = s.config.lock().unwrap();
        match config.client_id().map(|s| s.to_string()) {
            Some(id) => id,
            None => return (StatusCode::BAD_REQUEST, "No client_id configured").into_response(),
        }
    };
    match AuthClient::new().poll_for_token(&client_id, &body.device_code).await {
        Ok(PollResult::Token(token)) => {
            let mut config = s.config.lock().unwrap();
            config.github_token = Some(token);
            let _ = config.save();
            Json(serde_json::json!({"status": "authenticated"})).into_response()
        }
        Ok(PollResult::Pending) => Json(serde_json::json!({"status": "pending"})).into_response(),
        Ok(PollResult::SlowDown) => Json(serde_json::json!({"status": "slow_down"})).into_response(),
        Err(e) => (StatusCode::BAD_REQUEST, e.to_string()).into_response(),
    }
}

async fn api_auth_logout(State(s): State<Shared>) -> impl IntoResponse {
    let mut config = s.config.lock().unwrap();
    config.github_token = None;
    match config.save() {
        Ok(_) => StatusCode::OK,
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR,
    }
}

// ── Sync ──────────────────────────────────────────────────────────────────────

/// Core sync logic — fetches all starred repos and saves to DB.
/// Sends events to the broadcast channel and buffers them in sync_log.
async fn run_sync(state: Shared, token: String) {
    // Clear previous log at start of new sync
    state.sync_log.lock().unwrap().clear();

    macro_rules! emit {
        ($kind:literal, $msg:expr) => {{
            let ev = SyncEvent { kind: $kind.into(), msg: $msg.to_string() };
            state.sync_log.lock().unwrap().push(ev.clone());
            let _ = state.sync_tx.send(ev);
        }};
    }

    emit!("log", "Starting sync\u{2026}");

    let (progress_tx, mut progress_rx) = tokio::sync::watch::channel(0usize);
    let tx2 = state.sync_tx.clone();
    let state2 = Arc::clone(&state);
    tokio::spawn(async move {
        let mut last = 0usize;
        loop {
            if progress_rx.changed().await.is_err() {
                break;
            }
            let n = *progress_rx.borrow();
            if n.saturating_sub(last) >= 50 {
                last = n;
                let ev = SyncEvent {
                    kind: "log".into(),
                    msg: format!("Fetched {} repos\u{2026}", n),
                };
                state2.sync_log.lock().unwrap().push(ev.clone());
                let _ = tx2.send(ev);
            }
        }
    });

    let repos = match crate::api::ApiClient::new(&token)
        .fetch_all_starred(Some(progress_tx))
        .await
    {
        Ok(r) => r,
        Err(e) => {
            emit!("error", format!("Fetch failed: {}", e));
            state.sync_running.store(false, Ordering::SeqCst);
            return;
        }
    };

    let total = repos.len();
    emit!("log", format!("Fetched {} repos \u{2014} saving to database\u{2026}", total));

    {
        let db = state.db.lock().unwrap();
        let mut pairs: Vec<(i64, &crate::api::StarredRepo)> = Vec::with_capacity(total);
        for repo in &repos {
            match db.upsert_repo(repo) {
                Ok(id) => pairs.push((id, repo)),
                Err(e) => eprintln!("upsert {}: {}", repo.full_name, e),
            }
        }
        if let Err(e) = crate::classifier::Classifier::classify_and_store(&db, &pairs) {
            eprintln!("classifier: {}", e);
        }
    }

    {
        let ts = chrono::Utc::now().format("%Y-%m-%d %H:%M UTC").to_string();
        let mut config = state.config.lock().unwrap();
        config.last_sync = Some(ts);
        let _ = config.save();
    }

    emit!("done", format!("\u{2713} Synced {} repos", total));
    state.sync_running.store(false, Ordering::SeqCst);
}

/// Background task: on startup, compare local vs GitHub count; sync only if different.
async fn auto_sync_check(state: Shared) {
    // Give the server a moment to start accepting connections
    tokio::time::sleep(std::time::Duration::from_millis(800)).await;

    let (auto_update, token) = {
        let config = state.config.lock().unwrap();
        (config.auto_update, config.github_token.clone())
    };

    if !auto_update {
        return;
    }
    let Some(token) = token.filter(|t| !t.is_empty()) else {
        return;
    };

    let local_count = state.db.lock().unwrap().count_repos().unwrap_or(0);
    let api = crate::api::ApiClient::new(&token);

    match api.get_starred_count().await {
        Ok(github_count) => {
            if local_count == github_count {
                eprintln!("[auto-sync] Up-to-date ({} repos) — skipping", local_count);
                return;
            }
            eprintln!(
                "[auto-sync] Count mismatch (local={}, github={}) — syncing",
                local_count, github_count
            );
            if state
                .sync_running
                .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
                .is_ok()
            {
                run_sync(state, token).await;
            }
        }
        Err(e) => eprintln!("[auto-sync] Count check failed: {}", e),
    }
}

/// GET /api/sync — SSE stream. Sends buffered log first, then live events.
async fn api_sync_sse(State(s): State<Shared>) -> impl IntoResponse {
    // Subscribe before reading log to avoid missing events between the two
    let rx = s.sync_tx.subscribe();
    let buffered = s.sync_log.lock().unwrap().clone();

    let buf_stream = tokio_stream::iter(buffered).map(|ev| {
        let data = serde_json::to_string(&ev).unwrap_or_default();
        Ok::<Event, Infallible>(Event::default().data(data))
    });

    let live_stream = BroadcastStream::new(rx).filter_map(|r| match r {
        Ok(ev) => {
            let data = serde_json::to_string(&ev).unwrap_or_default();
            Some(Ok::<Event, Infallible>(Event::default().data(data)))
        }
        Err(_) => None,
    });

    Sse::new(buf_stream.chain(live_stream)).keep_alive(KeepAlive::default()).into_response()
}

/// POST /api/sync — manually trigger a sync.
async fn api_sync_trigger(State(s): State<Shared>) -> axum::response::Response {
    let token = {
        let config = s.config.lock().unwrap();
        match config.github_token.clone().filter(|t| !t.is_empty()) {
            Some(t) => t,
            None => return (StatusCode::UNAUTHORIZED, "Not authenticated").into_response(),
        }
    };

    if s.sync_running
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_err()
    {
        return (StatusCode::CONFLICT, "Sync already running").into_response();
    }

    tokio::spawn(run_sync(Arc::clone(&s), token));
    StatusCode::OK.into_response()
}

/// GET /api/sync/status — current sync state and recent log.
#[derive(Serialize)]
struct SyncStatus {
    running: bool,
    log: Vec<SyncEvent>,
}

async fn api_sync_status(State(s): State<Shared>) -> impl IntoResponse {
    let running = s.sync_running.load(Ordering::SeqCst);
    let log = s.sync_log.lock().unwrap().clone();
    Json(SyncStatus { running, log })
}

// ── Settings ──────────────────────────────────────────────────────────────────

#[derive(Serialize)]
struct SettingsResponse {
    client_id: Option<String>,
    auto_update: bool,
    has_openai_key: bool,
    openai_base_url: Option<String>,
    openai_model: Option<String>,
    use_copilot: bool,
    has_copilot_token: bool,
    last_sync: Option<String>,
    known_models: Vec<KnownModel>,
}

#[derive(Serialize)]
struct KnownModel {
    id: &'static str,
    desc: &'static str,
}

async fn api_settings_get(State(s): State<Shared>) -> impl IntoResponse {
    let config = s.config.lock().unwrap();
    Json(SettingsResponse {
        client_id: config.client_id.clone(),
        auto_update: config.auto_update,
        has_openai_key: config.openai_api_key.as_deref().is_some_and(|k| !k.is_empty()),
        openai_base_url: config.openai_base_url.clone(),
        openai_model: config.openai_model.clone(),
        use_copilot: config.use_copilot,
        has_copilot_token: config.copilot_github_token.as_deref().is_some_and(|k| !k.is_empty()),
        last_sync: config.last_sync.clone(),
        known_models: crate::ai::KNOWN_MODELS
            .iter()
            .map(|(id, desc)| KnownModel { id, desc })
            .collect(),
    })
}

#[derive(Deserialize)]
struct SettingsUpdate {
    client_id: Option<String>,
    auto_update: Option<bool>,
    openai_api_key: Option<String>,
    openai_base_url: Option<String>,
    openai_model: Option<String>,
    use_copilot: Option<bool>,
    copilot_github_token: Option<String>,
}

async fn api_settings_update(
    State(s): State<Shared>,
    Json(body): Json<SettingsUpdate>,
) -> impl IntoResponse {
    let mut config = s.config.lock().unwrap();
    if let Some(v) = body.client_id {
        config.client_id = if v.is_empty() { None } else { Some(v) };
    }
    if let Some(v) = body.auto_update {
        config.auto_update = v;
    }
    if let Some(v) = body.openai_api_key {
        config.openai_api_key = if v.is_empty() { None } else { Some(v) };
    }
    if let Some(v) = body.openai_base_url {
        config.openai_base_url = if v.is_empty() { None } else { Some(v) };
    }
    if let Some(v) = body.openai_model {
        config.openai_model = if v.is_empty() { None } else { Some(v) };
    }
    if let Some(v) = body.use_copilot {
        config.use_copilot = v;
    }
    if let Some(v) = body.copilot_github_token {
        config.copilot_github_token = if v.is_empty() { None } else { Some(v) };
    }
    match config.save() {
        Ok(_) => StatusCode::OK.into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

// ── AI Search ─────────────────────────────────────────────────────────────────

#[derive(Deserialize)]
struct AiSearchBody {
    query: String,
}

async fn api_ai_search(
    State(s): State<Shared>,
    Json(body): Json<AiSearchBody>,
) -> axum::response::Response {
    let ai_client = {
        let config = s.config.lock().unwrap();
        if config.use_copilot {
            let token = config
                .copilot_github_token
                .as_deref()
                .filter(|t| !t.is_empty())
                .or_else(|| config.github_token.as_deref().filter(|t| !t.is_empty()))
                .unwrap_or("")
                .to_string();
            crate::ai::AiClient::new_copilot(&token, config.openai_model.as_deref())
        } else {
            let key = config
                .openai_api_key
                .as_deref()
                .filter(|t| !t.is_empty())
                .unwrap_or("")
                .to_string();
            if key.is_empty() {
                return (StatusCode::BAD_REQUEST, "No AI API key configured. Add one in Settings.")
                    .into_response();
            }
            crate::ai::AiClient::new(
                &key,
                config.openai_base_url.as_deref(),
                config.openai_model.as_deref(),
            )
        }
    };

    let all_repos = {
        let db = s.db.lock().unwrap();
        db.get_all_repos().unwrap_or_default()
    };

    match ai_client.search(&body.query, &all_repos).await {
        Ok(names) => {
            let repos = {
                let db = s.db.lock().unwrap();
                db.get_repos_by_full_names(&names).unwrap_or_default()
            };
            Json(repos).into_response()
        }
        Err(e) => (StatusCode::BAD_REQUEST, e.to_string()).into_response(),
    }
}
