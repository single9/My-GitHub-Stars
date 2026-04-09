use dioxus::prelude::*;

use crate::ai::AiClient;
use crate::gui::{db, state::{GuiAppState, GuiScreen}};
use crate::storage::RepoRow;

fn fmt_stars(n: i64) -> String {
    if n >= 1_000_000 { format!("{:.1}M", n as f64 / 1_000_000.0) }
    else if n >= 1_000 { format!("{}k", n / 1_000) }
    else { n.to_string() }
}

#[component]
pub fn AiSearchScreen() -> Element {
    let mut state = use_context::<Signal<GuiAppState>>();

    let mut do_submit = move || {
        let query = state.peek().ai_query.trim().to_string();
        if query.is_empty() || state.peek().ai_loading {
            return;
        }

        let use_copilot = state.peek().config.use_copilot;
        let model = state.peek().config.openai_model.clone();
        let db_path = state.peek().db_path.clone();

        let client_opt: Option<AiClient> = if use_copilot {
            state.peek().config.copilot_github_token.as_deref()
                .map(|t| AiClient::new_copilot(t, model.as_deref()))
        } else {
            state.peek().config.openai_api_key.as_deref().map(|k| {
                AiClient::new(k, state.peek().config.openai_base_url.as_deref(), model.as_deref())
            })
        };

        let Some(client) = client_opt else {
            let msg = if use_copilot {
                "Copilot token not set. Go to Settings and add a GitHub PAT.".to_string()
            } else {
                "No OpenAI API key configured. Set one in Settings.".to_string()
            };
            state.write().ai_error = Some(msg);
            return;
        };

        {
            let mut s = state.write();
            s.ai_loading = true;
            s.ai_error = None;
            s.ai_results.clear();
            s.selected_repo = None;
            s.ai_query = query.clone();
        }

        spawn(async move {
            let all_repos = db::get_all_repos(db_path.clone()).await;
            let result = client.search(&query, &all_repos).await;
            match result {
                Ok(names) => {
                    let repos = db::get_repos_by_full_names(db_path, names).await;
                    let mut s = state.write();
                    s.ai_results = repos;
                    s.ai_loading = false;
                    s.selected_repo = if s.ai_results.is_empty() { None } else { Some(0) };
                }
                Err(e) => {
                    let mut s = state.write();
                    s.ai_error = Some(e.to_string());
                    s.ai_loading = false;
                }
            }
        });
    };

    let submit_click = {
        let mut do_submit = do_submit.clone();
        move |_: MouseEvent| { do_submit(); }
    };
    let submit_key = move |e: KeyboardEvent| {
        if e.key() == Key::Enter { do_submit(); }
    };

    let ai_query = state.read().ai_query.clone();
    let ai_loading = state.read().ai_loading;
    let ai_error = state.read().ai_error.clone();
    let results_count = state.read().ai_results.len();

    let selected_detail: Option<RepoRow> = {
        let s = state.read();
        s.selected_repo.and_then(|i| s.ai_results.get(i).cloned())
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
                h2 { style: "color:#d2a8ff;font-size:16px;margin:0;flex:none;", "✦ AI Search" }
                input {
                    style: "flex:1;background:#0d1117;border:1px solid #30363d;border-radius:6px;color:#c9d1d9;font-family:inherit;font-size:14px;padding:8px 12px;outline:none;",
                    placeholder: "Describe what you're looking for…",
                    value: "{ai_query}",
                    disabled: ai_loading,
                    oninput: move |e| { state.write().ai_query = e.value(); },
                    onkeydown: submit_key,
                    autofocus: true,
                }
                button {
                    style: "background:#6e40c9;border:none;border-radius:6px;color:white;cursor:pointer;font-family:inherit;font-size:13px;padding:8px 16px;white-space:nowrap;",
                    disabled: ai_loading,
                    onclick: submit_click,
                    if ai_loading { "Searching…" } else { "Search ↵" }
                }
            }

            // ── Results + Detail ──────────────────────────────────────────
            div {
                style: "flex:1;display:flex;flex-direction:column;overflow:hidden;",

                // Status / error
                if let Some(err) = &ai_error {
                    div {
                        style: "background:#2d1a1a;border-bottom:1px solid #f85149;padding:10px 16px;color:#f85149;",
                        "⚠ {err}"
                    }
                }
                if ai_loading {
                    div {
                        style: "padding:20px;color:#d2a8ff;",
                        "⟳ Querying AI model…"
                    }
                }

                // Results list
                div {
                    style: "flex:3;overflow-y:auto;border-bottom:1px solid #30363d;",
                    div {
                        style: "padding:8px 12px;color:#8b949e;font-size:11px;border-bottom:1px solid #30363d;",
                        "RESULTS ({results_count})"
                    }
                    if !ai_loading && !ai_error.is_some() && ai_query.is_empty() {
                        div { style: "padding:20px;color:#8b949e;",
                            "Describe what kind of repos you're looking for and press Enter or click Search."
                        }
                    }
                    {
                        let (repos, selected) = {
                            let s = state.read();
                            (s.ai_results.clone(), s.selected_repo)
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
                    } else if results_count > 0 {
                        p { style: "color:#8b949e;", "Select a result to see details" }
                    }
                }
            }
        }
    }
}
