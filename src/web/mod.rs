use std::sync::{Arc, Mutex};

use axum::{
    extract::{Query, State},
    http::{header, StatusCode},
    response::IntoResponse,
    routing::get,
    Json, Router,
};
use serde::{Deserialize, Serialize};

use crate::config::Config;
use crate::storage::Database;

// ── Embedded frontend ─────────────────────────────────────────────────────────

const FRONTEND: &str = include_str!("frontend.html");

// ── Shared state ──────────────────────────────────────────────────────────────

struct WebState {
    db: Mutex<Database>,
}

type Shared = Arc<WebState>;

// ── Entry point ───────────────────────────────────────────────────────────────

pub async fn run() -> anyhow::Result<()> {
    let db_path = Config::db_path()?;
    let db = Database::open(&db_path)?;

    let state: Shared = Arc::new(WebState {
        db: Mutex::new(db),
    });

    let app = Router::new()
        .route("/", get(frontend))
        .route("/api/stats", get(api_stats))
        .route("/api/categories", get(api_categories))
        .route("/api/repos", get(api_repos))
        .route("/api/search", get(api_search))
        .with_state(state);

    let addr = "127.0.0.1:3000";
    let listener = tokio::net::TcpListener::bind(addr).await?;

    println!("⭐  My GitHub Stars — web UI at http://{addr}");
    let _ = open::that(format!("http://{addr}"));

    axum::serve(listener, app).await?;
    Ok(())
}

// ── Handlers ──────────────────────────────────────────────────────────────────

async fn frontend() -> impl IntoResponse {
    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, "text/html; charset=utf-8")],
        FRONTEND,
    )
}

#[derive(Serialize)]
struct Stats {
    total_repos: i64,
    total_categories: i64,
}

async fn api_stats(State(s): State<Shared>) -> impl IntoResponse {
    let db = s.db.lock().unwrap();
    Json(Stats {
        total_repos: db.count_repos().unwrap_or(0),
        total_categories: db.count_categories().unwrap_or(0),
    })
}

async fn api_categories(State(s): State<Shared>) -> impl IntoResponse {
    let db = s.db.lock().unwrap();
    Json(db.get_categories().unwrap_or_default())
}

#[derive(Deserialize)]
struct ReposQuery {
    category_id: Option<i64>,
}

async fn api_repos(
    State(s): State<Shared>,
    Query(q): Query<ReposQuery>,
) -> impl IntoResponse {
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

async fn api_search(
    State(s): State<Shared>,
    Query(q): Query<SearchQuery>,
) -> impl IntoResponse {
    let db = s.db.lock().unwrap();
    Json(db.search_repos(&q.q).unwrap_or_default())
}
