use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph, Wrap},
};

use crate::app::{App, Screen, SyncStatus};
use crate::ai::{DEFAULT_BASE_URL, DEFAULT_MODEL, COPILOT_BASE_URL, COPILOT_DEFAULT_MODEL, KNOWN_MODELS};

fn fmt_stars(n: i64) -> String {
    if n >= 1_000_000 {
        format!("{:.1}M", n as f64 / 1_000_000.0)
    } else if n >= 1_000 {
        format!("{}k", n / 1_000)
    } else {
        n.to_string()
    }
}

pub fn draw(frame: &mut Frame, app: &mut App) {
    match app.screen {
        Screen::Setup => draw_setup(frame, app),
        Screen::Login => draw_login(frame, app),
        Screen::Home => draw_home(frame, app),
        Screen::Browse => draw_browse(frame, app),
        Screen::Search => draw_search(frame, app),
        Screen::AiSearch => draw_ai_search(frame, app),
        Screen::Settings => draw_settings(frame, app),
        Screen::Syncing => draw_syncing(frame, app),
    }
}

// ── helpers ──────────────────────────────────────────────────────────────────

fn title_block(title: &str) -> Block<'_> {
    Block::default()
        .title(format!(" {} ", title))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan))
}

fn highlight_style() -> Style {
    Style::default()
        .fg(Color::Black)
        .bg(Color::Cyan)
        .add_modifier(Modifier::BOLD)
}

// ── setup ─────────────────────────────────────────────────────────────────────

fn draw_setup(frame: &mut Frame, app: &App) {
    let area = frame.area();
    let block = title_block("GitHub Stars Pocket — First-time Setup");
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(2)
        .constraints([
            Constraint::Length(6),
            Constraint::Length(3),
            Constraint::Min(0),
        ])
        .split(inner);

    let instructions = Paragraph::new(vec![
        Line::from(Span::styled("How to get a GitHub OAuth App Client ID:", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD))),
        Line::from(""),
        Line::from("  1. Go to https://github.com/settings/developers"),
        Line::from("  2. Click \"New OAuth App\""),
        Line::from("  3. Fill in any name/URL, then enable \"Device Flow\""),
        Line::from("  4. Copy the \"Client ID\" and paste it below, then press Enter"),
    ])
    .wrap(Wrap { trim: false });
    frame.render_widget(instructions, chunks[0]);

    let input_display = format!("  {}▌", app.setup_input);
    let input = Paragraph::new(input_display)
        .style(Style::default().fg(Color::White))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Client ID ")
                .border_style(Style::default().fg(Color::Yellow)),
        );
    frame.render_widget(input, chunks[1]);

    let hint = Paragraph::new(Span::styled(
        "  Press Esc or Ctrl+C to quit",
        Style::default().fg(Color::DarkGray),
    ));
    frame.render_widget(hint, chunks[2]);
}

// ── login ─────────────────────────────────────────────────────────────────────

fn draw_login(frame: &mut Frame, app: &App) {
    let area = frame.area();
    let block = title_block("GitHub Stars Pocket — Login");

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(2)
        .constraints([
            Constraint::Length(3),
            Constraint::Length(5),
            Constraint::Min(0),
        ])
        .split(inner);

    let intro = Paragraph::new("To authenticate, open the following URL and enter the code below:")
        .style(Style::default().fg(Color::White))
        .wrap(Wrap { trim: true });
    frame.render_widget(intro, chunks[0]);

    let code_text = if let Some(code) = &app.device_user_code {
        vec![
            Line::from(vec![
                Span::raw("  URL : "),
                Span::styled(
                    app.device_verification_uri
                        .as_deref()
                        .unwrap_or("https://github.com/login/device"),
                    Style::default().fg(Color::Yellow).add_modifier(Modifier::UNDERLINED),
                ),
            ]),
            Line::from(""),
            Line::from(vec![
                Span::raw("  Code: "),
                Span::styled(
                    code.as_str(),
                    Style::default()
                        .fg(Color::Green)
                        .add_modifier(Modifier::BOLD),
                ),
            ]),
        ]
    } else {
        vec![Line::from(Span::styled(
            "  Requesting device code…",
            Style::default().fg(Color::DarkGray),
        ))]
    };

    let code_widget = Paragraph::new(Text::from(code_text))
        .block(Block::default().borders(Borders::ALL).title(" Authentication "))
        .wrap(Wrap { trim: false });
    frame.render_widget(code_widget, chunks[1]);

    let status = match &app.sync_status {
        SyncStatus::Error(e) => {
            Paragraph::new(format!("Error: {}", e)).style(Style::default().fg(Color::Red))
        }
        _ => Paragraph::new("Waiting for authorization… (press Ctrl+C to quit)")
            .style(Style::default().fg(Color::DarkGray)),
    };
    frame.render_widget(status, chunks[2]);
}

