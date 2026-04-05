use crate::helpers::walk_directory;
use crate::input::{handle_key, handle_mouse};
use crate::types::{DiffResult, EventProxy, FileEvent, TermSize};
use alacritty_terminal::term::{Config as TermConfig, Term};
use alacritty_terminal::vte;
use anyhow::Result;
use crossterm::event::Event;
use orchestrator_classifier::{
    ClassifierRegistry, ClassifierTrigger, ClassifierType, CliRuntime, CliRuntimeConfig,
    ConfigClassifier, LlmRuntimeStub,
};
use orchestrator_claude_log::{to_turn_builder, ClaudeWatchEvent, ClaudeWatchHandle};
use orchestrator_core::{AgentId, AgentStore, FileTree, FocusState, TraceStore, TurnStore};
use orchestrator_grid::CellRect;
use orchestrator_mcp::doctor::{DoctorMcpServer, DoctorRequest, DoctorResponse};
use orchestrator_mcp::{AgentInfo, McpEvent, McpRequest, McpResponse};
use orchestrator_ml::MlRuntime;
use orchestrator_pty::PtyHandle;
use orchestrator_storage::{Database, StorageHandle};
use orchestrator_widgets::{
    DbMetadataView, QueryResultView, SettingsField, TableInfoView, WidgetAction, WidgetRegistry,
};
use ratatui::layout::Rect;
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
    pub doctor_mcp: Option<DoctorMcpServer>,
    pub running: bool,
}

