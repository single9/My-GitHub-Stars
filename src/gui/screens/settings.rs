use dioxus::prelude::*;

use crate::ai::KNOWN_MODELS;
use crate::gui::state::{GuiAppState, GuiScreen};

#[component]
pub fn SettingsScreen() -> Element {
    let mut state = use_context::<Signal<GuiAppState>>();

    // Local edit buffer
    let mut openai_key = use_signal(|| state.peek().config.openai_api_key.clone().unwrap_or_default());
    let mut openai_url = use_signal(|| state.peek().config.openai_base_url.clone().unwrap_or_default());
    let mut model_input = use_signal(|| state.peek().config.openai_model.clone().unwrap_or_default());
    let mut copilot_token = use_signal(|| state.peek().config.copilot_github_token.clone().unwrap_or_default());
    let mut saved_msg = use_signal(|| Option::<String>::None);

    let save = move |_| {
        let openai_key_val = openai_key.peek().trim().to_string();
        let openai_url_val = openai_url.peek().trim().to_string();
        let model_val = model_input.peek().trim().to_string();
        let copilot_val = copilot_token.peek().trim().to_string();

        spawn(async move {
            let mut cfg = state.peek().config.clone();
            cfg.openai_api_key = if openai_key_val.is_empty() { None } else { Some(openai_key_val) };
            cfg.openai_base_url = if openai_url_val.is_empty() { None } else { Some(openai_url_val) };
            cfg.openai_model = if model_val.is_empty() { None } else { Some(model_val) };
            cfg.copilot_github_token = if copilot_val.is_empty() { None } else { Some(copilot_val) };

            if cfg.save().is_ok() {
                let mut s = state.write();
                s.config.openai_api_key = cfg.openai_api_key;
                s.config.openai_base_url = cfg.openai_base_url;
                s.config.openai_model = cfg.openai_model;
                s.config.copilot_github_token = cfg.copilot_github_token;
                saved_msg.set(Some("✓ Settings saved".to_string()));
            } else {
                saved_msg.set(Some("⚠ Failed to save settings".to_string()));
            }
        });
    };

    let auto_update = state.read().config.auto_update;
    let use_copilot = state.read().config.use_copilot;
    let client_id = state.read().config.client_id.clone();
    let token_status = if state.read().config.github_token.is_some() {
        "✓ Authenticated"
    } else {
        "✗ Not authenticated"
    };
    let client_id_display = client_id.unwrap_or_else(|| "Not set".to_string());

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
                h2 { style: "color:#8b949e;font-size:16px;margin:0;", "⚙ Settings" }
            }

            div {
                style: "flex:1;overflow-y:auto;padding:24px;max-width:640px;",

                // ── GitHub account ────────────────────────────────────────
                Section { title: "GitHub Account",
                    Row { label: "OAuth Client ID".to_string(), value: client_id_display }
                    Row { label: "Token status".to_string(), value: token_status.to_string() }
                    button {
                        style: "background:#2d1a1a;border:1px solid #f85149;border-radius:6px;color:#f85149;cursor:pointer;font-family:inherit;font-size:12px;padding:6px 14px;margin-top:8px;",
                        onclick: move |_| {
                            spawn(async move {
                                let mut cfg = state.peek().config.clone();
                                cfg.github_token = None;
                                let _ = cfg.save();
                                let mut s = state.write();
                                s.config.github_token = None;
                                s.screen = GuiScreen::Login;
                            });
                        },
                        "Log out (re-authenticate)"
                    }
                }

                // ── General ───────────────────────────────────────────────
                Section { title: "General",
                    div { style: "display:flex;align-items:center;justify-content:space-between;margin-bottom:8px;",
                        span { style: "color:#c9d1d9;font-size:13px;", "Auto-update on startup" }
                        {
                            let btn_bg = if auto_update { "#1f6feb" } else { "#21262d" };
                            let label = if auto_update { "On" } else { "Off" };
                            rsx! {
                                button {
                                    style: "background:{btn_bg};border:1px solid #30363d;border-radius:20px;color:white;cursor:pointer;font-family:inherit;font-size:12px;padding:4px 14px;",
                                    onclick: move |_| {
                                        let new_val = !state.peek().config.auto_update;
                                        state.write().config.auto_update = new_val;
                                        let cfg_clone = state.peek().config.clone();
                                        spawn(async move { let _ = cfg_clone.save(); });
                                    },
                                    "{label}"
                                }
                            }
                        }
                    }
                }

                // ── AI (OpenAI / Copilot) ─────────────────────────────────
                Section { title: "AI Search",
                    div { style: "display:flex;align-items:center;justify-content:space-between;margin-bottom:12px;",
                        span { style: "color:#c9d1d9;font-size:13px;", "Use GitHub Copilot" }
                        {
                            let copilot_bg = if use_copilot { "#6e40c9" } else { "#21262d" };
                            let copilot_label = if use_copilot { "On" } else { "Off" };
                            rsx! {
                                button {
                                    style: "background:{copilot_bg};border:1px solid #30363d;border-radius:20px;color:white;cursor:pointer;font-family:inherit;font-size:12px;padding:4px 14px;",
                                    onclick: move |_| {
                                        let new_val = !state.peek().config.use_copilot;
                                        state.write().config.use_copilot = new_val;
                                    },
                                    "{copilot_label}"
                                }
                            }
                        }
                    }

                    if use_copilot {
                        LabeledInput {
                            label: "GitHub Personal Access Token (Copilot)",
                            placeholder: "ghp_…",
                            value: copilot_token.read().clone(),
                            oninput: move |e: Event<FormData>| copilot_token.set(e.value()),
                            is_password: true,
                        }
                    } else {
                        LabeledInput {
                            label: "OpenAI API Key",
                            placeholder: "sk-…",
                            value: openai_key.read().clone(),
                            oninput: move |e: Event<FormData>| openai_key.set(e.value()),
                            is_password: true,
                        }
                        LabeledInput {
                            label: "API Base URL (optional)",
                            placeholder: "https://api.openai.com/v1",
                            value: openai_url.read().clone(),
                            oninput: move |e: Event<FormData>| openai_url.set(e.value()),
                            is_password: false,
                        }
                    }

                    ModelPicker {
                        value: model_input.read().clone(),
                        onchange: move |v: String| model_input.set(v),
                    }
                }

                // ── Save button ───────────────────────────────────────────
                div { style: "display:flex;align-items:center;gap:12px;margin-top:16px;",
                    button {
                        style: "background:#1f6feb;border:none;border-radius:6px;color:white;cursor:pointer;font-family:inherit;font-size:13px;padding:8px 24px;",
                        onclick: save,
                        "Save Settings"
                    }
                    if let Some(msg) = saved_msg.read().as_ref() {
                        span {
                            style: "color:#3fb950;font-size:12px;",
                            "{msg}"
                        }
                    }
                }
            }
        }
    }
}

