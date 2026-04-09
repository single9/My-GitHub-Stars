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
| `s` | Settings (toggle auto-update, re-auth) |
| `u` | Manual sync |
| `q` | Quit |

Inside **Browse**: `Tab` switches between the category list and repo list; `↑↓` to navigate; `Enter` on a repo opens it in your browser.

Inside **Search**: type to filter; `↑↓` to select; `Enter` to open in browser; `Esc` to go back.

## Data storage

| OS | Path |
|----|------|
| macOS | `~/Library/Application Support/github-stars-pocket/` |
| Linux | `~/.config/github-stars-pocket/` |
| Windows | `%APPDATA%\github-stars-pocket\` |

| File | Contents |
|------|----------|
| `config.toml` | Auth token, client ID, preferences |
| `stars.db` | SQLite database of repos and categories |

## Architecture

```
src/
├── main.rs          Entry point — load config, open DB, run event loop
├── app.rs           App state machine, event handling, background sync
├── api/             GitHub REST API client (paginated starred fetch)
├── auth/            OAuth Device Flow (request code → poll for token)
├── classifier/      Categorise repos by language + topic tags
├── config/          TOML config load/save
├── storage/         SQLite schema and queries (rusqlite)
└── tui/
    ├── events.rs    Crossterm event polling
    ├── ui.rs        All screen renderers (ratatui)
    └── mod.rs       Terminal init/restore
```

## License

MIT