impl App {
    #[allow(clippy::too_many_arguments)]
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
            grid: config.layout.clone(),
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
            doctor_mcp: None,
            running: true,
        }
    }

    /// Load classifier configs from `.agent-orchestrator/classifiers/` and register them.
    pub fn load_classifier_configs(&mut self) {
        let classifiers_dir = crate::config::dir_path().join("classifiers");
        let configs = orchestrator_classifier::config::load_configs(&classifiers_dir);
        for (_path, config) in configs {
            // Skip if already registered (avoids duplicates on reload)
            if self
                .classifier_registry
                .names()
                .contains(&config.name.as_str())
            {
                continue;
            }

            let enabled = config.enabled;
            let name = config.name.clone();

            let label_names: Vec<String> = config.labels.iter().map(|l| l.label.clone()).collect();
            let runtime: Option<Box<dyn orchestrator_classifier::ClassifierRuntime>> =
                match config.classifier_type {
                    ClassifierType::Ml => self.resolve_ml_runtime(&name),
                    ClassifierType::Llm => Some(Box::new(LlmRuntimeStub)),
                    ClassifierType::Cli => {
                        serde_json::from_value::<CliRuntimeConfig>(config.runtime.clone())
                            .ok()
                            .map(|cfg| {
                                Box::new(CliRuntime::new(cfg, label_names))
                                    as Box<dyn orchestrator_classifier::ClassifierRuntime>
                            })
                    }
                };

            let mut classifier = ConfigClassifier::new(config);
            if let Some(rt) = runtime {
                classifier = classifier.with_runtime(rt);
            }

            self.classifier_registry.register(Box::new(classifier));
            if !enabled {
                self.classifier_registry.set_enabled(&name, false);
            }
        }
    }

    fn resolve_ml_runtime(
        &self,
        classifier_name: &str,
    ) -> Option<Box<dyn orchestrator_classifier::ClassifierRuntime>> {
        let model = self.db_reader.load_latest_model(classifier_name).ok()??;
        let runtime = MlRuntime::from_saved_model(&model).ok()?;
        Some(Box::new(runtime))
    }

    pub fn pane_size(&self) -> (u16, u16) {
        for cell in &self.last_cell_rects {
            if cell.widget == orchestrator_grid::WidgetId::AgentPane {
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
        let agent_name = self
            .agents
            .get(id)
            .expect("agent just created")
            .name
            .clone();
        let cwd = std::env::current_dir()?;
        let (cols, rows) = self.pane_size();
        let pty = PtyHandle::spawn(
            provider,
            &cwd,
            cols,
            rows,
            &[("ORCHESTRATOR_AGENT_NAME", &agent_name)],
        )?;
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
                for result in &results {
                    if let Some(ref notif) = result.notification {
                        let xml = notif
                            .format_xml(&result.classifier_name, &result.trace_id.0.to_string());
                        if let Some(pty) = self.ptys.get_mut(&agent_id) {
                            let _ = pty.write_input(xml.as_bytes());
                        }
                    }
                }
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
                    if self.diff_tx.send(path.clone()).is_err() {
                        tracing::warn!("diff thread gone, dropping file event");
                    }
                }
                FileEvent::Removed(ref path) => {
                    self.file_tree.remove_entry(path);
                }
                FileEvent::Renamed(ref from, ref to) => {
                    self.file_tree.remove_entry(from);
                    self.file_tree.record_activity(to, active_agent);
                    if self.diff_tx.send(to.clone()).is_err() {
                        tracing::warn!("diff thread gone, dropping file event");
                    }
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
                        self.session_map.insert(info.session_id.clone(), agent_id);
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
                        for result in &results {
                            if let Some(ref notif) = result.notification {
                                let xml = notif.format_xml(
                                    &result.classifier_name,
                                    &result.trace_id.0.to_string(),
                                );
                                if let Some(pty) = self.ptys.get_mut(&agent_id) {
                                    let _ = pty.write_input(xml.as_bytes());
                                }
                            }
                        }
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
                    if event.response_tx.send(McpResponse::Agents(agents)).is_err() {
                        tracing::warn!("MCP response channel closed");
                    }
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

                    let resp = match target {
                        Some(id) => {
                            if let Some(pty) = self.ptys.get_mut(&id) {
                                let data =
                                    format!("[Message from {}]: {}\r", from_agent_name, message);
                                if let Err(e) = pty.write_input(data.as_bytes()) {
                                    McpResponse::Error(format!("Write failed: {}", e))
                                } else {
                                    McpResponse::MessageSent
                                }
                            } else {
                                McpResponse::Error(format!(
                                    "Agent '{}' has no active PTY",
                                    to_agent_name
                                ))
                            }
                        }
                        None => McpResponse::Error(format!("No agent named '{}'", to_agent_name)),
                    };
                    if event.response_tx.send(resp).is_err() {
                        tracing::warn!("MCP response channel closed");
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
                if let Err(e) = pty.write_input(data) {
                    tracing::warn!(error = %e, "PTY write failed");
                }
            }
        }
    }

    pub fn hit_test(&self, col: u16, row: u16) -> Option<(orchestrator_grid::WidgetId, u16, u16)> {
        for cell in &self.last_cell_rects {
            let r = &cell.rect;
            if col >= r.x && col < r.x + r.width && row >= r.y && row < r.y + r.height {
                let local_row = (row - r.y).saturating_sub(1);
                let local_col = (col - r.x).saturating_sub(1);
                return Some((cell.widget, local_row, local_col));
            }
        }
        None
    }

    pub fn resize_all_ptys(&mut self) {
        let (cols, rows) = self.pane_size();
        let ids: Vec<AgentId> = self.ptys.keys().copied().collect();
        for id in ids {
            if let Some(pty) = self.ptys.get(&id) {
                if let Err(e) = pty.resize(cols, rows) {
                    tracing::debug!(error = %e, "PTY resize failed");
                }
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
            WidgetAction::QueryTable {
                table,
                limit,
                offset,
            } => {
                if let Ok(result) = self.db_reader.query_table(&table, limit, offset) {
                    self.widgets.database_page.set_rows(QueryResultView {
                        columns: result.columns,
                        rows: result.rows,
                        total_rows: result.total_rows,
                    });
                }
            }
            WidgetAction::SaveConfig => {
                let config = fields_to_config(&self.widgets.settings_page.field_values());
                if crate::config::save(&config).is_ok() {
                    self.widgets.settings_page.mark_saved();
                }
            }
            WidgetAction::StartDoctor => {
                if let Err(e) = self.start_doctor() {
                    tracing::error!(error = %e, "failed to start doctor");
                }
            }
            WidgetAction::ToggleClassifier(name) => {
                let currently_enabled = self.classifier_registry.is_enabled(&name);
                self.classifier_registry
                    .set_enabled(&name, !currently_enabled);
                // Persist to config
                let mut config = fields_to_config(&self.widgets.settings_page.field_values());
                config.disabled_classifiers = self
                    .classifier_registry
                    .classifiers_info()
                    .iter()
                    .filter(|c| !c.enabled)
                    .map(|c| c.name.clone())
                    .collect();
                if let Err(e) = crate::config::save(&config) {
                    tracing::warn!(error = %e, "failed to save config");
                }
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

    pub fn start_doctor(&mut self) -> Result<()> {
        // Start doctor MCP server
        let classifiers_dir = crate::config::dir_path().join("classifiers");
        let doctor = orchestrator_mcp::doctor::start_doctor_mcp(&self.db_path, classifiers_dir)?;

        // Write temporary MCP config pointing to the doctor server
        let config_path = crate::config::dir_path().join("doctor-mcp.json");
        let config = serde_json::json!({
            "mcpServers": {
                "doctor": {
                    "type": "http",
                    "url": format!("http://127.0.0.1:{}", doctor.port),
                    "autoApprove": ["list_traces", "get_trace", "list_classifiers", "create_classifier", "label_trace", "list_training_labels", "train_classifier"]
                }
            }
        });
        std::fs::write(&config_path, serde_json::to_string_pretty(&config)?)?;

        // Write the /aorch-review slash command
        let commands_dir = std::env::current_dir()?.join(".claude/commands");
        std::fs::create_dir_all(&commands_dir)?;
        let skill_path = commands_dir.join("aorch-review.md");
        std::fs::write(
            &skill_path,
            concat!(
                "Review agent traces and diagnose issues.\n\n",
                "Steps:\n",
                "1. Use list_classifiers to see what classifiers are registered and their status.\n",
                "2. Use list_traces to get recent agent sessions.\n",
                "3. For each trace with high token usage or many turns, use get_trace to inspect the full conversation.\n",
                "4. Look for patterns: excessive token usage, repeated errors, looping behavior, classifier flags, aborted sessions.\n",
                "5. Produce a diagnosis report with:\n",
                "   - Summary of traces reviewed\n",
                "   - Any anomalies or issues found\n",
                "   - Token usage analysis (totals, averages, outliers)\n",
                "   - Classifier findings\n",
                "   - Recommendations\n",
            ),
        )?;

        // Spawn claude with --mcp-config and --system-prompt
        let id = self.agents.create("claude");
        let agent_name = self
            .agents
            .get(id)
            .expect("agent just created")
            .name
            .clone();
        let cwd = std::env::current_dir()?;
        let (cols, rows) = self.pane_size();
        let config_str = config_path.to_string_lossy().to_string();
        let system_prompt = concat!(
            "You are the doctor agent for an AI agent orchestrator. ",
            "Your job is to analyze agent traces, turns, and classifier results ",
            "to diagnose issues, identify patterns, and provide actionable insights.\n\n",
            "You have access to these MCP tools:\n",
            "- list_traces: Browse recent agent sessions with token usage and status\n",
            "- get_trace: Drill into a specific trace to see all turns and classifications\n",
            "- list_classifiers: See registered classifiers and their status\n",
            "- create_classifier: Create a new classifier config with name, trigger, type, and output labels\n",
            "- label_trace: Assign a training label to a trace for a specific classifier\n",
            "- list_training_labels: See labeled traces and label distribution for a classifier\n",
            "- train_classifier: Train an ML classifier using labeled traces\n\n",
            "You also have the /aorch-review skill which provides a step-by-step review workflow.\n\n",
            "Start by understanding what data is available, then analyze patterns ",
            "like excessive token usage, repeated errors, classifier flags, or unusual behavior.",
        );
        let pty = PtyHandle::spawn_with_args(
            "claude",
            &[
                "--mcp-config",
                &config_str,
                "--system-prompt",
                system_prompt,
            ],
            &cwd,
            cols,
            rows,
            &[("ORCHESTRATOR_AGENT_NAME", &agent_name)],
        )?;

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

        self.widgets.doctor_page.set_agent(id);
        self.doctor_mcp = Some(doctor);

        Ok(())
    }

    pub fn poll_doctor_events(&mut self) {
        let Some(ref doctor) = self.doctor_mcp else {
            return;
        };
        let events: Vec<_> = std::iter::from_fn(|| doctor.rx.try_recv().ok()).collect();
        for event in events {
            match event.request {
                DoctorRequest::ListClassifiers => {
                    let classifiers = self.classifier_registry.classifiers_info();
                    let _ = event
                        .response_tx
                        .send(DoctorResponse::Classifiers(classifiers));
                }
                DoctorRequest::ReloadClassifiers => {
                    self.load_classifier_configs();
                    let _ = event.response_tx.send(DoctorResponse::Ok);
                }
            }
        }
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
            self.widgets.database_page.set_metadata(DbMetadataView {
                file_size_bytes: meta.file_size_bytes,
                total_rows: meta.total_rows,
                tables: widget_tables,
            });
            // Auto-query the first/selected table
            if let Some((table, limit, offset)) = self.widgets.database_page.current_query() {
                if let Ok(result) = self.db_reader.query_table(&table, limit, offset) {
                    self.widgets.database_page.set_rows(QueryResultView {
                        columns: result.columns,
                        rows: result.rows,
                        total_rows: result.total_rows,
                    });
                }
            }
        }
    }

    pub fn refresh_classifier_panel(&mut self) {
        use orchestrator_widgets::ClassifierStatusView;

        let statuses: Vec<ClassifierStatusView> = self
            .classifier_registry
            .classifiers_info()
            .into_iter()
            .map(|c| ClassifierStatusView {
                name: c.name,
                trigger: c.trigger.display_name(),
                enabled: c.enabled,
            })
            .collect();
        self.widgets.classifier_panel.set_classifiers(statuses);
    }

    pub fn refresh_classifiers(&mut self) {
        use orchestrator_widgets::{ClassifierDetailView, LabelView};

        let details: Vec<ClassifierDetailView> = self
            .classifier_registry
            .classifiers_with_configs()
            .into_iter()
            .map(|(enabled, cfg)| {
                let trigger: ClassifierTrigger = cfg.trigger.clone().into();
                ClassifierDetailView {
                    name: cfg.name.clone(),
                    description: cfg.description.clone(),
                    version: cfg.version.clone(),
                    classifier_type: format!("{:?}", cfg.classifier_type).to_lowercase(),
                    trigger: trigger.display_name(),
                    enabled,
                    labels: cfg
                        .labels
                        .iter()
                        .map(|l| LabelView {
                            label: l.label.clone(),
                            notification: l.notification.message.clone(),
                        })
                        .collect(),
                }
            })
            .collect();
        self.widgets.classifiers_page.set_classifiers(details);
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
    let mut config = crate::config::Config::default();
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
