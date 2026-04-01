use crate::helpers::walk_directory;
use crate::input::{handle_key, handle_mouse};
use crate::types::{DiffResult, EventProxy, FileEvent, TermSize};
use alacritty_terminal::term::{Config as TermConfig, Term};
use alacritty_terminal::vte;
use anyhow::Result;
use crossterm::event::Event;
use orchestrator_claude_log::{to_turn_builder, ClaudeWatchEvent, ClaudeWatchHandle};
use orchestrator_classifier::ClassifierRegistry;
use orchestrator_core::{AgentId, AgentStore, FileTree, FocusState, TraceStore, TurnStore};
use ratatui::layout::Rect;
use orchestrator_grid::CellRect;
use orchestrator_mcp::{AgentInfo, McpEvent, McpRequest, McpResponse};
use orchestrator_pty::PtyHandle;
use orchestrator_storage::{Database, StorageHandle};
use orchestrator_widgets::{
    DbMetadataView, QueryResultView, SettingsField, TableInfoView, WidgetAction, WidgetRegistry,
};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::mpsc;

pub struct App {
    pub agents: AgentStore,
    pub turns: TurnStore,
    pub traces: TraceStore,
    pub storage: StorageHandle,
    pub db_reader: Database,
    pub db_path: PathBuf,
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
    pub mcp_rx: mpsc::Receiver<McpEvent>,
    pub claude_watcher: Option<ClaudeWatchHandle>,
    /// Maps session_id → agent_id for Claude log association.
    pub session_map: HashMap<String, AgentId>,
    /// Maps child PID → agent_id for session discovery.
    pub pid_map: HashMap<u32, AgentId>,
    pub last_cell_rects: Vec<CellRect>,
    pub last_nav_rect: Rect,
    pub classifier_registry: ClassifierRegistry,
    pub running: bool,
}

