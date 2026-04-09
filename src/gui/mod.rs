use dioxus::prelude::*;

use crate::config::Config;
use crate::storage::Database;

mod db;
pub mod screens;
pub mod state;

use state::{GuiAppState, GuiScreen};

// ── Global CSS ────────────────────────────────────────────────────────────────

const GLOBAL_CSS: &str = r#"
* { box-sizing: border-box; margin: 0; padding: 0; }
body, html {
    background: #0d1117;
    color: #c9d1d9;
    font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", system-ui, sans-serif;
    font-size: 14px;
    height: 100%;
    overflow: hidden;
}
button:hover { filter: brightness(1.15); }
input:focus { border-color: #58a6ff !important; }
::-webkit-scrollbar { width: 6px; height: 6px; }
::-webkit-scrollbar-track { background: #0d1117; }
::-webkit-scrollbar-thumb { background: #30363d; border-radius: 3px; }
::-webkit-scrollbar-thumb:hover { background: #8b949e; }
"#;

// ── Root component ────────────────────────────────────────────────────────────

#[component]
fn App() -> Element {
    // Initialise state
    let config = Config::load().unwrap_or_default();
    let db_path = Config::db_path().unwrap_or_else(|_| std::path::PathBuf::from("stars.db"));

    // Ensure DB schema is created
    if let Ok(db) = Database::open(&db_path) {
        let _ = db; // schema created by open()
    }

    let app_state = use_signal(|| GuiAppState::new(config, db_path));
    provide_context(app_state);

    let screen = use_context::<Signal<GuiAppState>>().read().screen.clone();

    rsx! {
        style { {GLOBAL_CSS} }
        match screen {
            GuiScreen::Setup    => rsx! { screens::setup::SetupScreen {} },
            GuiScreen::Login    => rsx! { screens::login::LoginScreen {} },
            GuiScreen::Home     => rsx! { screens::home::HomeScreen {} },
            GuiScreen::Browse   => rsx! { screens::browse::BrowseScreen {} },
            GuiScreen::Search   => rsx! { screens::search::SearchScreen {} },
            GuiScreen::AiSearch => rsx! { screens::ai_search::AiSearchScreen {} },
            GuiScreen::Settings => rsx! { screens::settings::SettingsScreen {} },
            GuiScreen::Syncing  => rsx! { screens::syncing::SyncingScreen {} },
        }
    }
}

// ── Entry point ───────────────────────────────────────────────────────────────

pub fn run() {
    use dioxus::desktop::{Config, WindowBuilder};
    use dioxus::desktop::tao::dpi::LogicalSize;

    dioxus::LaunchBuilder::desktop()
        .with_cfg(
            Config::new().with_window(
                WindowBuilder::new()
                    .with_title("My GitHub Stars")
                    .with_always_on_top(false)
                    .with_inner_size(LogicalSize::new(1200.0_f64, 800.0_f64)),
            ),
        )
        .launch(App);
}