// ── home ──────────────────────────────────────────────────────────────────────

fn draw_home(frame: &mut Frame, app: &App) {
    let area = frame.area();
    let block = title_block("GitHub Stars Pocket");
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([Constraint::Length(7), Constraint::Min(0)])
        .split(inner);

    // Stats box
    let stats = vec![
        Line::from(vec![
            Span::styled("  ★ Starred repos : ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                app.total_repos.to_string(),
                Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(vec![
            Span::styled("  ⊕ Categories    : ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                app.total_categories.to_string(),
                Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(vec![
            Span::styled("  ↺ Last sync     : ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                app.config
                    .last_sync
                    .as_deref()
                    .unwrap_or("Never"),
                Style::default().fg(Color::White),
            ),
            if app.bg_syncing {
                let spinner = ["⠋","⠙","⠹","⠸","⠼","⠴","⠦","⠧","⠇","⠏"];
                let s = spinner[app.tick_count % spinner.len()];
                Span::styled(format!("  {} syncing…", s), Style::default().fg(Color::Yellow))
            } else {
                Span::raw("")
            },
        ]),
    ];
    let stats_widget = Paragraph::new(stats)
        .block(Block::default().borders(Borders::ALL).title(" Stats "))
        .wrap(Wrap { trim: false });
    frame.render_widget(stats_widget, chunks[0]);

    // Navigation help
    let help = Paragraph::new(vec![
        Line::from(""),
        Line::from(vec![
            Span::styled("  [b]", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
            Span::raw(" Browse by category   "),
            Span::styled("[/]", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
            Span::raw(" Search repos"),
        ]),
        Line::from(vec![
            Span::styled("  [i]", Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD)),
            Span::raw(" AI Search            "),
            Span::styled("[s]", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
            Span::raw(" Settings"),
        ]),
        Line::from(vec![
            Span::styled("  [u]", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
            Span::raw(" Sync now             "),
            Span::styled("[q]", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
            Span::raw(" Quit"),
        ]),
    ])
    .block(Block::default().borders(Borders::ALL).title(" Navigation "));
    frame.render_widget(help, chunks[1]);
}

// ── browse ────────────────────────────────────────────────────────────────────

fn draw_browse(frame: &mut Frame, app: &mut App) {
    let area = frame.area();
    let block = title_block("Browse — [←/→] switch pane  [↑/↓] navigate  [Enter] open URL  [q] back");
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(30), Constraint::Percentage(70)])
        .split(inner);

    // Categories list
    let cat_items: Vec<ListItem> = app
        .categories
        .iter()
        .map(|c| {
            let icon = if c.category_type == "language" { "◈" } else { "#" };
            ListItem::new(format!("{} {} ({})", icon, c.name, c.count))
        })
        .collect();

    let mut cat_state = ListState::default();
    cat_state.select(app.selected_category);

    let cat_list = List::new(cat_items)
        .block(title_block("Categories"))
        .highlight_style(highlight_style())
        .highlight_symbol("▶ ");
    frame.render_stateful_widget(cat_list, chunks[0], &mut cat_state);
    app.category_list_state = cat_state;

    // Repos list + detail
    let repo_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(60), Constraint::Percentage(40)])
        .split(chunks[1]);

    let repo_items: Vec<ListItem> = app
        .displayed_repos
        .iter()
        .map(|r| {
            ListItem::new(vec![Line::from(vec![
                Span::styled(
                    &r.name,
                    Style::default().fg(Color::White).add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    format!("  ★ {}", fmt_stars(r.stars_count)),
                    Style::default().fg(Color::Yellow),
                ),
            ])])
        })
        .collect();

    let mut repo_state = ListState::default();
    repo_state.select(app.selected_repo);

    let repo_list = List::new(repo_items)
        .block(title_block("Repositories"))
        .highlight_style(highlight_style())
        .highlight_symbol("▶ ");
    frame.render_stateful_widget(repo_list, repo_chunks[0], &mut repo_state);
    app.repo_list_state = repo_state;

    // Detail panel
    let detail = if let Some(idx) = app.selected_repo {
        if let Some(repo) = app.displayed_repos.get(idx) {
            let topics = repo.topics();

            let lines = vec![
                Line::from(vec![
                    Span::styled("Name    : ", Style::default().fg(Color::DarkGray)),
                    Span::styled(repo.full_name.clone(), Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
                ]),
                Line::from(vec![
                    Span::styled("Lang    : ", Style::default().fg(Color::DarkGray)),
                    Span::styled(
                        repo.language.clone().unwrap_or_else(|| "—".to_string()),
                        Style::default().fg(Color::Cyan),
                    ),
                ]),
                Line::from(vec![
                    Span::styled("Topics  : ", Style::default().fg(Color::DarkGray)),
                    Span::styled(topics.join(", "), Style::default().fg(Color::Magenta)),
                ]),
                Line::from(vec![
                    Span::styled("URL     : ", Style::default().fg(Color::DarkGray)),
                    Span::styled(repo.url.clone(), Style::default().fg(Color::Yellow).add_modifier(Modifier::UNDERLINED)),
                ]),
                Line::from(""),
                Line::from(vec![
                    Span::styled("Desc    : ", Style::default().fg(Color::DarkGray)),
                    Span::styled(
                        repo.description.clone().unwrap_or_else(|| "No description".to_string()),
                        Style::default().fg(Color::White),
                    ),
                ]),
            ];
            Paragraph::new(lines).wrap(Wrap { trim: true })
        } else {
            Paragraph::new("Select a repository to see details.")
        }
    } else {
        Paragraph::new("Select a repository to see details.")
    };
    let detail_block = title_block("Detail  [Enter] open in browser");
    frame.render_widget(detail.block(detail_block), repo_chunks[1]);
}

// ── search ────────────────────────────────────────────────────────────────────

fn draw_search(frame: &mut Frame, app: &mut App) {
    let area = frame.area();
    let block = title_block("Search  [↑/↓] navigate  [Enter] open URL  [Esc] back");
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(0)])
        .split(inner);

    // Search input
    let input = Paragraph::new(format!("  {}_", app.search_query))
        .style(Style::default().fg(Color::White))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Query ")
                .border_style(Style::default().fg(Color::Yellow)),
        );
    frame.render_widget(input, chunks[0]);

    // Results
    let result_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(60), Constraint::Percentage(40)])
        .split(chunks[1]);

    let repo_items: Vec<ListItem> = app
        .search_results
        .iter()
        .map(|r| {
            ListItem::new(vec![Line::from(vec![
                Span::styled(&r.name, Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
                Span::styled(
                    format!("  {} ", r.language.as_deref().unwrap_or("")),
                    Style::default().fg(Color::Cyan),
                ),
                Span::styled(format!("★ {}", fmt_stars(r.stars_count)), Style::default().fg(Color::Yellow)),
            ])])
        })
        .collect();

    let mut repo_state = ListState::default();
    repo_state.select(app.selected_repo);

    let results_title = format!("Results ({})", app.search_results.len());
    let result_list = List::new(repo_items)
        .block(title_block(&results_title))
        .highlight_style(highlight_style())
        .highlight_symbol("▶ ");
    frame.render_stateful_widget(result_list, result_chunks[0], &mut repo_state);
    app.repo_list_state = repo_state;

    // Detail panel
    let detail = if let Some(idx) = app.selected_repo {
        if let Some(repo) = app.search_results.get(idx) {
            Paragraph::new(vec![
                Line::from(vec![
                    Span::styled("Name : ", Style::default().fg(Color::DarkGray)),
                    Span::styled(&repo.full_name, Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
                ]),
                Line::from(vec![
                    Span::styled("URL  : ", Style::default().fg(Color::DarkGray)),
                    Span::styled(&repo.url, Style::default().fg(Color::Yellow)),
                ]),
                Line::from(""),
                Line::from(vec![
                    Span::styled("Desc : ", Style::default().fg(Color::DarkGray)),
                    Span::styled(
                        repo.description.as_deref().unwrap_or("No description"),
                        Style::default().fg(Color::White),
                    ),
                ]),
            ])
            .wrap(Wrap { trim: true })
        } else {
            Paragraph::new("")
        }
    } else {
        Paragraph::new("Type to search…")
    };
    frame.render_widget(detail.block(title_block("Detail")), result_chunks[1]);
}

// ── settings ──────────────────────────────────────────────────────────────────

fn draw_settings(frame: &mut Frame, app: &App) {
    let area = frame.area();
    let block = title_block("Settings  [q/Esc] back");
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let auto = if app.config.auto_update { "✓ ON " } else { "  OFF" };
    let client_id_display = app
        .config
        .client_id()
        .map(|id| {
            if id.len() > 8 {
                format!("{}…", &id[..8])
            } else {
                id.to_string()
            }
        })
        .unwrap_or_else(|| "(not set)".to_string());

    let ai_key_display = app
        .config
        .openai_api_key
        .as_deref()
        .map(|k| {
            if k.len() > 8 { format!("{}…", &k[..8]) } else { k.to_string() }
        })
        .unwrap_or_else(|| "(not set)".to_string());

    let copilot_on = app.config.use_copilot;
    let copilot_label = if copilot_on { "✓ ON " } else { "  OFF" };

    let (base_url, model) = if copilot_on {
        (
            COPILOT_BASE_URL,
            app.config.openai_model.as_deref().unwrap_or(COPILOT_DEFAULT_MODEL),
        )
    } else {
        (
            app.config.openai_base_url.as_deref().unwrap_or(DEFAULT_BASE_URL),
            app.config.openai_model.as_deref().unwrap_or(DEFAULT_MODEL),
        )
    };

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(3)])
        .split(inner);

    let mut lines = vec![
        Line::from(""),
        Line::from(vec![
            Span::styled("  [a] Auto-update on startup : ", Style::default().fg(Color::White)),
            Span::styled(
                auto,
                if app.config.auto_update {
                    Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(Color::Red)
                },
            ),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("      OAuth Client ID        : ", Style::default().fg(Color::White)),
            Span::styled(&client_id_display, Style::default().fg(Color::Cyan)),
        ]),
        Line::from(""),
        Line::from(Span::styled("  ── AI Search ──────────────────────────────────────", Style::default().fg(Color::DarkGray))),
        Line::from(""),
        Line::from(vec![
            Span::styled("  [c] GitHub Copilot         : ", Style::default().fg(Color::White)),
            Span::styled(
                copilot_label,
                if copilot_on {
                    Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(Color::DarkGray)
                },
            ),
            if copilot_on {
                Span::styled("  (uses your GitHub token)", Style::default().fg(Color::DarkGray))
            } else {
                Span::raw("")
            },
        ]),
        Line::from(vec![
            Span::styled(
                if copilot_on { "  [p] Copilot GitHub token   : " } else { "      Copilot GitHub token   : " },
                Style::default().fg(if copilot_on { Color::White } else { Color::DarkGray }),
            ),
            Span::styled(
                app.config.copilot_github_token.as_deref()
                    .map(|k| if k.len() > 8 { format!("{}…", &k[..8]) } else { k.to_string() })
                    .unwrap_or_else(|| "(not set)".to_string()),
                if !copilot_on {
                    Style::default().fg(Color::DarkGray)
                } else if app.config.copilot_github_token.is_some() {
                    Style::default().fg(Color::Green)
                } else {
                    Style::default().fg(Color::Red)
                },
            ),
        ]),
        Line::from(vec![
            Span::styled(
                "      Tip: create a PAT at ",
                Style::default().fg(Color::DarkGray),
            ),
            Span::styled(
                "github.com/settings/tokens",
                Style::default().fg(Color::Cyan),
            ),
            Span::styled(
                " (no scope needed)",
                Style::default().fg(Color::DarkGray),
            ),
        ]),
        Line::from(vec![
            Span::styled("  [k] OpenAI API key         : ", Style::default().fg(
                if copilot_on { Color::DarkGray } else { Color::White }
            )),
            Span::styled(
                if copilot_on { "—" } else { &ai_key_display },
                if copilot_on {
                    Style::default().fg(Color::DarkGray)
                } else if app.config.openai_api_key.is_some() {
                    Style::default().fg(Color::Green)
                } else {
                    Style::default().fg(Color::Red)
                },
            ),
        ]),
        Line::from(vec![
            Span::styled("      Base URL               : ", Style::default().fg(Color::White)),
            Span::styled(base_url, Style::default().fg(Color::Cyan)),
        ]),
        Line::from(vec![
            Span::styled("      Model             [m]  : ", Style::default().fg(Color::White)),
            Span::styled(model, Style::default().fg(Color::Cyan)),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("  [l] Log out", Style::default().fg(Color::White)),
            Span::styled(" (clears stored token)", Style::default().fg(Color::DarkGray)),
        ]),
    ];

    if app.settings_editing_key {
        lines.push(Line::from(""));
        let field_label = if app.settings_editing_field == "copilot" {
            "Editing Copilot GitHub token — Enter to save, Esc to cancel"
        } else if app.settings_editing_field == "model" {
            "Editing model name — Enter to save, Esc to cancel"
        } else {
            "Editing OpenAI API key — Enter to save, Esc to cancel"
        };
        lines.push(Line::from(Span::styled(
            format!("  {}", field_label),
            Style::default().fg(Color::Yellow),
        )));
    }

    frame.render_widget(Paragraph::new(lines), chunks[0]);

    if app.settings_editing_key {
        let input_display = format!("  {}▌", app.settings_key_input);
        let field_title = if app.settings_editing_field == "copilot" {
            " Copilot GitHub Token "
        } else if app.settings_editing_field == "model" {
            " Custom Model Name "
        } else {
            " OpenAI API Key "
        };
        let input = Paragraph::new(input_display)
            .style(Style::default().fg(Color::White))
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(field_title)
                    .border_style(Style::default().fg(Color::Yellow)),
            );
        frame.render_widget(input, chunks[1]);
    }

    // Model picker overlay
    if app.settings_model_picking {
        let popup_area = centered_rect(62, KNOWN_MODELS.len() as u16 + 5, area);
        frame.render_widget(Clear, popup_area);

        let items: Vec<ListItem> = KNOWN_MODELS
            .iter()
            .enumerate()
            .map(|(i, (id, desc))| {
                let selected = i == app.settings_model_cursor;
                let current = app.config.openai_model.as_deref()
                    .unwrap_or(COPILOT_DEFAULT_MODEL) == *id;
                let marker = if current { "●" } else { " " };
                let line = Line::from(vec![
                    Span::styled(
                        format!(" {} {:<36}", marker, id),
                        if selected {
                            Style::default().fg(Color::Black).bg(Color::Cyan).add_modifier(Modifier::BOLD)
                        } else if current {
                            Style::default().fg(Color::Cyan)
                        } else {
                            Style::default().fg(Color::White)
                        },
                    ),
                    Span::styled(
                        format!(" {}", desc),
                        if selected {
                            Style::default().fg(Color::Black).bg(Color::Cyan)
                        } else {
                            Style::default().fg(Color::DarkGray)
                        },
                    ),
                ]);
                ListItem::new(line)
            })
            .chain(std::iter::once({
                let selected = app.settings_model_cursor == KNOWN_MODELS.len();
                ListItem::new(Line::from(Span::styled(
                    "   ✎ Custom model name...",
                    if selected {
                        Style::default().fg(Color::Black).bg(Color::Yellow).add_modifier(Modifier::BOLD)
                    } else {
                        Style::default().fg(Color::Yellow)
                    },
                )))
            }))
            .collect();

        let mut list_state = ListState::default();
        list_state.select(Some(app.settings_model_cursor));

        let picker = List::new(items)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(" Select Model  [↑/↓] navigate  [Enter] select  [Esc] cancel ")
                    .border_style(Style::default().fg(Color::Cyan)),
            );
        frame.render_stateful_widget(picker, popup_area, &mut list_state);
    }
}

