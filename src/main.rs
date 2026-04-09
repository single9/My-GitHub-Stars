mod app;
mod ai;
mod auth;
mod api;
mod classifier;
mod config;
mod storage;
mod tui;

#[cfg(feature = "gui")]
mod gui;

use anyhow::Result;

// ── GUI entry point ───────────────────────────────────────────────────────────
#[cfg(feature = "gui")]
fn main() {
    gui::run();
}

// ── TUI entry point ───────────────────────────────────────────────────────────
#[cfg(not(feature = "gui"))]
#[tokio::main]
async fn main() -> Result<()> {
    let config = config::Config::load()?;
    let db_path = config::Config::db_path()?;
    let db = storage::Database::open(&db_path)?;

    let mut app = app::App::new(config);

    // Pre-load data if we already have it
    app.load_stats(&db);
    app.load_categories(&db);

    let mut terminal = tui::init_terminal()?;

    let result = app::run_app(&mut terminal, &mut app, &db).await;

    tui::restore_terminal(&mut terminal)?;

    if let Err(e) = result {
        eprintln!("Error: {:#}", e);
        std::process::exit(1);
    }

    Ok(())
}
