mod app;
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
use orchestrator_core::AgentId;
use orchestrator_terminal::render_term;
use orchestrator_widgets::RenderContext;
use ratatui::backend::CrosstermBackend;
use ratatui::layout::Rect;
use ratatui::Terminal;
use std::io::stdout;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

fn main() -> Result<()> {
    enable_raw_mode()?;
    let mut out = stdout();
    out.execute(EnterAlternateScreen)?;
    out.execute(EnableMouseCapture)?;

    // Background threads
    let input_rx = threads::start_input_thread();
    let (file_tx, file_rx) = mpsc::channel();
    let _watcher = threads::start_file_watcher(file_tx);
    let (diff_tx, diff_rx) = threads::start_diff_thread();

    let backend = CrosstermBackend::new(stdout());
    let mut terminal = Terminal::new(backend)?;

    let mut app = App::new(input_rx, file_rx, diff_tx, diff_rx);

    // Pre-compute layout rects so pane_size() works before first render
    {
        let size = terminal.size()?;
        let area = Rect::new(0, 0, size.width, size.height);
        app.last_cell_rects = app.grid.compute_rects(area);
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
        app.file_tree.refresh_caches();

        // Render
        terminal.draw(|frame| {
            let area = frame.area();
            let cell_rects = app.grid.compute_rects(area);

            let terms = &app.terms;
            let term_renderer = |id: AgentId, buf: &mut ratatui::buffer::Buffer, area: Rect| {
                if let Some(term) = terms.get(&id) {
                    render_term(term, buf, area);
                }
            };

            let ctx = RenderContext {
                agents: &app.agents,
                focus: &app.focus,
                file_tree: &app.file_tree,
                render_term: &term_renderer,
            };

            for cell_rect in &cell_rects {
                if let Some(widget) = app.widgets.get_mut(&cell_rect.widget) {
                    widget.render(frame, cell_rect.rect, &ctx);
                }
            }

            app.last_cell_rects = cell_rects;
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

    let mut out = stdout();
    out.execute(DisableMouseCapture)?;
    out.execute(LeaveAlternateScreen)?;
    disable_raw_mode()?;
    Ok(())
}