fn centered_rect(width: u16, height: u16, area: Rect) -> Rect {
    let x = area.x + area.width.saturating_sub(width) / 2;
    let y = area.y + area.height.saturating_sub(height) / 2;
    Rect {
        x,
        y,
        width: width.min(area.width),
        height: height.min(area.height),
    }
}

// ── AI search ─────────────────────────────────────────────────────────────────

fn draw_ai_search(frame: &mut Frame, app: &mut App) {
    let area = frame.area();
    let hint = if app.ai_focus_query {
        "AI Search  [Enter] submit  [Tab] → results  [Esc] back"
    } else {
        "AI Search  [↑/↓] navigate  [Enter] open URL  [Esc] → query"
    };
    let block = title_block(hint);
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let spinner = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(0)])
        .split(inner);

    // Query input
    let cursor = if app.ai_focus_query { "▌" } else { "" };
    let query_display = format!("  {}{}", app.ai_query, cursor);
    let query_border_color = if app.ai_focus_query {
        Color::Yellow
    } else {
        Color::DarkGray
    };
    let input = Paragraph::new(query_display)
        .style(Style::default().fg(Color::White))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Natural-language query (Enter to search) ")
                .border_style(Style::default().fg(query_border_color)),
        );
    frame.render_widget(input, chunks[0]);

    // Status / results area
    let result_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(60), Constraint::Percentage(40)])
        .split(chunks[1]);

    if app.ai_loading {
        let spin = spinner[app.tick_count % spinner.len()];
        let loading = Paragraph::new(format!("  {} Querying AI…", spin))
            .style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))
            .block(title_block("Results"));
        frame.render_widget(loading, result_chunks[0]);
        frame.render_widget(
            Paragraph::new("").block(title_block("Detail")),
            result_chunks[1],
        );
        return;
    }

    if let Some(err) = &app.ai_error {
        let err_msg = Paragraph::new(vec![
            Line::from(""),
            Line::from(Span::styled(
                format!("  ✗ {}", err),
                Style::default().fg(Color::Red),
            )),
        ])
        .block(title_block("Results"))
        .wrap(Wrap { trim: true });
        frame.render_widget(err_msg, result_chunks[0]);
        frame.render_widget(
            Paragraph::new("").block(title_block("Detail")),
            result_chunks[1],
        );
        return;
    }

    // Results list
    let repo_items: Vec<ListItem> = app
        .ai_results
        .iter()
        .map(|r| {
            ListItem::new(vec![Line::from(vec![
                Span::styled(
                    &r.name,
                    Style::default().fg(Color::White).add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    format!("  {} ", r.language.as_deref().unwrap_or("")),
                    Style::default().fg(Color::Cyan),
                ),
                Span::styled(
                    format!("★ {}", fmt_stars(r.stars_count)),
                    Style::default().fg(Color::Yellow),
                ),
            ])])
        })
        .collect();

    let mut repo_state = ListState::default();
    repo_state.select(app.selected_repo);

    let results_title = if app.ai_results.is_empty() && !app.ai_query.is_empty() {
        "Results (0 — try a different query)".to_string()
    } else {
        format!("Results ({})", app.ai_results.len())
    };
    let result_list = List::new(repo_items)
        .block(title_block(&results_title))
        .highlight_style(highlight_style())
        .highlight_symbol("▶ ");
    frame.render_stateful_widget(result_list, result_chunks[0], &mut repo_state);
    app.repo_list_state = repo_state;

    // Detail panel
    let detail = if let Some(idx) = app.selected_repo {
        if let Some(repo) = app.ai_results.get(idx) {
            let topics = repo.topics();
            Paragraph::new(vec![
                Line::from(vec![
                    Span::styled("Name    : ", Style::default().fg(Color::DarkGray)),
                    Span::styled(
                        &repo.full_name,
                        Style::default().fg(Color::White).add_modifier(Modifier::BOLD),
                    ),
                ]),
                Line::from(vec![
                    Span::styled("Lang    : ", Style::default().fg(Color::DarkGray)),
                    Span::styled(
                        repo.language.clone().unwrap_or_else(|| "—".to_string()),
                        Style::default().fg(Color::Cyan),
                    ),
                ]),
                Line::from(vec![
                    Span::styled("Topics  : ", Style::default().fg(Color::DarkGray)),
                    Span::styled(topics.join(", "), Style::default().fg(Color::Magenta)),
                ]),
                Line::from(vec![
                    Span::styled("URL     : ", Style::default().fg(Color::DarkGray)),
                    Span::styled(
                        &repo.url,
                        Style::default().fg(Color::Yellow).add_modifier(Modifier::UNDERLINED),
                    ),
                ]),
                Line::from(""),
                Line::from(vec![
                    Span::styled("Desc    : ", Style::default().fg(Color::DarkGray)),
                    Span::styled(
                        repo.description.clone().unwrap_or_else(|| "No description".to_string()),
                        Style::default().fg(Color::White),
                    ),
                ]),
            ])
            .wrap(Wrap { trim: true })
        } else {
            Paragraph::new("Select a result to see details.")
        }
    } else if app.ai_query.is_empty() {
        Paragraph::new("Type a natural-language query and press Enter…")
    } else {
        Paragraph::new("Select a result to see details.")
    };
    frame.render_widget(detail.block(title_block("Detail  [Enter] open in browser")), result_chunks[1]);
}

