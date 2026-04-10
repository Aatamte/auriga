mod app;
mod config;
pub mod context;
mod helpers;
mod input;
mod skills_storage;
mod threads;
mod types;

use anyhow::Result;
use app::App;
use crossterm::event::{DisableMouseCapture, EnableMouseCapture};
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use crossterm::ExecutableCommand;
use orchestrator_core::{AgentId, Page};
use orchestrator_terminal::render_term;
use orchestrator_widgets::{RenderContext, Widget};
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
            "orchestrator": {
                "type": "http",
                "url": format!("http://127.0.0.1:{}", port),
                "autoApprove": ["list_agents", "send_message", "list_context", "get_context"]
            }
        }
    });
    std::fs::write(".mcp.json", serde_json::to_string_pretty(&config)?)?;
    Ok(())
}

fn main() -> Result<()> {
    // Init project config directory
    let config = config::init()?;

    // Logging: disabled for now, enable when needed
    let enable_logging = false;
    let _log_guard = if enable_logging {
        let log_dir = config::dir_path();
        let file_appender = tracing_appender::rolling::daily(&log_dir, "orchestrator.log");
        let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);
        tracing_subscriber::fmt()
            .with_writer(non_blocking)
            .with_env_filter("info")
            .init();
        Some(guard)
    } else {
        None
    };

    enable_raw_mode()?;
    let mut out = stdout();
    out.execute(EnterAlternateScreen)?;
    out.execute(EnableMouseCapture)?;

    // Apply font size
    apply_font_size(&mut out, config.font_size);

    // Background threads
    let input_rx = threads::start_input_thread();
    let (file_tx, file_rx) = mpsc::channel();
    let _watcher = threads::start_file_watcher(file_tx);
    let (diff_tx, diff_rx) = threads::start_diff_thread();

    // MCP server
    let mcp = orchestrator_mcp::start_mcp_server(config.mcp_port)?;
    if let Err(e) = write_mcp_json(mcp.port) {
        tracing::warn!(error = %e, "failed to write .mcp.json");
    }

    // Storage
    let db_path = config::dir_path().join("orchestrator.db");
    let storage = orchestrator_storage::start_storage_thread(db_path.clone())?;
    let db_reader = orchestrator_storage::Database::open(&db_path)?;

    // Claude log watcher
    let claude_watcher = match (
        orchestrator_claude_log::claude_project_dir(),
        orchestrator_claude_log::claude_sessions_dir(),
    ) {
        (Some(project_dir), Some(sessions_dir)) => {
            orchestrator_claude_log::start_claude_watcher(project_dir, sessions_dir).ok()
        }
        _ => None,
    };

    let backend = CrosstermBackend::new(stdout());
    let mut terminal = Terminal::new(backend)?;

    // Generation thread for native mode (managed agent loop).
    // Each request carries the agent's provider name so we can dispatch
    // to the right backend (claude, codex, ...).
    let (gen_req_tx, gen_req_rx) = mpsc::channel::<(
        orchestrator_core::AgentId,
        String,
        orchestrator_agent::GenerateRequest,
    )>();
    let (gen_resp_tx, gen_resp_rx) = mpsc::channel();
    thread::spawn(move || {
        while let Ok((agent_id, provider_name, request)) = gen_req_rx.recv() {
            let provider = orchestrator_agent::providers::resolve(&provider_name);
            let result: Result<orchestrator_agent::GenerateResponse, String> =
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
        &config,
    );

    // Load classifier configs from .agent-orchestrator/classifiers/
    if app::USE_CLASSIFIERS {
        app.load_classifier_configs();
    }

    // Register built-in skills
    app.register_default_skills();

    // Apply disabled classifiers from config
    if app::USE_CLASSIFIERS {
        for name in &config.disabled_classifiers {
            app.classifier_registry.set_enabled(name, false);
        }
    }

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
        app.poll_doctor_events();
        app.poll_claude_logs();
        app.poll_generate_responses();
        app.file_tree.refresh_caches();

        // Live-refresh pages when active
        if app::USE_CLASSIFIERS && app.focus.page == Page::Home {
            app.refresh_classifier_panel();
        }
        if app.focus.page == Page::Database {
            app.refresh_database();
        }
        if app::USE_CLASSIFIERS && app.focus.page == Page::Classifiers {
            app.refresh_classifiers();
        }
        if app.focus.page == Page::Prompts {
            app.refresh_prompts();
        }
        if app.focus.page == Page::Context {
            app.refresh_context();
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

            // Tab bar
            app.widgets
                .nav_bar
                .render(frame, nav_area, current_page, &hidden);
            app.last_nav_rect = nav_area;

            // Keybinds footer
            render_keybinds_footer(frame, hint_area);

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
                Page::Classifiers => {
                    app.widgets
                        .classifiers_page
                        .render(frame, content_area, &ctx);
                    app.last_cell_rects = Vec::new();
                }
                Page::Prompts => {
                    app.widgets.prompts_page.render(frame, content_area, &ctx);
                    app.last_cell_rects = Vec::new();
                }
                Page::Context => {
                    app.widgets.context_page.render(frame, content_area, &ctx);
                    app.last_cell_rects = Vec::new();
                }
                Page::Doctor => {
                    app.widgets.doctor_page.render(frame, content_area, &ctx);
                    app.last_cell_rects = Vec::new();
                }
            }
        })?;

        // Resize PTYs if pane size changed
        let current_pane_size = app.pane_size();
        if current_pane_size != last_pane_size {
            app.resize_all_ptys();
            last_pane_size = current_pane_size;
        }

        // Small sleep to avoid busy-spinning when nothing is happening
        thread::sleep(Duration::from_millis(8));
    }

    // Flush remaining traces before exit
    app.flush_all_traces();
    app.storage.shutdown();

    // Cleanup
    if let Err(e) = std::fs::remove_file(".mcp.json") {
        tracing::debug!(error = %e, "failed to clean up .mcp.json");
    }

    let mut out = stdout();
    out.execute(DisableMouseCapture)?;
    out.execute(LeaveAlternateScreen)?;
    disable_raw_mode()?;
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

    let spans = vec![
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

    frame.render_widget(Paragraph::new(Line::from(spans)), area);
}

fn apply_font_size(out: &mut impl std::io::Write, size: u16) {
    // xterm OSC 50 — widely supported (xterm, kitty, alacritty, WezTerm)
    let _ = out.write_all(format!("\x1b]50;size={}\x07", size).as_bytes());
    // iTerm2 proprietary sequence
    let _ = out.write_all(format!("\x1b]1337;SetFontSize={}\x07", size).as_bytes());
    let _ = out.flush();
}
