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
use tokio_stream::StreamExt;

use crate::auth::{AuthClient, PollResult};
use crate::config::Config;
use crate::storage::Database;

// ── Embedded frontend ─────────────────────────────────────────────────────────

const FRONTEND: &str = include_str!("frontend.html");

// ── Shared state ──────────────────────────────────────────────────────────────

struct WebState {
    db: Mutex<Database>,
    config: Mutex<Config>,
    sync_running: AtomicBool,
}

type Shared = Arc<WebState>;

// ── Entry point ───────────────────────────────────────────────────────────────

pub async fn run() -> anyhow::Result<()> {
    let config = Config::load()?;
    let db_path = Config::db_path()?;
    let db = Database::open(&db_path)?;
    let state: Shared = Arc::new(WebState {
        db: Mutex::new(db),
        config: Mutex::new(config),
        sync_running: AtomicBool::new(false),
    });

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
        // Sync
        .route("/api/sync", get(api_sync))
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

// ── Sync (SSE) ────────────────────────────────────────────────────────────────

async fn api_sync(State(s): State<Shared>) -> axum::response::Response {
    // Verify auth before acquiring sync lock
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

    let (tx, rx) = tokio::sync::mpsc::channel::<String>(128);
    let state_clone = s.clone();

    tokio::spawn(async move {
        macro_rules! send {
            ($type:literal, $msg:expr) => {{
                let data = serde_json::json!({"type": $type, "msg": $msg.to_string()}).to_string();
                if tx.send(data).await.is_err() {
                    state_clone.sync_running.store(false, Ordering::SeqCst);
                    return;
                }
            }};
        }

        send!("log", "Starting sync…");

        let (progress_tx, mut progress_rx) = tokio::sync::watch::channel(0usize);
        let tx2 = tx.clone();
        tokio::spawn(async move {
            let mut last = 0usize;
            loop {
                if progress_rx.changed().await.is_err() {
                    break;
                }
                let n = *progress_rx.borrow();
                if n.saturating_sub(last) >= 50 {
                    last = n;
                    let data = serde_json::json!({"type":"log","msg": format!("Fetched {} repos…", n)}).to_string();
                    if tx2.send(data).await.is_err() {
                        break;
                    }
                }
            }
        });

        let repos = match crate::api::ApiClient::new(&token)
            .fetch_all_starred(Some(progress_tx))
            .await
        {
            Ok(r) => r,
            Err(e) => {
                send!("error", format!("Fetch failed: {}", e));
                state_clone.sync_running.store(false, Ordering::SeqCst);
                return;
            }
        };

        let total = repos.len();
        send!("log", format!("Fetched {} repos — saving to database…", total));

        {
            let db = state_clone.db.lock().unwrap();
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
            let mut config = state_clone.config.lock().unwrap();
            config.last_sync = Some(ts);
            let _ = config.save();
        }

        send!("done", format!("✓ Synced {} repos", total));
        state_clone.sync_running.store(false, Ordering::SeqCst);
    });

    let stream = tokio_stream::wrappers::ReceiverStream::new(rx)
        .map(|data| Ok::<Event, Infallible>(Event::default().data(data)));

    Sse::new(stream).keep_alive(KeepAlive::default()).into_response()
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
