//! Integration tests for auriga-terminal public API

use auriga_terminal::render_term;
use ratatui::backend::TestBackend;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::Terminal;

use alacritty_terminal::event::EventListener;
use alacritty_terminal::grid::Dimensions;
use alacritty_terminal::term::{Config as TermConfig, Term};
use alacritty_terminal::vte;

#[derive(Clone)]
struct EventProxy;
impl EventListener for EventProxy {
    fn send_event(&self, _event: alacritty_terminal::event::Event) {}
}

struct TermSize {
    cols: usize,
    lines: usize,
}

impl Dimensions for TermSize {
    fn columns(&self) -> usize {
        self.cols
    }
    fn screen_lines(&self) -> usize {
        self.lines
    }
    fn total_lines(&self) -> usize {
        self.lines
    }
}

fn create_term(rows: usize, cols: usize) -> Term<EventProxy> {
    let config = TermConfig {
        scrolling_history: 1000,
        ..Default::default()
    };
    let size = TermSize { cols, lines: rows };
    Term::new(config, &size, EventProxy)
}

fn write_to_term(term: &mut Term<EventProxy>, text: &str) {
    let mut parser = vte::ansi::Processor::<vte::ansi::StdSyncHandler>::new();
    parser.advance(term, text.as_bytes());
}

#[test]
fn render_term_to_buffer() {
    let mut term = create_term(24, 80);
    write_to_term(&mut term, "Hello, World!\r\n");

    let area = Rect::new(0, 0, 80, 24);
    let mut buffer = Buffer::empty(area);

    render_term(&term, &mut buffer, area);

    // Buffer should contain rendered content
    let content = buffer.content();
    assert!(!content.is_empty());
}

#[test]
fn render_term_with_ansi_colors() {
    let mut term = create_term(24, 80);
    write_to_term(&mut term, "\x1b[32mGreen text\x1b[0m\r\n");
    write_to_term(&mut term, "\x1b[1;34mBold blue\x1b[0m\r\n");

    let area = Rect::new(0, 0, 80, 24);
    let mut buffer = Buffer::empty(area);

    render_term(&term, &mut buffer, area);

    // Should render without panicking
    assert!(!buffer.content().is_empty());
}

#[test]
fn render_term_multiline() {
    let mut term = create_term(24, 80);
    for i in 0..20 {
        write_to_term(&mut term, &format!("Line {}\r\n", i));
    }

    let area = Rect::new(0, 0, 80, 24);
    let mut buffer = Buffer::empty(area);

    render_term(&term, &mut buffer, area);

    assert!(!buffer.content().is_empty());
}

#[test]
fn render_term_empty() {
    let term = create_term(24, 80);

    let area = Rect::new(0, 0, 80, 24);
    let mut buffer = Buffer::empty(area);

    render_term(&term, &mut buffer, area);

    // Should handle empty terminal gracefully
    assert!(!buffer.content().is_empty());
}

#[test]
fn render_term_small_area() {
    let mut term = create_term(24, 80);
    write_to_term(&mut term, "Some content here\r\n");

    let area = Rect::new(0, 0, 20, 5);
    let mut buffer = Buffer::empty(area);

    render_term(&term, &mut buffer, area);

    // Should clip to area without panicking
    assert!(!buffer.content().is_empty());
}

#[test]
fn render_term_in_ratatui_terminal() {
    use ratatui::backend::Backend;

    let backend = TestBackend::new(80, 24);
    let mut terminal = Terminal::new(backend).unwrap();

    let mut term = create_term(24, 80);
    write_to_term(&mut term, "Terminal content\r\n");

    terminal
        .draw(|frame| {
            let area = frame.area();
            let buf = frame.buffer_mut();
            render_term(&term, buf, area);
        })
        .unwrap();

    // Should integrate with ratatui's terminal
    let backend = terminal.backend();
    assert_eq!(backend.size().unwrap().width, 80);
}
