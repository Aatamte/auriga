use crate::helpers::walk_directory;
use crate::input::{handle_key, handle_mouse};
use crate::types::{DiffResult, EventProxy, FileEvent, TermSize};
use alacritty_terminal::term::{Config as TermConfig, Term};
use alacritty_terminal::vte;
use anyhow::Result;
use crossterm::event::Event;
use orchestrator_core::{AgentId, AgentStore, FileTree, FocusState};
use orchestrator_grid::CellRect;
use orchestrator_pty::PtyHandle;
use orchestrator_widgets::{WidgetAction, WidgetRegistry};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::mpsc;

pub struct App {
    pub agents: AgentStore,
    pub focus: FocusState,
    pub file_tree: FileTree,
    pub grid: orchestrator_grid::Grid,
    pub widgets: WidgetRegistry,
    pub ptys: HashMap<AgentId, PtyHandle>,
    pub terms: HashMap<AgentId, Term<EventProxy>>,
    pub vte_parsers: HashMap<AgentId, vte::ansi::Processor<vte::ansi::StdSyncHandler>>,
    pub input_rx: mpsc::Receiver<Event>,
    pub file_rx: mpsc::Receiver<FileEvent>,
    pub diff_tx: mpsc::Sender<PathBuf>,
    pub diff_rx: mpsc::Receiver<DiffResult>,
    pub last_cell_rects: Vec<CellRect>,
    pub running: bool,
}

impl App {
    pub fn new(
        input_rx: mpsc::Receiver<Event>,
        file_rx: mpsc::Receiver<FileEvent>,
        diff_tx: mpsc::Sender<PathBuf>,
        diff_rx: mpsc::Receiver<DiffResult>,
    ) -> Self {
        let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        let mut file_tree = FileTree::new(cwd.clone());
        file_tree.set_entries(walk_directory(&cwd));

        Self {
            agents: AgentStore::new(),
            focus: FocusState::new(),
            file_tree,
            grid: orchestrator_grid::load_or_default(),
            widgets: WidgetRegistry::new(),
            ptys: HashMap::new(),
            terms: HashMap::new(),
            vte_parsers: HashMap::new(),
            input_rx,
            file_rx,
            diff_tx,
            diff_rx,
            last_cell_rects: Vec::new(),
            running: true,
        }
    }

    pub fn pane_size(&self) -> (u16, u16) {
        for cell in &self.last_cell_rects {
            if cell.widget == "agent-pane" {
                let cols = cell.rect.width.saturating_sub(2).max(1);
                let rows = cell.rect.height.saturating_sub(2).max(1);
                return (cols, rows);
            }
        }
        let (cols, rows) = crossterm::terminal::size().unwrap_or((120, 40));
        (cols, rows)
    }

    pub fn spawn_agent(&mut self, provider: &str) -> Result<AgentId> {
        let id = self.agents.create(provider);
        let cwd = std::env::current_dir()?;
        let (cols, rows) = self.pane_size();
        let pty = PtyHandle::spawn(provider, &cwd, cols, rows)?;
        let term_config = TermConfig {
            scrolling_history: 10_000,
            ..Default::default()
        };
        let term_size = TermSize {
            cols: cols as usize,
            lines: rows as usize,
        };
        let term = Term::new(term_config, &term_size, EventProxy);
        self.ptys.insert(id, pty);
        self.terms.insert(id, term);
        self.vte_parsers.insert(id, vte::ansi::Processor::new());

        if self.focus.active_agent.is_none() {
            self.focus.set_active_agent(id);
        }

        Ok(id)
    }

    pub fn kill_agent(&mut self, id: AgentId) {
        self.ptys.remove(&id);
        self.terms.remove(&id);
        self.vte_parsers.remove(&id);
        self.agents.remove(id);

        if self.focus.active_agent == Some(id) {
            let ids = self.agents.ids();
            match ids.first() {
                Some(&next) => self.focus.set_active_agent(next),
                None => self.focus.clear_active_agent(),
            }
        }
    }

