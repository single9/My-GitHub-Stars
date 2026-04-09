# My GitHub Stars (`ghstars`)

A terminal UI (and desktop GUI) app for browsing, searching, and categorising your GitHub starred repositories.

```
┌ My GitHub Stars ──────────────────────────────────────────────┐
│  ★ Starred repos : 1029                                       │
│  ⊕ Categories   : 94                                          │
│  ↺ Last sync    : 2026-04-09 03:32 UTC                        │
│                                                               │
│  [b] Browse by category   [/] Search repos   [i] AI Search    │
│  [s] Settings             [u] Sync now       [q] Quit         │
└───────────────────────────────────────────────────────────────┘
```

## Features

- **Browse** stars grouped by programming language and GitHub topic tags
- **Search** across repo name, description, and topics in real time
- **AI Search** — describe what you need in plain language; the app finds the most relevant repos from your stars using an LLM
- **Model picker** — choose from popular GitHub Models or enter any custom model ID
- **Sync** fetches all starred repos from the GitHub API and stores them locally (SQLite)
- **Auto-update** runs a background sync on startup so the UI is immediately usable
- **OAuth Device Flow** — no secrets stored in code; authorisation happens in your browser
- **Two UI modes** — classic terminal (TUI) or native desktop window (GUI, powered by Dioxus)

## Installation

Requires Rust 1.88+ (edition 2024).

### System dependencies

**TUI build** — no system libraries required (SQLite is bundled).

**GUI build** (`--features gui`) requires the following system libraries on Linux:

```bash
# Debian / Ubuntu
sudo apt-get install -y \
  libsoup-3.0-dev \
  libwebkit2gtk-4.1-dev \
  libxdo-dev
```

> On Fedora/RHEL: `sudo dnf install libsoup3-devel webkit2gtk4.1-devel libxdo-devel`
> On Arch: `sudo pacman -S libsoup3 webkit2gtk-4.1 xdotool`

### Build

```bash
git clone https://github.com/single9/my-gh-stars
cd my-gh-stars

# TUI (default)
cargo install --path .

# Desktop GUI
cargo install --path . --features gui --no-default-features
```

## Running

```bash
# Terminal UI (default)
ghstars

# Desktop GUI
cargo run --features gui --no-default-features
```

## Setup

### 1. Create a GitHub OAuth App

1. Go to <https://github.com/settings/developers> → **New OAuth App**
2. Fill in any name and homepage URL
3. Set **Authorization callback URL** to `http://localhost` (unused by Device Flow)
4. Tick **Enable Device Flow**
5. Click **Register application** and copy the **Client ID**

### 2. First launch

```bash
ghstars
```

On first run you will be prompted to paste your **Client ID**.
The app will then display a short code and open `github.com/login/device` in your browser.
Enter the code there to authorise — the app starts automatically once confirmed.

## Usage

### Home screen

| Key | Action                       |
| --- | ---------------------------- |
| `b` | Browse repos by category     |
| `/` | Search repos (real-time)     |
| `i` | AI Search (natural language) |
| `s` | Settings                     |
| `u` | Manual sync                  |
| `q` | Quit                         |

> In GUI mode, all actions are available via clickable buttons on the Home screen.

### Browse

`Tab` switches between the category list and repo list. `↑↓` to navigate; `Enter` on a repo opens it in your browser.

> In GUI mode, click a category to load its repos; double-click a repo to open it in your browser.

### Search

Type to filter; `↑↓` to select; `Enter` to open in browser; `Esc` to go back.

### AI Search

Type a natural-language query — e.g. _"async HTTP client in Rust"_ or _"vector database for Python"_ — and press `Enter`.

The app first narrows candidates by keyword matching, then sends them to the configured LLM to rank by relevance.

| Key     | Action                               |
| ------- | ------------------------------------ |
| `Enter` | Submit query                         |
| `Tab`   | Move focus to results list           |
| `↑↓`    | Navigate results                     |
| `Enter` | Open selected repo in browser        |
| `Esc`   | Return to query field / back to Home |

### Settings (`s`)

