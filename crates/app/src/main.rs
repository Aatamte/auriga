mod app;
mod config;
mod helpers;
mod input;
mod skills_storage;
mod threads;
mod types;

use anyhow::Result;
use app::App;
use auriga_core::{AgentId, Page};
use auriga_terminal::render_term;
use auriga_widgets::{RenderContext, Widget};
use crossterm::event::{DisableMouseCapture, EnableMouseCapture};
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use crossterm::ExecutableCommand;
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::Terminal;
use std::io::stdout;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

fn write_mcp_json(port: u16) -> Result<()> {
    let config = serde_json::json!({
        "mcpServers": {
            "auriga": {
                "type": "http",
                "url": format!("http://127.0.0.1:{}", port),
                "autoApprove": ["list_agents", "send_message"]
            }
        }
    });
    std::fs::write(".mcp.json", serde_json::to_string_pretty(&config)?)?;
    Ok(())
}

/// Restores terminal to normal state. Safe to call multiple times.
fn restore_terminal() {
    let mut out = stdout();
    let _ = out.execute(DisableMouseCapture);
    let _ = out.execute(LeaveAlternateScreen);
    let _ = disable_raw_mode();
}

/// Guard that restores terminal state on drop.
struct TerminalGuard;

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        restore_terminal();
    }
}

fn main() -> Result<()> {
    // Pre-terminal initialization (safe to fail without cleanup)
    let config = config::init()?;

    // Logging: disabled for now, enable when needed
    let enable_logging = false;
    let _log_guard = if enable_logging {
        let log_dir = config::dir_path();
        let file_appender = tracing_appender::rolling::daily(&log_dir, "auriga.log");
        let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);
        tracing_subscriber::fmt()
            .with_writer(non_blocking)
            .with_env_filter("info")
            .init();
        Some(guard)
    } else {
        None
    };

    // Enter terminal raw mode
    enable_raw_mode()?;
    let mut out = stdout();
    out.execute(EnterAlternateScreen)?;
    out.execute(EnableMouseCapture)?;

    // Guard ensures terminal is restored on any exit (error, panic, or normal)
    let _terminal_guard = TerminalGuard;

    // Run the app - any error will trigger guard cleanup
    let result = run_app(&config);

    // Normal shutdown cleanup (guard will also run, but restore_terminal is idempotent)
    if let Err(e) = std::fs::remove_file(".mcp.json") {
        tracing::debug!(error = %e, "failed to clean up .mcp.json");
    }

    result
}

