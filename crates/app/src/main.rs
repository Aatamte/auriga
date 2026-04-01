mod app;
mod config;
mod helpers;
mod input;
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
                "autoApprove": ["list_agents", "send_message"]
            }
        }
    });
    std::fs::write(".mcp.json", serde_json::to_string_pretty(&config)?)?;
    Ok(())
}

fn main() -> Result<()> {
    // Init project config directory
    let config = config::init()?;

    enable_raw_mode()?;
    let mut out = stdout();
    out.execute(EnterAlternateScreen)?;
    out.execute(EnableMouseCapture)?;

    // Background threads
    let input_rx = threads::start_input_thread();
    let (file_tx, file_rx) = mpsc::channel();
    let _watcher = threads::start_file_watcher(file_tx);
    let (diff_tx, diff_rx) = threads::start_diff_thread();

    // MCP server
    let mcp = orchestrator_mcp::start_mcp_server(config.mcp_port)?;
    let _ = write_mcp_json(mcp.port);

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

    let mut app = App::new(
        input_rx, file_rx, diff_tx, diff_rx, mcp.rx, storage, db_reader, db_path,
        claude_watcher, &config,
    );

    // Pre-compute layout rects so pane_size() works before first render
    {
        let size = terminal.size()?;
        let area = Rect::new(0, 0, size.width, size.height);
        let [nav_area, content_area] =
            Layout::vertical([Constraint::Length(1), Constraint::Min(0)]).areas(area);
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
        app.poll_claude_logs();
        app.file_tree.refresh_caches();

        // Live-refresh pages when active
        if app.focus.page == Page::Database {
            app.refresh_database();
        }
        if app.focus.page == Page::Classifiers {
            app.refresh_classifiers();
        }

        // Render
        terminal.draw(|frame| {
            let area = frame.area();
            let current_page = app.focus.page;

            // Split: tab bar (1 row) + content
            let [nav_area, content_area] =
                Layout::vertical([Constraint::Length(1), Constraint::Min(0)]).areas(area);

            // Tab bar
            app.widgets.nav_bar.render(frame, nav_area, current_page);
            app.last_nav_rect = nav_area;

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
            };

            // Page content
            match current_page {
                Page::Home => {
                    let cell_rects = app.grid.compute_rects(content_area);
                    for cell_rect in &cell_rects {
                        if let Some(widget) = app.widgets.get_mut(&cell_rect.widget) {
                            widget.render(frame, cell_rect.rect, &ctx);
                        }
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
                    app.widgets.classifiers_page.render(frame, content_area, &ctx);
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
    let _ = std::fs::remove_file(".mcp.json");

    let mut out = stdout();
    out.execute(DisableMouseCapture)?;
    out.execute(LeaveAlternateScreen)?;
    disable_raw_mode()?;
    Ok(())
}