    pub fn poll_pty_output(&mut self) {
        let ids: Vec<AgentId> = self.ptys.keys().copied().collect();
        for id in ids {
            if let Some(pty) = self.ptys.get(&id) {
                let mut got_data = false;
                while let Some(data) = pty.try_read() {
                    if let (Some(term), Some(parser)) =
                        (self.terms.get_mut(&id), self.vte_parsers.get_mut(&id))
                    {
                        parser.advance(term, &data);
                        got_data = true;
                    }
                }
                if let Some(agent) = self.agents.get_mut(id) {
                    agent.status = if got_data {
                        orchestrator_core::AgentStatus::Working
                    } else {
                        orchestrator_core::AgentStatus::Idle
                    };
                }
            }
        }
    }

    pub fn poll_file_events(&mut self) {
        let active_agent = self.focus.active_agent;
        while let Ok(event) = self.file_rx.try_recv() {
            match event {
                FileEvent::Modified(ref path) | FileEvent::Created(ref path) => {
                    self.file_tree.record_activity(path, active_agent);
                    let _ = self.diff_tx.send(path.clone());
                }
                FileEvent::Removed(ref path) => {
                    self.file_tree.remove_entry(path);
                }
                FileEvent::Renamed(ref from, ref to) => {
                    self.file_tree.remove_entry(from);
                    self.file_tree.record_activity(to, active_agent);
                    let _ = self.diff_tx.send(to.clone());
                }
            }
        }
    }

    pub fn poll_diff_results(&mut self) {
        while let Ok(result) = self.diff_rx.try_recv() {
            self.file_tree
                .update_diff(&result.path, result.added, result.removed);
        }
    }

    pub fn poll_input(&mut self) {
        while let Ok(event) = self.input_rx.try_recv() {
            match event {
                Event::Key(key) => handle_key(self, key),
                Event::Mouse(mouse) => handle_mouse(self, mouse),
                Event::Resize(_, _) => self.resize_all_ptys(),
                _ => {}
            }
            if !self.running {
                return;
            }
        }
    }

    pub fn write_to_active(&mut self, data: &[u8]) {
        if let Some(id) = self.focus.active_agent {
            if let Some(pty) = self.ptys.get_mut(&id) {
                let _ = pty.write_input(data);
            }
        }
    }

    pub fn hit_test(&self, col: u16, row: u16) -> Option<(String, u16, u16)> {
        for cell in &self.last_cell_rects {
            let r = &cell.rect;
            if col >= r.x && col < r.x + r.width && row >= r.y && row < r.y + r.height {
                let local_row = (row - r.y).saturating_sub(1);
                let local_col = (col - r.x).saturating_sub(1);
                return Some((cell.widget.clone(), local_row, local_col));
            }
        }
        None
    }

    pub fn resize_all_ptys(&mut self) {
        let (cols, rows) = self.pane_size();
        let ids: Vec<AgentId> = self.ptys.keys().copied().collect();
        for id in ids {
            if let Some(pty) = self.ptys.get(&id) {
                let _ = pty.resize(cols, rows);
            }
            if let Some(term) = self.terms.get_mut(&id) {
                let size = TermSize {
                    cols: cols as usize,
                    lines: rows as usize,
                };
                term.resize(size);
            }
        }
    }

    pub fn handle_widget_action(&mut self, action: WidgetAction) {
        match action {
            WidgetAction::SelectAgent(id) => {
                self.focus.set_active_agent(id);
            }
            WidgetAction::ToggleDir(idx) => {
                self.file_tree.toggle_dir(idx);
            }
            WidgetAction::FocusAgent(id) => {
                self.focus.set_active_agent(id);
            }
            WidgetAction::BackToGrid => {
                self.widgets
                    .agent_pane
                    .set_mode(orchestrator_widgets::agent_pane::PaneMode::Grid);
            }
        }
    }

    pub fn toggle_pane_mode(&mut self) {
        let new_mode = match self.widgets.agent_pane.mode {
            orchestrator_widgets::agent_pane::PaneMode::Grid => {
                orchestrator_widgets::agent_pane::PaneMode::Focused
            }
            orchestrator_widgets::agent_pane::PaneMode::Focused => {
                orchestrator_widgets::agent_pane::PaneMode::Grid
            }
        };
        self.widgets.agent_pane.set_mode(new_mode);
    }
}
