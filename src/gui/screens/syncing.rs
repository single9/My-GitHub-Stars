use dioxus::prelude::*;

use crate::api::ApiClient;
use crate::gui::{db, state::{GuiAppState, GuiScreen, LogEntry, SyncStatus}};

#[component]
pub fn SyncingScreen() -> Element {
    let mut state = use_context::<Signal<GuiAppState>>();

    let mut started = use_signal(|| false);
    use_effect(move || {
        if *started.read() {
            return;
        }
        started.set(true);

        {
            let mut s = state.write();
            s.sync_log.clear();
            s.sync_status = SyncStatus::Running;
            s.sync_log.push(LogEntry { text: "Starting sync…".into(), color: "#8b949e" });
        }

        let db_path = state.peek().db_path.clone();

        let Some(token) = state.peek().config.github_token.clone() else {
            state.write().sync_log.push(LogEntry {
                text: "No GitHub token — please log in first.".into(),
                color: "#f85149",
            });
            state.write().sync_status = SyncStatus::Failed;
            return;
        };

        spawn(async move {
            let client = ApiClient::new(token);
            let (page_tx, mut page_rx) = tokio::sync::watch::channel(0usize);
            let (result_tx, result_rx) = tokio::sync::oneshot::channel();

            tokio::spawn(async move {
                let r = client.fetch_all_starred(Some(page_tx)).await;
                let _ = result_tx.send(r);
            });

            // Stream progress
            let mut last_page = usize::MAX;
            loop {
                tokio::select! {
                    changed = page_rx.changed() => {
                        if changed.is_ok() {
                            let page = *page_rx.borrow_and_update();
                            if page != last_page {
                                last_page = page;
                                let line = format!("Fetching page {}…", page + 1);
                                let mut s = state.write();
                                match s.sync_log.last_mut() {
                                    Some(l) if l.text.starts_with("Fetching page") => l.text = line,
                                    _ => s.sync_log.push(LogEntry { text: line, color: "#79c0ff" }),
                                }
                            }
                        } else {
                            // Sender dropped — fetch task finished
                            break;
                        }
                    }
                    _ = tokio::time::sleep(std::time::Duration::from_millis(50)) => {}
                }
            }

            let repos = match result_rx.await {
                Ok(Ok(repos)) => repos,
                Ok(Err(e)) => {
                    let msg = format!("Fetch error: {e}");
                    state.write().sync_log.push(LogEntry { text: msg, color: "#f85149" });
                    state.write().sync_status = SyncStatus::Failed;
                    return;
                }
                Err(_) => {
                    state.write().sync_log.push(LogEntry {
                        text: "Fetch task panicked.".into(),
                        color: "#f85149",
                    });
                    state.write().sync_status = SyncStatus::Failed;
                    return;
                }
            };

            let total = repos.len();
            state.write().sync_log.push(LogEntry {
                text: format!("Fetched {total} repos — storing…"),
                color: "#8b949e",
            });

            if let Err(e) = db::store_repos(db_path.clone(), repos).await {
                state.write().sync_log.push(LogEntry { text: format!("Store error: {e}"), color: "#f85149" });
                state.write().sync_status = SyncStatus::Failed;
                return;
            }

            // Reload stats + categories
            let (total_repos, total_cats) = db::load_stats(db_path.clone()).await;
            let cats = db::load_categories(db_path.clone()).await;

            // Persist last_sync timestamp
            let now = chrono::Utc::now().format("%Y-%m-%d %H:%M UTC").to_string();
            {
                let mut cfg = state.peek().config.clone();
                cfg.last_sync = Some(now.clone());
                let _ = cfg.save();
                let mut s = state.write();
                s.config.last_sync = Some(now);
            }

            {
                let mut s = state.write();
                s.total_repos = total_repos;
                s.total_categories = total_cats;
                s.categories = cats;
                s.selected_category = if !s.categories.is_empty() { Some(0) } else { None };
                s.sync_status = SyncStatus::Done;
                s.sync_log.push(LogEntry {
                    text: format!("Done! Synced {total} repos."),
                    color: "#3fb950",
                });
            }

            tokio::time::sleep(std::time::Duration::from_secs(1)).await;
            state.write().screen = GuiScreen::Home;
        });
    });

    let log = state.read().sync_log.clone();
    let status = state.read().sync_status.clone();

    let (spinner, status_color) = match status {
        SyncStatus::Running => ("⟳ Syncing…", "#e3b341"),
        SyncStatus::Done    => ("✓ Done",      "#3fb950"),
        SyncStatus::Failed  => ("✗ Failed",    "#f85149"),
        SyncStatus::Idle    => ("",             "#8b949e"),
    };

    rsx! {
        div {
            style: "display:flex;flex-direction:column;height:100vh;",

            div {
                style: "background:#161b22;border-bottom:1px solid #30363d;padding:12px 20px;display:flex;align-items:center;gap:12px;",
                h2 { style: "color:#3fb950;font-size:16px;margin:0;", "↺ Syncing" }
                span { style: "color:{status_color};font-size:13px;", "{spinner}" }
            }

            div {
                style: "flex:1;overflow-y:auto;padding:16px;font-family:monospace;font-size:13px;",
                for (i, entry) in log.iter().enumerate() {
                    div { key: "{i}", style: "color:{entry.color};margin-bottom:4px;line-height:1.5;",
                        "> {entry.text}"
                    }
                }
            }

            if matches!(state.read().sync_status, SyncStatus::Failed) {
                div { style: "padding:16px;border-top:1px solid #30363d;",
                    button {
                        style: "background:#161b22;border:1px solid #30363d;border-radius:6px;color:#8b949e;cursor:pointer;font-family:inherit;font-size:13px;padding:8px 20px;",
                        onclick: move |_| { state.write().screen = GuiScreen::Home; },
                        "← Back to Home"
                    }
                }
            }
        }
    }
}