| Key | Action                                         |
| --- | ---------------------------------------------- |
| `a` | Toggle auto-update on startup                  |
| `c` | Toggle GitHub Copilot mode (GitHub Models API) |
| `p` | Set GitHub PAT for Copilot mode                |
| `k` | Set OpenAI API key                             |
| `m` | Pick or change the AI model                    |
| `l` | Log out (clear stored token)                   |

## Data storage

| OS      | Path                                         |
| ------- | -------------------------------------------- |
| macOS   | `~/Library/Application Support/my-gh-stars/` |
| Linux   | `~/.config/my-gh-stars/`                     |
| Windows | `%APPDATA%\my-gh-stars\`                     |

| File          | Contents                                       |
| ------------- | ---------------------------------------------- |
| `config.toml` | Auth token, client ID, AI API key, preferences |
| `stars.db`    | SQLite database of repos and categories        |

## AI Search

AI Search uses a two-stage pipeline:

1. **Keyword pre-filter** — scores each starred repo against your query words (name, description, language, topics) and takes the top 150 candidates
2. **LLM ranking** — sends those candidates to the configured model and asks it to rank by relevance

This keeps requests well within token limits regardless of how many stars you have.

### Option A — GitHub Models (recommended)

Uses the **GitHub Models API** (`models.inference.ai.azure.com`) — OpenAI-compatible, no extra subscription needed. Copilot subscribers get higher rate limits; it's also available on the free tier.

1. Go to [github.com/settings/tokens](https://github.com/settings/tokens) → **Generate new token (classic)** — no scopes needed
2. Open **Settings** (`s`)
3. Press `c` to enable **GitHub Copilot / GitHub Models** mode
4. Press `p`, paste your PAT, and press `Enter`
5. Press `m` to pick a model (default: `gpt-4o-mini`)

### Option B — OpenAI API key

1. Open **Settings** (`s`)
2. Press `k`, enter your OpenAI API key, and press `Enter`
3. Press `m` to pick a model if desired

### Model picker

Press `m` in Settings to open the model selection menu:

```
┌─ Select Model  [↑/↓] navigate  [Enter] select  [Esc] cancel ─────────┐
│ ● gpt-4o-mini                    OpenAI  · fast, cheap (default)       │
│   gpt-4o                         OpenAI  · powerful                    │
│   o3-mini                        OpenAI  · reasoning                   │
│   meta-llama-3.3-70b-instruct    Meta    · open-source, multilingual   │
│   mistral-large-2411             Mistral · multilingual                │
│   phi-4                          Microsoft · small, fast               │
│   deepseek-r1                    DeepSeek · reasoning                  │
│   ...                                                                  │
│   ✎ Custom model name...                                               │
└────────────────────────────────────────────────────────────────────────┘
```

`●` marks the currently active model. The **✎ Custom** option lets you type any model ID — useful for Ollama, LM Studio, or other self-hosted endpoints.

### Custom base URL / local models

Edit `config.toml` directly to point at a different endpoint:

```toml
openai_base_url = "http://localhost:11434/v1"
openai_model    = "llama3"
```

## Architecture

```
src/
├── main.rs          Entry point — selects TUI or GUI based on feature flag
├── app.rs           TUI app state machine, event handling, background sync
├── ai/              LLM client (OpenAI-compatible) + known model list
├── api/             GitHub REST API client (paginated starred fetch)
├── auth/            OAuth Device Flow (request code → poll for token)
├── classifier/      Categorise repos by language + topic tags
├── config/          TOML config load/save (includes AI settings)
├── storage/         SQLite schema and queries (rusqlite)
├── tui/
│   ├── events.rs    Crossterm event polling
│   ├── ui.rs        All screen renderers (ratatui)
│   └── mod.rs       Terminal init/restore
└── gui/             Desktop GUI (Dioxus 0.7, compiled with --features gui)
    ├── mod.rs       Root App component, window config, run() entry point
    ├── state.rs     GuiAppState, GuiScreen enum, SyncStatus, LogEntry
    ├── db.rs        Async DB helpers (spawn_blocking, fresh connections)
    └── screens/     One file per screen: setup, login, home, browse,
                     search, ai_search, settings, syncing
```

## License

MIT
