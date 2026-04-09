use dioxus::prelude::*;

use crate::gui::{db, state::{GuiAppState, GuiScreen}};
use crate::storage::RepoRow;

fn fmt_stars(n: i64) -> String {
    if n >= 1_000_000 { format!("{:.1}M", n as f64 / 1_000_000.0) }
    else if n >= 1_000 { format!("{}k", n / 1_000) }
    else { n.to_string() }
}

#[component]
pub fn SearchScreen() -> Element {
    let mut state = use_context::<Signal<GuiAppState>>();

    let oninput = move |e: Event<FormData>| {
        let query = e.value();
        state.write().search_query = query.clone();
        state.write().selected_repo = None;
        let db_path = state.peek().db_path.clone();
        spawn(async move {
            let results = if query.is_empty() {
                Vec::new()
            } else {
                db::search_repos(db_path, query).await
            };
            state.write().search_results = results;
            state.write().selected_repo = None;
        });
    };

    let query = state.read().search_query.clone();
    let results_count = state.read().search_results.len();

    let selected_detail: Option<RepoRow> = {
        let s = state.read();
        s.selected_repo.and_then(|i| s.search_results.get(i).cloned())
    };

    rsx! {
        div {
            style: "display:flex;flex-direction:column;height:100vh;",

            // ── Header ────────────────────────────────────────────────────
            div {
                style: "background:#161b22;border-bottom:1px solid #30363d;padding:12px 20px;display:flex;align-items:center;gap:12px;",
                button {
                    style: "background:none;border:none;color:#8b949e;cursor:pointer;font-family:inherit;font-size:13px;padding:4px 8px;border-radius:4px;",
                    onclick: move |_| { state.write().screen = GuiScreen::Home; },
                    "← Back"
                }
                h2 { style: "color:#79c0ff;font-size:16px;margin:0;flex:none;", "Search" }
                input {
                    style: "flex:1;background:#0d1117;border:1px solid #30363d;border-radius:6px;color:#c9d1d9;font-family:inherit;font-size:14px;padding:8px 12px;outline:none;",
                    placeholder: "Search repos, descriptions, topics…",
                    value: "{query}",
                    oninput,
                    autofocus: true,
                }
            }

            // ── Results + Detail ──────────────────────────────────────────
            div {
                style: "flex:1;display:flex;flex-direction:column;overflow:hidden;",

                // Results list
                div {
                    style: "flex:3;overflow-y:auto;border-bottom:1px solid #30363d;",
                    div {
                        style: "padding:8px 12px;color:#8b949e;font-size:11px;border-bottom:1px solid #30363d;",
                        "RESULTS ({results_count})"
                    }
                    if query.is_empty() {
                        div { style: "padding:20px;color:#8b949e;", "Type to search…" }
                    }
                    {
                        let (repos, selected) = {
                            let s = state.read();
                            (s.search_results.clone(), s.selected_repo)
                        };
                        repos.into_iter().enumerate().map(move |(i, repo)| {
                            let is_sel = selected == Some(i);
                            let repo_bg = if is_sel { "#21262d" } else { "transparent" };
                            let lang = repo.language.clone().unwrap_or_default();
                            rsx! {
                                div {
                                    key: "{repo.id}",
                                    style: "padding:10px 16px;cursor:pointer;border-bottom:1px solid #21262d;display:flex;align-items:center;justify-content:space-between;background:{repo_bg};",
                                    onclick: move |_| { state.write().selected_repo = Some(i); },
                                    ondoubleclick: move |_| { let _ = open::that(&repo.url); },
                                    div {
                                        div { style: "color:#c9d1d9;font-size:13px;font-weight:600;", "{repo.name}" }
                                        div { style: "color:#8b949e;font-size:11px;", "{repo.full_name}" }
                                    }
                                    div { style: "display:flex;gap:8px;align-items:center;",
                                        if !lang.is_empty() {
                                            span { style: "color:#79c0ff;font-size:11px;", "{lang}" }
                                        }
                                        span { style: "color:#e3b341;font-size:12px;", "★ {fmt_stars(repo.stars_count)}" }
                                    }
                                }
                            }
                        })
                    }
                }

                // Detail panel
                div {
                    style: "flex:2;overflow-y:auto;padding:16px;",
                    if let Some(repo) = selected_detail {
                        div {
                            div { style: "display:flex;align-items:center;justify-content:space-between;margin-bottom:12px;",
                                h3 { style: "color:#c9d1d9;margin:0;font-size:15px;", "{repo.full_name}" }
                                button {
                                    style: "background:#1f6feb;border:none;border-radius:6px;color:white;cursor:pointer;font-family:inherit;font-size:12px;padding:6px 14px;",
                                    onclick: move |_| { let _ = open::that(&repo.url); },
                                    "Open on GitHub ↗"
                                }
                            }
                            if let Some(desc) = &repo.description {
                                p { style: "color:#c9d1d9;margin-bottom:10px;line-height:1.5;", "{desc}" }
                            }
                            div { style: "display:flex;gap:8px;flex-wrap:wrap;",
                                if let Some(lang) = &repo.language {
                                    span { style: "background:#21262d;border:1px solid #30363d;border-radius:4px;color:#79c0ff;font-size:11px;padding:2px 8px;",
                                        "◈ {lang}"
                                    }
                                }
                                span { style: "background:#21262d;border:1px solid #30363d;border-radius:4px;color:#e3b341;font-size:11px;padding:2px 8px;",
                                    "★ {fmt_stars(repo.stars_count)}"
                                }
                                for (i, topic) in repo.topics().into_iter().enumerate() {
                                    span { key: "{i}", style: "background:#21262d;border:1px solid #30363d;border-radius:4px;color:#d2a8ff;font-size:11px;padding:2px 8px;",
                                        "# {topic}"
                                    }
                                }
                            }
                        }
                    } else if !query.is_empty() {
                        p { style: "color:#8b949e;", "Select a result to see details" }
                    }
                }
            }
        }
    }
}
