# GitHub Stars Pocket (`gsp`)

A terminal UI app for browsing, searching, and categorising your GitHub starred repositories.

```
┌ GitHub Stars Pocket ──────────────────────────────────────────┐
│  ★ Starred repos : 1029                                       │
│  ⊕ Categories   : 94                                          │
│  ↺ Last sync    : 2026-04-09 03:32 UTC                        │
│                                                               │
│  [b] Browse by category   [/] Search repos                    │
│  [s] Settings             [u] Sync now                        │
│  [q] Quit                                                     │
└───────────────────────────────────────────────────────────────┘
```

## Features

- **Browse** stars grouped by programming language and GitHub topic tags
- **Search** across repo name, description, and topics in real time
- **AI Search** — describe what you need in plain language; the app queries an OpenAI-compatible LLM to find the most relevant repos from your stars
- **Sync** fetches all starred repos from the GitHub API and stores them locally (SQLite)
- **Auto-update** runs a background sync on startup so the UI is immediately usable
- **OAuth Device Flow** — no secrets stored in code; authorisation happens in your browser

## Installation

```bash
git clone https://github.com/yourname/github-stars-pocket
cd github-stars-pocket
cargo install --path .
```

Requires Rust 1.85+ (edition 2024).

## Setup

### 1. Create a GitHub OAuth App

1. Go to <https://github.com/settings/developers> → **New OAuth App**
2. Fill in any name and homepage URL
3. Set **Authorization callback URL** to `http://localhost` (unused by Device Flow)
4. Tick **Enable Device Flow**
5. Click **Register application** and copy the **Client ID**

### 2. First launch

```bash
gsp
```

On first run you will be prompted to paste your **Client ID**.  
The app will then display a short code and open `github.com/login/device` in your browser.  
Enter the code there to authorise — the app starts automatically once confirmed.

## Usage

| Key | Action |
|-----|--------|
| `b` | Browse repos by category |
| `/` | Search repos |
| `i` | AI Search (natural language) |
| `s` | Settings (toggle auto-update, set AI key, re-auth) |
| `u` | Manual sync |
| `q` | Quit |

Inside **Browse**: `Tab` switches between the category list and repo list; `↑↓` to navigate; `Enter` on a repo opens it in your browser.

Inside **Search**: type to filter; `↑↓` to select; `Enter` to open in browser; `Esc` to go back.

Inside **AI Search**: type a natural-language query (e.g. *"async HTTP client in Rust"*) and press `Enter` to submit. The app sends your starred repos to the configured LLM and returns the most relevant results. `Tab` moves focus to the results list; `↑↓` to select; `Enter` to open in browser; `Esc` returns to the query field.

## Data storage

| OS | Path |
|----|------|
| macOS | `~/Library/Application Support/github-stars-pocket/` |
| Linux | `~/.config/github-stars-pocket/` |
| Windows | `%APPDATA%\github-stars-pocket\` |

| File | Contents |
|------|----------|
| `config.toml` | Auth token, client ID, AI API key, preferences |
| `stars.db` | SQLite database of repos and categories |

## AI Search

AI Search sends your entire starred-repo list (name, description, language, topics) to an OpenAI-compatible chat API and asks the model to rank repos by relevance to your natural-language query.

### Setup

**Option A — GitHub Copilot / GitHub Models (recommended)**

Uses the **GitHub Models API** (`models.inference.ai.azure.com`) — OpenAI-compatible, authenticated with a GitHub Personal Access Token (PAT). Copilot subscribers get higher rate limits; it's also available on GitHub's free tier.

1. Go to [github.com/settings/tokens](https://github.com/settings/tokens) → **Generate new token (classic)** — no scopes needed
2. Open **Settings** (`s` from Home)
3. Press `c` to toggle **GitHub Copilot ON**
4. Press `p` and paste the PAT, then press `Enter`

**Option B — OpenAI API key**

1. Open **Settings** (`s` from Home)
2. Press `k` and enter your OpenAI API key, then press `Enter`

### Custom base URL / local models

Edit `config.toml` directly to use a different endpoint (e.g. Ollama, LM Studio):

```toml
openai_base_url = "http://localhost:11434/v1"
openai_model    = "llama3"
```

The default model is `gpt-4o-mini`.

## Architecture

```
src/
├── main.rs          Entry point — load config, open DB, run event loop
├── app.rs           App state machine, event handling, background sync
├── ai/              OpenAI-compatible client for natural-language search
├── api/             GitHub REST API client (paginated starred fetch)
├── auth/            OAuth Device Flow (request code → poll for token)
├── classifier/      Categorise repos by language + topic tags
├── config/          TOML config load/save (includes AI settings)
├── storage/         SQLite schema and queries (rusqlite)
└── tui/
    ├── events.rs    Crossterm event polling
    ├── ui.rs        All screen renderers (ratatui)
    └── mod.rs       Terminal init/restore
```

## License

MIT