// ── syncing ───────────────────────────────────────────────────────────────────

fn draw_syncing(frame: &mut Frame, app: &App) {
    let area = frame.area();
    let is_done = matches!(
        &app.sync_status,
        SyncStatus::Done(_) | SyncStatus::Error(_)
    );
    let title = if is_done {
        "Sync — press any key to continue"
    } else {
        "Syncing…"
    };
    let block = title_block(title);
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let spinner = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
    let spin_char = spinner[app.tick_count % spinner.len()];

    // Build log lines
    let mut items: Vec<ListItem> = app
        .sync_log
        .iter()
        .map(|e| {
            ListItem::new(Line::from(Span::styled(
                e.message.clone(),
                Style::default().fg(e.color),
            )))
        })
        .collect();

    // Append live spinner line if still running
    if !is_done {
        let spinner_line = match &app.sync_status {
            SyncStatus::FetchingStars(_) => format!("  {} Fetching…", spin_char),
            SyncStatus::Classifying => format!("  {} Classifying…", spin_char),
            _ => format!("  {} Working…", spin_char),
        };
        items.push(ListItem::new(Line::from(Span::styled(
            spinner_line,
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ))));
    }

    // Always scroll to bottom: offset so last item is visible
    let height = inner.height as usize;
    let total = items.len();
    let offset = total.saturating_sub(height);
    let mut state = ListState::default().with_offset(offset);

    let list = List::new(items);
    frame.render_stateful_widget(list, inner, &mut state);
}
