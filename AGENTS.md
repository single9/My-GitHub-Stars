# AGENTS — Contributor Guide

This file is for anyone (human or tool) working on this codebase.
Read it before making changes.

---

## Project overview

`gsp` is a Rust TUI application (ratatui + crossterm) that fetches a user's GitHub starred
repositories via OAuth Device Flow, stores them in a local SQLite database, categorises them
by language and topic, and provides browse/search screens.

Binary name: `gsp`
Config & DB location (resolved by `dirs::config_dir()`):

| OS      | Path                                         |
| ------- | -------------------------------------------- |
| macOS   | `~/Library/Application Support/my-gh-stars/` |
| Linux   | `~/.config/my-gh-stars/`                     |
| Windows | `%APPDATA%\my-gh-stars\`                     |

---

## Module map

| Module        | File(s)                 | Responsibility                                                          |
| ------------- | ----------------------- | ----------------------------------------------------------------------- |
| Entry point   | `src/main.rs`           | Load config, open DB, init terminal, call `run_app`                     |
| State machine | `src/app.rs`            | `App` struct, all screen logic, background sync channels                |
| GitHub API    | `src/api/mod.rs`        | `ApiClient` — paginated `/user/starred` fetch with `watch` progress     |
| OAuth         | `src/auth/mod.rs`       | Device Flow: `request_device_code` + `poll_for_token` → `PollResult`    |
| Classifier    | `src/classifier/mod.rs` | `classify_and_store` — assigns language + topic categories per repo     |
| Config        | `src/config/mod.rs`     | `Config` TOML struct (client_id, token, auto_update, last_sync)         |
| Storage       | `src/storage/mod.rs`    | `Database` wrapping rusqlite; schema, upserts, search, category queries |
| TUI events    | `src/tui/events.rs`     | `poll_event` → `AppEvent` (Key / Tick at 200 ms)                        |
| TUI render    | `src/tui/ui.rs`         | `draw()` dispatcher + one `draw_*` fn per screen                        |
| TUI init      | `src/tui/mod.rs`        | `init_terminal` / `restore_terminal` with crossterm raw mode            |

---

## Key data flow

```
GitHub API  ──fetch_all_starred──►  tokio::spawn task
                                         │
                          watch::Sender<usize>   (live page count, non-blocking)
                          oneshot::Sender<Result> (final repos or error)
                                         │
                    App tick loop  ◄─────┘
                         │
                    db.upsert_repo + Classifier::classify_and_store
                         │
                    app.load_stats / load_categories
```

Two sync paths exist:

1. **Background sync** (`app.bg_syncing = true`) — triggered on tick 1 when `auto_update` is
   set; runs without leaving the Home screen; results applied silently.
2. **Foreground sync** (Screen::Syncing) — triggered manually with `[u]` or after first login;
   shows a log-style feed of progress messages and a spinner.

Both use the same `fetch_done_rx: Option<oneshot::Receiver<…>>` field; the kick-off guard
checks `app.fetch_done_rx.is_none()` to avoid double-starting.

---

## Screen flow

```
Setup ──► Login ──► Syncing ──► Home ──┬──► Browse
                                       ├──► Search
                                       └──► Settings
```

`Screen` is an enum in `src/app.rs`. Transitions happen inside `handle_key` or the tick loop.

---

## Concurrency rules

- `rusqlite::Connection` is `!Send` — **never** move `db` into a `tokio::spawn`.
  Do all DB work synchronously in the main event loop after receiving results via channel.
- Use `watch::channel` for streaming progress (latest-value, non-blocking send).
- Use `oneshot::channel` for a single final result.
- Never use `mpsc` with `.await` inside a task that the main loop is also waiting on —
  this caused a deadlock in a previous iteration (forwarding task pattern).

---

## Adding a new screen

1. Add a variant to `Screen` in `src/app.rs`.
2. Add a `draw_<name>` function in `src/tui/ui.rs` and wire it into `draw()`.
3. Add key handling in `App::handle_key` for the new screen variant.
4. Add navigation to/from the screen from an existing screen's key handler.

---

## Adding a new category type

`Classifier::classify_and_store` in `src/classifier/mod.rs` assigns categories.
Currently two types exist: `"language"` and `"topic"`.
To add a new type, push extra `(category_name, type_str)` tuples into the `cats` vec.

---

## SQLite schema

```sql
CREATE TABLE repos (
    id          INTEGER PRIMARY KEY,
    github_id   INTEGER UNIQUE NOT NULL,
    name        TEXT NOT NULL,
    full_name   TEXT NOT NULL,
    owner       TEXT NOT NULL,
    description TEXT,
    language    TEXT,
    stars       INTEGER NOT NULL DEFAULT 0,
    topics      TEXT NOT NULL DEFAULT '[]',   -- JSON array
    url         TEXT NOT NULL,
    starred_at  TEXT NOT NULL
);

CREATE TABLE categories (
    id   INTEGER PRIMARY KEY,
    name TEXT NOT NULL,
    kind TEXT NOT NULL,    -- "language" | "topic"
    UNIQUE(name, kind)
);

CREATE TABLE repo_categories (
    repo_id     INTEGER REFERENCES repos(id),
    category_id INTEGER REFERENCES categories(id),
    PRIMARY KEY (repo_id, category_id)
);

CREATE TABLE settings (
    key   TEXT PRIMARY KEY,
    value TEXT             -- stored as text; booleans use "true"/"false"
);
```

Search uses `LIKE '%query%'` on `name`, `full_name`, `description`, and `topics`.

---

## Settings storage

All application settings are stored in the `settings` table of `stars.db` as
key/value pairs (both `TEXT`). Boolean values are serialised as `"true"` or
`"false"`.

| Key                    | Type    | Notes                                      |
| ---------------------- | ------- | ------------------------------------------ |
| `auto_update`          | bool    | Sync on startup when `true`                |
| `github_token`         | string? | Written by OAuth Device Flow               |
| `client_id`            | string? | GitHub OAuth App Client ID                 |
| `last_sync`            | string? | UTC timestamp of the last successful sync  |
| `openai_api_key`       | string? | OpenAI-compatible API key                  |
| `openai_base_url`      | string? | Override API base URL                      |
| `openai_model`         | string? | Model name for AI search                   |
| `use_copilot`          | bool    | Use GitHub Copilot instead of a plain key  |
| `copilot_github_token` | string? | PAT used to obtain a Copilot session token |

`Config::load()` opens `stars.db` directly via `rusqlite`. If the `settings`
table is empty on first run and a legacy `config.toml` is present, its values
are migrated automatically.

Do **not** commit `stars.db` — it is in `.gitignore`.

---

## Git Commit guidelines

- Write commit messages in **English**.
- Subject line: imperative mood, ≤72 chars (e.g. `Fix sync deadlock with watch channel`).
- Do **not** mention any tool, assistant, or automation system in commit messages
  (no "generated by", "co-authored with assistant", "Copilot", or any similar phrase).
- Do not include any AI author or co-author attribution in commit messages (e.g., no `Co-authored-by: Copilot` or similar trailers).
- Do not commit `stars.db` — it is in `.gitignore`.
- Keep each commit focused on one logical change.

---

## Build & run

```bash
cargo build          # dev build
cargo build --release
cargo run            # runs as `gsp`
```

No tests exist yet. Before adding a feature, confirm the dev build passes with no new errors.
