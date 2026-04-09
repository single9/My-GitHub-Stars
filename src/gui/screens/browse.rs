use dioxus::prelude::*;

use crate::gui::{db, state::{GuiAppState, GuiScreen}};
use crate::storage::RepoRow;

fn fmt_stars(n: i64) -> String {
    if n >= 1_000_000 {
        format!("{:.1}M", n as f64 / 1_000_000.0)
    } else if n >= 1_000 {
        format!("{}k", n / 1_000)
    } else {
        n.to_string()
    }
}

#[component]
pub fn BrowseScreen() -> Element {
    let mut state = use_context::<Signal<GuiAppState>>();

    // Load categories on mount if not yet loaded
    let mut loaded = use_signal(|| false);
    use_effect(move || {
        if *loaded.read() {
            return;
        }
        loaded.set(true);
        if !state.peek().categories.is_empty() {
            return;
        }
        let db_path = state.peek().db_path.clone();
        spawn(async move {
            let cats = db::load_categories(db_path.clone()).await;
            let displayed = if let Some(cat) = cats.first() {
                db::load_repos_for_category(db_path, cat.id).await
            } else {
                Vec::new()
            };
            let mut s = state.write();
            s.categories = cats;
            if !s.categories.is_empty() && s.selected_category.is_none() {
                s.selected_category = Some(0);
            }
            s.displayed_repos = displayed;
            s.selected_repo = None;
        });
    });

    let selected_repo_detail: Option<RepoRow> = {
        let s = state.read();
        s.selected_repo.and_then(|i| s.displayed_repos.get(i).cloned())
    };

    rsx! {
        div {
            style: "display:flex;flex-direction:column;height:100vh;",

            // ── Header ────────────────────────────────────────────────────
            div {
                style: "background:#161b22;border-bottom:1px solid #30363d;padding:12px 20px;display:flex;align-items:center;gap:16px;",
                button {
                    style: "background:none;border:none;color:#8b949e;cursor:pointer;font-family:inherit;font-size:13px;padding:4px 8px;border-radius:4px;",
                    onclick: move |_| { state.write().screen = GuiScreen::Home; },
                    "← Back"
                }
                h2 { style: "color:#79c0ff;font-size:16px;margin:0;", "Browse by Category" }
            }

            // ── Content: sidebar + repos ──────────────────────────────────
            div {
                style: "display:flex;flex:1;overflow:hidden;",

                // ── Category sidebar ──────────────────────────────────────
                div {
                    style: "width:240px;border-right:1px solid #30363d;overflow-y:auto;",
                    div {
                        style: "padding:8px 12px;color:#8b949e;font-size:11px;border-bottom:1px solid #30363d;",
                        "CATEGORIES"
                    }
                    {
                        let (cats, selected) = {
                            let s = state.read();
                            (s.categories.clone(), s.selected_category)
                        };
                        cats.into_iter().enumerate().map(move |(i, cat)| {
                            let is_selected = selected == Some(i);
                            let cat_bg = if is_selected { "#1f6feb" } else { "transparent" };
                            let icon = if cat.category_type == "language" { "◈" } else { "#" };
                            rsx! {
                                div {
                                    key: "{cat.id}",
                                    style: "padding:10px 14px;cursor:pointer;border-bottom:1px solid #21262d;background:{cat_bg};",
                                    onclick: move |_| {
                                        let db_path = state.peek().db_path.clone();
                                        let cat_id = cat.id;
                                        state.write().selected_category = Some(i);
                                        state.write().selected_repo = None;
                                        spawn(async move {
                                            let repos = db::load_repos_for_category(db_path, cat_id).await;
                                            state.write().displayed_repos = repos;
                                        });
                                    },
                                    div { style: "color:#c9d1d9;font-size:13px;",
                                        "{icon} {cat.name}"
                                    }
                                    div { style: "color:#8b949e;font-size:11px;", "{cat.count} repos" }
                                }
                            }
                        })
                    }
                }

                // ── Repos panel ───────────────────────────────────────────
                div {
                    style: "flex:1;display:flex;flex-direction:column;overflow:hidden;",

                    // Repos list (top 60%)
                    div {
                        style: "flex:3;overflow-y:auto;border-bottom:1px solid #30363d;",
                        div {
                            style: "padding:8px 12px;color:#8b949e;font-size:11px;border-bottom:1px solid #30363d;",
                            {
                                let count = state.read().displayed_repos.len();
                                format!("REPOSITORIES ({count})")
                            }
                        }
                        {
                            let (repos, selected) = {
                                let s = state.read();
                                (s.displayed_repos.clone(), s.selected_repo)
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
                                            if !lang.is_empty() {
                                                div { style: "color:#79c0ff;font-size:11px;", "{lang}" }
                                            }
                                        }
                                        span { style: "color:#e3b341;font-size:12px;", "★ {fmt_stars(repo.stars_count)}" }
                                    }
                                }
                            })
                        }
                    }

                    // Detail panel (bottom 40%)
                    div {
                        style: "flex:2;overflow-y:auto;padding:16px;",
                        if let Some(repo) = selected_repo_detail {
                            div {
                                div { style: "display:flex;align-items:center;justify-content:space-between;margin-bottom:16px;",
                                    h3 { style: "color:#c9d1d9;margin:0;font-size:15px;", "{repo.full_name}" }
                                    button {
                                        style: "background:#1f6feb;border:none;border-radius:6px;color:white;cursor:pointer;font-family:inherit;font-size:12px;padding:6px 14px;",
                                        onclick: move |_| { let _ = open::that(&repo.url); },
                                        "Open on GitHub ↗"
                                    }
                                }
                                if let Some(desc) = &repo.description {
                                    p { style: "color:#c9d1d9;margin-bottom:12px;line-height:1.5;", "{desc}" }
                                }
                                div { style: "display:flex;gap:16px;flex-wrap:wrap;",
                                    if let Some(lang) = &repo.language {
                                        Chip { label: format!("◈ {lang}"), color: "#79c0ff" }
                                    }
                                    Chip { label: format!("★ {}", fmt_stars(repo.stars_count)), color: "#e3b341" }
                                    for (i, topic) in repo.topics().into_iter().enumerate() {
                                        Chip { key: "{i}", label: format!("# {topic}"), color: "#d2a8ff" }
                                    }
                                }
                            }
                        } else {
                            p { style: "color:#8b949e;", "Select a repository to see details" }
                        }
                    }
                }
            }
        }
    }
}

#[component]
fn Chip(label: String, color: String) -> Element {
    rsx! {
        span {
            style: "background:#21262d;border:1px solid #30363d;border-radius:4px;color:{color};font-size:11px;padding:3px 8px;",
            "{label}"
        }
    }
}