#[component]
fn Section(title: String, children: Element) -> Element {
    rsx! {
        div {
            style: "margin-bottom:24px;",
            h3 { style: "color:#8b949e;font-size:11px;letter-spacing:0.08em;margin-bottom:12px;", "{title}" }
            div { style: "background:#161b22;border:1px solid #30363d;border-radius:8px;padding:16px;display:flex;flex-direction:column;gap:8px;",
                {children}
            }
        }
    }
}

#[component]
fn Row(label: String, value: String) -> Element {
    rsx! {
        div { style: "display:flex;align-items:center;justify-content:space-between;",
            span { style: "color:#8b949e;font-size:12px;", "{label}" }
            span { style: "color:#c9d1d9;font-size:12px;", "{value}" }
        }
    }
}

#[component]
fn LabeledInput(
    label: String,
    placeholder: String,
    value: String,
    oninput: EventHandler<Event<FormData>>,
    is_password: bool,
) -> Element {
    let input_type = if is_password { "password" } else { "text" };
    rsx! {
        div { style: "margin-bottom:8px;",
            label { style: "color:#8b949e;font-size:11px;display:block;margin-bottom:4px;", "{label}" }
            input {
                r#type: "{input_type}",
                style: "width:100%;background:#0d1117;border:1px solid #30363d;border-radius:6px;color:#c9d1d9;font-family:inherit;font-size:13px;padding:7px 10px;box-sizing:border-box;outline:none;",
                placeholder: "{placeholder}",
                value: "{value}",
                oninput,
            }
        }
    }
}

/// Model picker — shows a text input pre-filled with the selected model name plus
/// a collapsible list of well-known models that can be clicked to populate the field.
#[component]
fn ModelPicker(value: String, onchange: EventHandler<String>) -> Element {
    let mut open = use_signal(|| false);
    let mut custom = use_signal(move || value.clone());

    rsx! {
        div { style: "margin-bottom:8px;position:relative;",
            label { style: "color:#8b949e;font-size:11px;display:block;margin-bottom:4px;", "Model" }

            // Text input + toggle button side-by-side
            div { style: "display:flex;gap:6px;",
                input {
                    r#type: "text",
                    style: "flex:1;background:#0d1117;border:1px solid #30363d;border-radius:6px;color:#c9d1d9;font-family:inherit;font-size:13px;padding:7px 10px;box-sizing:border-box;outline:none;",
                    placeholder: "gpt-4o-mini",
                    value: "{custom.read()}",
                    oninput: move |e: Event<FormData>| {
                        let v = e.value();
                        custom.set(v.clone());
                        onchange.call(v);
                    },
                }
                button {
                    r#type: "button",
                    style: "background:#21262d;border:1px solid #30363d;border-radius:6px;color:#8b949e;cursor:pointer;font-family:inherit;font-size:12px;padding:6px 10px;white-space:nowrap;",
                    onclick: move |_| { let cur = *open.read(); open.set(!cur); },
                    if *open.read() { "▲ Hide" } else { "▼ Pick" }
                }
            }

            // Dropdown list of known models
            if *open.read() {
                div { style: "position:absolute;z-index:100;width:100%;background:#161b22;border:1px solid #30363d;border-radius:6px;margin-top:4px;overflow:hidden;box-shadow:0 4px 16px rgba(0,0,0,0.5);",
                    for (id, desc) in KNOWN_MODELS.iter() {
                        {
                            let id_str = id.to_string();
                            let id_str2 = id_str.clone();
                            let is_selected = *custom.read() == id_str;
                            let row_bg = if is_selected { "#1f6feb22" } else { "transparent" };
                            rsx! {
                                div {
                                    key: "{id_str}",
                                    style: "display:flex;justify-content:space-between;padding:9px 12px;cursor:pointer;background:{row_bg};border-bottom:1px solid #21262d;",
                                    onclick: move |_| {
                                        custom.set(id_str2.clone());
                                        onchange.call(id_str2.clone());
                                        open.set(false);
                                    },
                                    span { style: "color:#c9d1d9;font-size:13px;font-family:monospace;", "{id_str}" }
                                    span { style: "color:#8b949e;font-size:11px;margin-left:8px;", "{desc}" }
                                }
                            }
                        }
                    }
                    div {
                        style: "padding:8px 12px;",
                        span { style: "color:#8b949e;font-size:11px;", "Or type a custom model name above" }
                    }
                }
            }
        }
    }
}