fn run_app(config: &config::Config) -> Result<()> {
    // Apply font size
    apply_font_size(&mut stdout(), config.font_size);

    // Background threads
    let input_rx = threads::start_input_thread();
    let (file_tx, file_rx) = mpsc::channel();
    let _watcher = threads::start_file_watcher(file_tx);
    let (diff_tx, diff_rx) = threads::start_diff_thread();

    // MCP server
    let mcp = auriga_mcp::start_mcp_server(config.mcp_port)?;
    if let Err(e) = write_mcp_json(mcp.port) {
        tracing::warn!(error = %e, "failed to write .mcp.json");
    }

    // Storage
    let db_path = config::dir_path().join("auriga.db");
    let storage = auriga_storage::start_storage_thread(db_path.clone())?;
    let db_reader = auriga_storage::Database::open(&db_path)?;

    // Claude log watcher
    let claude_watcher = match (
        auriga_claude_log::claude_project_dir(),
        auriga_claude_log::claude_sessions_dir(),
    ) {
        (Some(project_dir), Some(sessions_dir)) => {
            auriga_claude_log::start_claude_watcher(project_dir, sessions_dir).ok()
        }
        _ => None,
    };

    let backend = CrosstermBackend::new(stdout());
    let mut terminal = Terminal::new(backend)?;

    // Generation thread for native mode (managed agent loop).
    // Each request carries the agent's provider name so we can dispatch
    // to the right backend (claude, codex, ...).
    let (gen_req_tx, gen_req_rx) =
        mpsc::channel::<(auriga_core::AgentId, String, auriga_agent::GenerateRequest)>();
    let (gen_resp_tx, gen_resp_rx) = mpsc::channel();
    thread::spawn(move || {
        while let Ok((agent_id, provider_name, request)) = gen_req_rx.recv() {
            let provider = auriga_agent::providers::resolve(&provider_name);
            let result: Result<auriga_agent::GenerateResponse, String> =
                match provider.generate(&request) {
                    Ok(resp) => Ok(resp),
                    Err(e) => Err(e.to_string()),
                };
            if gen_resp_tx.send((agent_id, result)).is_err() {
                break;
            }
        }
    });

    let mut app = App::new(
        input_rx,
        file_rx,
        diff_tx,
        diff_rx,
        mcp.rx,
        storage,
        db_reader,
        db_path,
        claude_watcher,
        gen_req_tx,
        gen_resp_rx,
        config,
    );

    // Register built-in skills
    app.register_default_skills();
    app.refresh_settings();

    // Pre-compute layout rects so pane_size() works before first render
    {
        let size = terminal.size()?;
        let area = Rect::new(0, 0, size.width, size.height);
        let [nav_area, content_area, _hint_area] = Layout::vertical([
            Constraint::Length(1),
            Constraint::Min(0),
            Constraint::Length(1),
        ])
        .areas(area);
        app.last_nav_rect = nav_area;
        app.last_cell_rects = app.grid.compute_rects(content_area);
    }

    let mut last_pane_size: (u16, u16) = app.pane_size();
    let mut last_agent_count: usize = app.agents.list().len();
    let mut last_pane_mode = app.widgets.agent_pane.mode;

    while app.running {
        // Drain all channels non-blocking
        app.poll_input();
        if !app.running {
            break;
        }
        app.poll_pty_output();
        app.poll_file_events();
        app.poll_diff_results();
        app.poll_mcp_requests();
        app.poll_claude_logs();
        app.poll_generate_responses();
        app.file_tree.refresh_caches();

        // Live-refresh pages when active
        if app.focus.page == Page::Database {
            app.refresh_database();
        }
        if app.focus.page == Page::Prompts {
            app.refresh_prompts();
        }
        if app.focus.page == Page::Settings {
            app.refresh_settings();
        }

        // Render
        terminal.draw(|frame| {
            let area = frame.area();
            let current_page = app.focus.page;

            // Split: tab bar (1 row) + content + keybinds footer (1 row)
            let [nav_area, content_area, hint_area] = Layout::vertical([
                Constraint::Length(1),
                Constraint::Min(0),
                Constraint::Length(1),
            ])
            .areas(area);

            let hidden = app::hidden_pages();
            let terms = &app.terms;
            let term_renderer = |id: AgentId, buf: &mut ratatui::buffer::Buffer, area: Rect| {
                if let Some(term) = terms.get(&id) {
                    render_term(term, buf, area);
                }
            };
            let ctx = RenderContext {
                agents: &app.agents,
                turns: &app.turns,
                traces: &app.traces,
                focus: &app.focus,
                file_tree: &app.file_tree,
                render_term: &term_renderer,
                hidden_pages: &hidden,
            };

            // Tab bar
            app.widgets
                .nav_bar
                .render(frame, nav_area, current_page, &ctx);
            app.last_nav_rect = nav_area;

            // Keybinds footer
            render_keybinds_footer(frame, hint_area);

            // Page content
            match current_page {
                Page::Home => {
                    let cell_rects = app.grid.compute_rects(content_area);
                    for cell_rect in &cell_rects {
                        app.widgets
                            .get_mut(cell_rect.widget)
                            .render(frame, cell_rect.rect, &ctx);
                    }
                    app.last_cell_rects = cell_rects;
                }
                Page::Settings => {
                    app.widgets.settings_page.render(frame, content_area, &ctx);
                    app.last_cell_rects = Vec::new();
                }
                Page::Database => {
                    app.widgets.database_page.render(frame, content_area, &ctx);
                    app.last_cell_rects = Vec::new();
                }
                Page::Prompts => {
                    app.widgets.prompts_page.render(frame, content_area, &ctx);
                    app.last_cell_rects = Vec::new();
                }
            }
        })?;

        // Resize PTYs if pane size, mode, or agent count changed
        let current_pane_size = app.pane_size();
        let current_agent_count = app.agents.list().len();
        let current_pane_mode = app.widgets.agent_pane.mode;
        if current_pane_size != last_pane_size
            || current_agent_count != last_agent_count
            || current_pane_mode != last_pane_mode
        {
            app.resize_all_ptys();
            last_pane_size = current_pane_size;
            last_agent_count = current_agent_count;
            last_pane_mode = current_pane_mode;
        }

        // Small sleep to avoid busy-spinning when nothing is happening
        thread::sleep(Duration::from_millis(8));
    }

    // Flush remaining traces before exit
    app.flush_all_traces();
    app.storage.shutdown();

    Ok(())
}

fn render_keybinds_footer(frame: &mut ratatui::Frame, area: Rect) {
    use ratatui::style::{Color, Modifier, Style};
    use ratatui::text::{Line, Span};
    use ratatui::widgets::Paragraph;

    let key = Style::default()
        .fg(Color::Cyan)
        .add_modifier(Modifier::BOLD);
    let desc = Style::default().fg(Color::DarkGray);
    let sep = Span::styled("   ", desc);

    let keybinds = vec![
        Span::raw(" "),
        Span::styled("^T", key),
        Span::raw(" "),
        Span::styled("terminal", desc),
        sep.clone(),
        Span::styled("^L", key),
        Span::raw(" "),
        Span::styled("claude", desc),
        sep.clone(),
        Span::styled("^O", key),
        Span::raw(" "),
        Span::styled("codex", desc),
        sep.clone(),
        Span::styled("^W", key),
        Span::raw(" "),
        Span::styled("close", desc),
        sep.clone(),
        Span::styled("^B", key),
        Span::raw(" "),
        Span::styled("grid/focus", desc),
        sep.clone(),
        Span::styled("^Q", key),
        Span::raw(" "),
        Span::styled("quit", desc),
    ];

    let version = format!("v{} ", env!("CARGO_PKG_VERSION"));

    let [left_area, right_area] =
        Layout::horizontal([Constraint::Min(0), Constraint::Length(version.len() as u16)])
            .areas(area);

    frame.render_widget(Paragraph::new(Line::from(keybinds)), left_area);
    frame.render_widget(Paragraph::new(Span::styled(version, desc)), right_area);
}

fn apply_font_size(out: &mut impl std::io::Write, size: u16) {
    // xterm OSC 50 — widely supported (xterm, kitty, alacritty, WezTerm)
    let _ = out.write_all(format!("\x1b]50;size={}\x07", size).as_bytes());
    // iTerm2 proprietary sequence
    let _ = out.write_all(format!("\x1b]1337;SetFontSize={}\x07", size).as_bytes());
    let _ = out.flush();
}
