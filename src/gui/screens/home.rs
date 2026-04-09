use dioxus::prelude::*;

use crate::gui::{db, state::{GuiAppState, GuiScreen}};

#[component]
pub fn HomeScreen() -> Element {
    let mut state = use_context::<Signal<GuiAppState>>();

    // Load stats + categories on mount
    let mut loaded = use_signal(|| false);
    use_effect(move || {
        if *loaded.read() {
            return;
        }
        loaded.set(true);
        let db_path = state.peek().db_path.clone();
        spawn(async move {
            let (total_repos, total_cats) = db::load_stats(db_path.clone()).await;
            let categories = db::load_categories(db_path.clone()).await;
            let displayed = if let Some(cat) = categories.first() {
                db::load_repos_for_category(db_path, cat.id).await
            } else {
                Vec::new()
            };
            let mut s = state.write();
            s.total_repos = total_repos;
            s.total_categories = total_cats;
            s.categories = categories;
            if !s.categories.is_empty() && s.selected_category.is_none() {
                s.selected_category = Some(0);
            }
            s.displayed_repos = displayed;
        });
    });

    let total_repos = state.read().total_repos;
    let total_cats = state.read().total_categories;
    let last_sync = state.read().config.last_sync.clone();
    let bg_syncing = state.read().bg_syncing;

    rsx! {
        div {
            style: "display:flex;flex-direction:column;height:100vh;padding:24px;gap:20px;overflow:auto;",

            // ── Header ────────────────────────────────────────────────────
            div {
                h1 { style: "color:#79c0ff;font-size:22px;margin:0;", "⭐ My GitHub Stars" }
                p { style: "color:#8b949e;font-size:12px;margin:4px 0 0;",
                    "Browse and search your starred repositories"
                }
            }

            // ── Stats ─────────────────────────────────────────────────────
            div {
                style: "display:flex;gap:12px;",
                StatCard { label: "Starred Repos", value: total_repos.to_string(), color: "#e3b341" }
                StatCard { label: "Categories", value: total_cats.to_string(), color: "#79c0ff" }
                div {
                    style: "flex:1;background:#161b22;border:1px solid #30363d;border-radius:8px;padding:16px;",
                    div { style: "color:#8b949e;font-size:11px;margin-bottom:4px;", "LAST SYNC" }
                    div {
                        style: "color:#c9d1d9;font-size:15px;font-weight:600;",
                        {last_sync.as_deref().unwrap_or("Never")}
                        if bg_syncing {
                            span { style: "color:#e3b341;font-size:12px;margin-left:8px;", "⟳ syncing…" }
                        }
                    }
                }
            }

            // ── Navigation grid ───────────────────────────────────────────
            div {
                style: "display:grid;grid-template-columns:repeat(3,1fr);gap:12px;",
                NavButton {
                    label: "Browse by Category",
                    icon: "◈",
                    color: "#79c0ff",
                    onclick: move |_| { state.write().screen = GuiScreen::Browse; }
                }
                NavButton {
                    label: "Search Repos",
                    icon: "🔍",
                    color: "#79c0ff",
                    onclick: move |_| {
                        state.write().search_query.clear();
                        state.write().search_results.clear();
                        state.write().screen = GuiScreen::Search;
                    }
                }
                NavButton {
                    label: "AI Search",
                    icon: "✦",
                    color: "#d2a8ff",
                    onclick: move |_| {
                        let mut s = state.write();
                        s.ai_query.clear();
                        s.ai_results.clear();
                        s.ai_error = None;
                        s.screen = GuiScreen::AiSearch;
                    }
                }
                NavButton {
                    label: "Settings",
                    icon: "⚙",
                    color: "#8b949e",
                    onclick: move |_| { state.write().screen = GuiScreen::Settings; }
                }
                NavButton {
                    label: "Sync Now",
                    icon: "↺",
                    color: "#3fb950",
                    onclick: move |_| {
                        let mut s = state.write();
                        s.sync_log.clear();
                        s.sync_status = crate::gui::state::SyncStatus::Idle;
                        s.screen = GuiScreen::Syncing;
                    }
                }
            }
        }
    }
}

#[component]
fn StatCard(label: String, value: String, color: String) -> Element {
    rsx! {
        div {
            style: "background:#161b22;border:1px solid #30363d;border-radius:8px;padding:16px;min-width:140px;",
            div { style: "color:#8b949e;font-size:11px;margin-bottom:4px;", "{label}" }
            div { style: "color:{color};font-size:24px;font-weight:700;", "{value}" }
        }
    }
}

#[component]
fn NavButton(
    label: String,
    icon: String,
    color: String,
    onclick: EventHandler<MouseEvent>,
) -> Element {
    rsx! {
        button {
            style: "background:#161b22;border:1px solid #30363d;border-radius:8px;padding:20px;text-align:left;cursor:pointer;font-family:inherit;transition:border-color 0.15s,background 0.15s;",
            onclick: move |e| onclick.call(e),
            div { style: "color:{color};font-size:24px;margin-bottom:8px;", "{icon}" }
            div { style: "color:#c9d1d9;font-size:13px;font-weight:600;", "{label}" }
        }
    }
}
