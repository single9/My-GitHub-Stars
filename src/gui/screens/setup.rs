use dioxus::prelude::*;

use crate::gui::state::{GuiAppState, GuiScreen};

#[component]
pub fn SetupScreen() -> Element {
    let mut state = use_context::<Signal<GuiAppState>>();

    let oninput = move |e: Event<FormData>| {
        state.write().setup_input = e.value();
    };

    let onkeydown = move |e: KeyboardEvent| {
        if e.key() == Key::Enter {
            let id = state.read().setup_input.trim().to_string();
            if !id.is_empty() {
                state.write().config.client_id = Some(id);
                let _ = state.read().config.save();
                state.write().setup_input.clear();
                // Navigate to Login — LoginScreen will start the device flow
                state.write().screen = GuiScreen::Login;
            }
        }
    };

    let value = state.read().setup_input.clone();

    rsx! {
        div {
            style: "display:flex;align-items:center;justify-content:center;height:100vh;",
            div {
                style: "background:#161b22;border:1px solid #30363d;border-radius:12px;padding:40px;max-width:560px;width:100%;",
                h1 {
                    style: "color:#79c0ff;font-size:20px;margin-bottom:8px;",
                    "My GitHub Stars — First-time Setup"
                }
                div { style: "width:40px;height:2px;background:#79c0ff;margin-bottom:24px;" }

                p { style: "color:#e3b341;font-weight:bold;margin-bottom:12px;",
                    "How to get a GitHub OAuth App Client ID:"
                }
                ol {
                    style: "color:#c9d1d9;line-height:2;padding-left:20px;margin-bottom:24px;",
                    li { "Go to " span { style: "color:#79c0ff;", "https://github.com/settings/developers" } }
                    li { "Click \"New OAuth App\"" }
                    li { "Fill in any name/URL, then enable \"Device Flow\"" }
                    li { "Copy the Client ID and paste it below, then press Enter" }
                }

                label { style: "color:#8b949e;font-size:12px;display:block;margin-bottom:6px;",
                    "CLIENT ID"
                }
                input {
                    style: "width:100%;background:#0d1117;border:2px solid #e3b341;border-radius:6px;color:#c9d1d9;font-family:inherit;font-size:14px;padding:10px 14px;outline:none;",
                    placeholder: "Ov23...",
                    value: "{value}",
                    oninput,
                    onkeydown,
                    autofocus: true,
                }
                p { style: "color:#8b949e;font-size:12px;margin-top:16px;",
                    "Press Enter to continue • Esc to quit"
                }
            }
        }
    }
}
