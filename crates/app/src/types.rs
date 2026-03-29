use alacritty_terminal::event::EventListener;
use alacritty_terminal::grid::Dimensions;
use std::path::PathBuf;

/// No-op event listener for alacritty_terminal.
#[derive(Clone)]
pub struct EventProxy;
impl EventListener for EventProxy {
    fn send_event(&self, _event: alacritty_terminal::event::Event) {}
}

/// Terminal size for alacritty_terminal.
pub struct TermSize {
    pub cols: usize,
    pub lines: usize,
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

pub enum FileEvent {
    Modified(PathBuf),
    Created(PathBuf),
    Removed(PathBuf),
    Renamed(PathBuf, PathBuf),
}

pub struct DiffResult {
    pub path: PathBuf,
    pub added: usize,
    pub removed: usize,
}