impl App {
    pub fn new(
        input_rx: mpsc::Receiver<Event>,
        file_rx: mpsc::Receiver<FileEvent>,
        diff_tx: mpsc::Sender<PathBuf>,
        diff_rx: mpsc::Receiver<DiffResult>,
        mcp_rx: mpsc::Receiver<McpEvent>,
        storage: StorageHandle,
        db_reader: Database,
        db_path: PathBuf,
        claude_watcher: Option<ClaudeWatchHandle>,
        config: &crate::config::Config,
    ) -> Self {
        let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        let mut file_tree = FileTree::new(cwd.clone());
        file_tree.set_entries(walk_directory(&cwd));

        let mut widgets = WidgetRegistry::new();
        widgets.settings_page.set_fields(config_to_fields(config));

        Self {
            agents: AgentStore::new(),
            turns: TurnStore::new(),
            traces: TraceStore::new(),
            storage,
            db_reader,
            db_path,
            focus: FocusState::new(),
            file_tree,
            grid: orchestrator_grid::load_or_default(),
            widgets,
            ptys: HashMap::new(),
            terms: HashMap::new(),
            vte_parsers: HashMap::new(),
            input_rx,
            file_rx,
            diff_tx,
            diff_rx,
            mcp_rx,
            claude_watcher,
            session_map: HashMap::new(),
            pid_map: HashMap::new(),
            last_cell_rects: Vec::new(),
            last_nav_rect: Rect::default(),
            classifier_registry: ClassifierRegistry::new(),
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
        let agent_name = self.agents.get(id).expect("agent just created").name.clone();
        let cwd = std::env::current_dir()?;
        let (cols, rows) = self.pane_size();
        let pty = PtyHandle::spawn(provider, &cwd, cols, rows, &[
            ("ORCHESTRATOR_AGENT_NAME", &agent_name),
        ])?;
        if let Some(pid) = pty.child_pid() {
            if let Some(agent) = self.agents.get_mut(id) {
                agent.child_pid = Some(pid);
            }
            self.pid_map.insert(pid, id);
        }
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
        // Abort any active traces and flush them to persistence
        let now = chrono_now();
        while let Some(trace) = self.traces.active_trace(id) {
            let trace_id = trace.id;
            self.traces.abort(trace_id, now.clone());
        }
        self.flush_finished_traces(id);
        self.traces.remove_agent_traces(id);
        self.turns.remove_agent_turns(id);

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

    /// Flush all finished traces for an agent to the persistence layer.
    fn flush_finished_traces(&mut self, agent_id: AgentId) {
        let finished = self.traces.take_finished();
        for trace in finished {
            if trace.agent_id == agent_id {
                let turns: Vec<_> = self
                    .turns
                    .turns_for(agent_id)
                    .into_iter()
                    .filter(|t| t.session_id.as_deref() == Some(&trace.session_id))
                    .cloned()
                    .collect();
                let results = self.classifier_registry.run_on_complete(&trace, &turns);
                for result in results {
                    self.storage.save_classification(result);
                }
                self.storage.save_trace(trace, turns);
            }
        }
    }

    /// Flush all remaining traces on shutdown.
    pub fn flush_all_traces(&mut self) {
        let now = chrono_now();
        let agent_ids = self.agents.ids();
        for agent_id in &agent_ids {
            while let Some(trace) = self.traces.active_trace(*agent_id) {
                let trace_id = trace.id;
                self.traces.abort(trace_id, now.clone());
            }
        }
        let finished = self.traces.take_finished();
        for trace in finished {
            let turns: Vec<_> = self
                .turns
                .turns_for(trace.agent_id)
                .into_iter()
                .filter(|t| t.session_id.as_deref() == Some(&trace.session_id))
                .cloned()
                .collect();
            let results = self.classifier_registry.run_on_complete(&trace, &turns);
            for result in results {
                self.storage.save_classification(result);
            }
            self.storage.save_trace(trace, turns);
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

    pub fn poll_claude_logs(&mut self) {
        let Some(ref watcher) = self.claude_watcher else {
            return;
        };
        while let Some(event) = watcher.try_recv() {
            match event {
                ClaudeWatchEvent::SessionDiscovered(info) => {
                    // Map PID → session_id → agent_id
                    if let Some(&agent_id) = self.pid_map.get(&info.pid) {
                        self.session_map
                            .insert(info.session_id.clone(), agent_id);
                        if let Some(agent) = self.agents.get_mut(agent_id) {
                            agent.session_id = Some(info.session_id);
                        }
                    }
                }
                ClaudeWatchEvent::LogEntry(entry) => {
                    // Skip entries we can't map to an agent
                    let Some(&agent_id) = self.session_map.get(&entry.session_id) else {
                        continue;
                    };

                    // Skip non-message entries (file-history-snapshot, last-prompt, etc.)
                    let Some(builder) = to_turn_builder(&entry) else {
                        continue;
                    };

                    // Check for duplicate by UUID
                    if self.turns.find_by_uuid(&entry.uuid).is_some() {
                        continue;
                    }

                    // Insert turn
                    let turn_id = self.turns.insert(agent_id, builder);

                    // Ensure trace exists for this session
                    if self
                        .traces
                        .find_by_session(agent_id, &entry.session_id)
                        .is_none()
                    {
                        let provider = self
                            .agents
                            .get(agent_id)
                            .map(|a| a.provider.clone())
                            .unwrap_or_default();
                        self.traces.create(
                            agent_id,
                            entry.session_id.clone(),
                            provider,
                            entry.timestamp.clone(),
                        );
                    }

                    // Update trace counters
                    if let Some(trace) = self.traces.active_trace_mut(agent_id) {
                        trace.turn_count += 1;
                        // Accumulate token usage from assistant turns
                        if let Some(turn) = self.turns.get(turn_id) {
                            if let orchestrator_core::TurnMeta::Assistant(ref meta) = turn.meta {
                                if let Some(ref usage) = meta.usage {
                                    trace.token_usage.input_tokens += usage.input_tokens;
                                    trace.token_usage.output_tokens += usage.output_tokens;
                                    if let Some(v) = usage.cache_creation_input_tokens {
                                        *trace
                                            .token_usage
                                            .cache_creation_input_tokens
                                            .get_or_insert(0) += v;
                                    }
                                    if let Some(v) = usage.cache_read_input_tokens {
                                        *trace
                                            .token_usage
                                            .cache_read_input_tokens
                                            .get_or_insert(0) += v;
                                    }
                                }
                                if trace.model.is_none() {
                                    trace.model = meta.model.clone();
                                }
                            }
                        }
                    }

                    // Immediately flush to SQLite + run incremental classifiers
                    if let Some(trace) = self.traces.active_trace(agent_id) {
                        let trace_clone = trace.clone();
                        let turns: Vec<_> = self
                            .turns
                            .turns_for(agent_id)
                            .into_iter()
                            .filter(|t| t.session_id.as_deref() == Some(&entry.session_id))
                            .cloned()
                            .collect();
                        let results = self
                            .classifier_registry
                            .run_incremental(&trace_clone, &turns);
                        for result in results {
                            self.storage.save_classification(result);
                        }
                        self.storage.save_trace(trace_clone, turns);
                    }
                }
            }
        }
    }

    pub fn poll_mcp_requests(&mut self) {
        while let Ok(event) = self.mcp_rx.try_recv() {
            match event.request {
                McpRequest::ListAgents => {
                    let agents: Vec<AgentInfo> = self
                        .agents
                        .list()
                        .iter()
                        .map(|a| AgentInfo {
                            id: a.id.0.to_string(),
                            name: a.name.clone(),
                            status: format!("{:?}", a.status),
                        })
                        .collect();
                    let _ = event.response_tx.send(McpResponse::Agents(agents));
                }
                McpRequest::SendMessage {
                    from_agent_name,
                    to_agent_name,
                    message,
                } => {
                    let target = self
                        .agents
                        .list()
                        .iter()
                        .find(|a| a.name == to_agent_name)
                        .map(|a| a.id);

                    match target {
                        Some(id) => {
                            if let Some(pty) = self.ptys.get_mut(&id) {
                                let data =
                                    format!("[Message from {}]: {}\r", from_agent_name, message);
                                if let Err(e) = pty.write_input(data.as_bytes()) {
                                    let _ = event.response_tx.send(McpResponse::Error(
                                        format!("Write failed: {}", e),
                                    ));
                                } else {
                                    let _ =
                                        event.response_tx.send(McpResponse::MessageSent);
                                }
                            } else {
                                let _ = event.response_tx.send(McpResponse::Error(format!(
                                    "Agent '{}' has no active PTY",
                                    to_agent_name
                                )));
                            }
                        }
                        None => {
                            let _ = event.response_tx.send(McpResponse::Error(format!(
                                "No agent named '{}'",
                                to_agent_name
                            )));
                        }
                    }
                }
            }
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
            WidgetAction::NavigateTo(page) => {
                self.focus.page = page;
            }
            WidgetAction::RefreshDatabase => {
                self.refresh_database();
            }
            WidgetAction::QueryTable { table, limit, offset } => {
                if let Ok(result) = self.db_reader.query_table(&table, limit, offset) {
                    self.widgets.database_page.set_rows(
                        QueryResultView {
                            columns: result.columns,
                            rows: result.rows,
                            total_rows: result.total_rows,
                        },
                    );
                }
            }
            WidgetAction::SaveConfig => {
                let config = fields_to_config(&self.widgets.settings_page.field_values());
                if crate::config::save(&config).is_ok() {
                    self.widgets.settings_page.mark_saved();
                }
            }
            WidgetAction::ToggleClassifier(name) => {
                let currently_enabled = self.classifier_registry.is_enabled(&name);
                self.classifier_registry.set_enabled(&name, !currently_enabled);
                // Persist to config
                let mut config = fields_to_config(&self.widgets.settings_page.field_values());
                config.disabled_classifiers = self
                    .classifier_registry
                    .classifiers_info()
                    .iter()
                    .filter(|c| !c.enabled)
                    .map(|c| c.name.clone())
                    .collect();
                let _ = crate::config::save(&config);
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

    pub fn refresh_database(&mut self) {
        if let Ok(meta) = self.db_reader.metadata(&self.db_path) {
            let widget_tables: Vec<TableInfoView> = meta
                .tables
                .iter()
                .map(|t| TableInfoView {
                    name: t.name.clone(),
                    row_count: t.row_count,
                })
                .collect();
            self.widgets.database_page.set_metadata(
                DbMetadataView {
                    file_size_bytes: meta.file_size_bytes,
                    total_rows: meta.total_rows,
                    tables: widget_tables,
                },
            );
            // Auto-query the first/selected table
            if let Some((table, limit, offset)) = self.widgets.database_page.current_query() {
                if let Ok(result) = self.db_reader.query_table(&table, limit, offset) {
                    self.widgets.database_page.set_rows(
                        QueryResultView {
                            columns: result.columns,
                            rows: result.rows,
                            total_rows: result.total_rows,
                        },
                    );
                }
            }
        }
    }

    pub fn refresh_classifiers(&mut self) {
        use orchestrator_widgets::{ClassificationResultView, ClassifierStatusView};

        let statuses: Vec<ClassifierStatusView> = self
            .classifier_registry
            .classifiers_info()
            .into_iter()
            .map(|c| ClassifierStatusView {
                name: c.name,
                trigger: format!("{:?}", c.trigger),
                enabled: c.enabled,
            })
            .collect();
        self.widgets.classifiers_page.set_classifiers(statuses);

        if let Ok(results) = self.db_reader.list_recent_classifications(50) {
            let views: Vec<ClassificationResultView> = results
                .into_iter()
                .map(|r| ClassificationResultView {
                    classifier_name: r.classifier_name,
                    trace_id: r.trace_id.0.to_string(),
                    timestamp: r.timestamp,
                    payload: serde_json::to_string(&r.payload).unwrap_or_default(),
                })
                .collect();
            self.widgets.classifiers_page.set_results(views);
        }
    }
}

fn config_to_fields(config: &crate::config::Config) -> Vec<SettingsField> {
    vec![SettingsField {
        label: "MCP Port",
        key: "mcp_port",
        value: config.mcp_port.to_string(),
        description: "Port for the MCP JSON-RPC server",
    }]
}

fn fields_to_config(values: &[(&str, &str)]) -> crate::config::Config {
    let mut config = crate::config::Config {
        mcp_port: 7850,
        disabled_classifiers: Vec::new(),
    };
    for &(key, val) in values {
        if key == "mcp_port" {
            if let Ok(port) = val.parse::<u16>() {
                config.mcp_port = port;
            }
        }
    }
    config
}

/// Simple UTC timestamp without pulling in the chrono crate.
fn chrono_now() -> String {
    use std::time::SystemTime;
    let dur = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default();
    let secs = dur.as_secs();
    // Good enough ISO 8601 for trace timestamps
    format!("{}Z", secs)
}
