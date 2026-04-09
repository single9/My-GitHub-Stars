use dioxus::prelude::*;

use crate::auth::{AuthClient, PollResult};
use crate::gui::state::{GuiAppState, GuiScreen};

#[component]
pub fn LoginScreen() -> Element {
    let mut state = use_context::<Signal<GuiAppState>>();

    // Start device flow and token polling once on mount.
    let mut started = use_signal(|| false);
    use_effect(move || {
        if *started.read() {
            return;
        }
        started.set(true);

        let client_id = state.peek().config.client_id().map(str::to_owned);
        if let Some(client_id) = client_id {
            spawn(async move {
                // ── Request device code ────────────────────────────────────
                let auth = AuthClient::new();
                match auth.request_device_code(&client_id).await {
                    Ok(device) => {
                        {
                            let mut s = state.write();
                            s.device_user_code = Some(device.user_code);
                            s.device_verification_uri = Some(device.verification_uri);
                            s.device_code = Some(device.device_code);
                            s.device_poll_interval_secs = device.interval;
                            s.auth_error = None;
                        }

                        // ── Poll for token ─────────────────────────────────
                        loop {
                            let (code, interval) = {
                                let s = state.peek();
                                (s.device_code.clone(), s.device_poll_interval_secs)
                            };
                            let Some(code) = code else { break };

                            tokio::time::sleep(std::time::Duration::from_secs(interval)).await;

                            match auth.poll_for_token(&client_id, &code).await {
                                Ok(PollResult::Token(token)) => {
                                    {
                                        let mut s = state.write();
                                        s.config.github_token = Some(token);
                                        let _ = s.config.save();
                                    }
                                    state.write().screen = GuiScreen::Syncing;
                                    break;
                                }
                                Ok(PollResult::SlowDown) => {
                                    state.write().device_poll_interval_secs += 5;
                                }
                                Ok(PollResult::Pending) => {}
                                Err(e) => {
                                    state.write().auth_error = Some(e.to_string());
                                    break;
                                }
                            }
                        }
                    }
                    Err(e) => {
                        state.write().auth_error = Some(e.to_string());
                    }
                }
            });
        }
    });

    let user_code = state.read().device_user_code.clone();
    let verify_uri = state.read().device_verification_uri.clone();
    let auth_error = state.read().auth_error.clone();

    let open_url = move |_| {
        let uri = state.peek().device_verification_uri.clone()
            .unwrap_or_else(|| "https://github.com/login/device".to_string());
        let _ = open::that(&uri);
    };

    rsx! {
        div {
            style: "display:flex;align-items:center;justify-content:center;height:100vh;",
            div {
                style: "background:#161b22;border:1px solid #30363d;border-radius:12px;padding:40px;max-width:480px;width:100%;",
                h1 { style: "color:#79c0ff;font-size:20px;margin-bottom:8px;", "GitHub Authentication" }
                div { style: "width:40px;height:2px;background:#79c0ff;margin-bottom:24px;" }

                p { style: "color:#c9d1d9;margin-bottom:20px;line-height:1.6;",
                    "Open the URL below and enter the code to authorise this application."
                }

                div {
                    style: "background:#0d1117;border:1px solid #30363d;border-radius:8px;padding:20px;margin-bottom:20px;",

                    div { style: "margin-bottom:16px;",
                        span { style: "color:#8b949e;font-size:12px;display:block;margin-bottom:4px;", "URL" }
                        if let Some(uri) = &verify_uri {
                            span { style: "color:#e3b341;word-break:break-all;", "{uri}" }
                        } else {
                            span { style: "color:#8b949e;", "Requesting…" }
                        }
                    }

                    div {
                        span { style: "color:#8b949e;font-size:12px;display:block;margin-bottom:4px;", "CODE" }
                        if let Some(code) = &user_code {
                            span {
                                style: "color:#3fb950;font-size:28px;font-weight:bold;letter-spacing:4px;",
                                "{code}"
                            }
                        } else {
                            span { style: "color:#8b949e;", "Waiting for code…" }
                        }
                    }
                }

                if let Some(err) = &auth_error {
                    div {
                        style: "background:#2d1a1a;border:1px solid #f85149;border-radius:6px;padding:12px;margin-bottom:16px;color:#f85149;",
                        "⚠ {err}"
                    }
                }

                if verify_uri.is_some() {
                    button {
                        style: "width:100%;background:#1f6feb;border:none;border-radius:6px;color:white;cursor:pointer;font-family:inherit;font-size:14px;padding:12px;",
                        onclick: open_url,
                        "Open GitHub in Browser"
                    }
                }

                p { style: "color:#8b949e;font-size:12px;margin-top:16px;text-align:center;",
                    "Waiting for authorisation…"
                }
            }
        }
    }
}
