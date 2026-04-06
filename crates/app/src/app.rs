use crate::helpers::walk_directory;
use crate::input::{handle_key, handle_mouse};
use crate::types::{DiffResult, EventProxy, FileEvent, TermSize};
use alacritty_terminal::term::{Config as TermConfig, Term};
use alacritty_terminal::vte;
use anyhow::Result;
use crossterm::event::Event;
use orchestrator_agent::providers::claude::ClaudeProvider;
use orchestrator_agent::{
    AgentConfig, AgentMode, GenerateRequest, GenerateResponse, Provider, Session,
};
use orchestrator_classifier::{
    ClassifierRegistry, ClassifierTrigger, ClassifierType, CliRuntime, CliRuntimeConfig,
    ConfigClassifier, LlmRuntimeStub,
};
use orchestrator_claude_log::{to_turn_builder, ClaudeWatchEvent, ClaudeWatchHandle};
use orchestrator_core::{
    AgentId, AgentStore, DisplayMode, FileTree, FocusState, TraceStore, TurnStore,
};
use orchestrator_grid::{CellRect, Grid};
use orchestrator_mcp::doctor::{DoctorMcpServer, DoctorRequest, DoctorResponse};
use orchestrator_mcp::{AgentInfo, McpEvent, McpRequest, McpResponse};
use orchestrator_ml::MlRuntime;
use orchestrator_pty::PtyHandle;
use orchestrator_skills::SkillRegistry;
use orchestrator_storage::{Database, StorageHandle};
use orchestrator_widgets::{
    DbMetadataView, FieldKind, QueryResultView, SettingsField, TableInfoView, WidgetAction,
    WidgetRegistry,
};
use ratatui::layout::Rect;
use std::collections::HashMap;
use std::path::PathBuf;

/// When false, always use the built-in default layout instead of reading from config.json.
const USE_FS_LAYOUT: bool = false;
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
    pub default_display_mode: DisplayMode,
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
    pub skill_registry: SkillRegistry,
    pub doctor_mcp: Option<DoctorMcpServer>,
    pub sessions: HashMap<AgentId, Session>,
    pub generate_tx: mpsc::Sender<(AgentId, GenerateRequest)>,
    pub generate_rx: mpsc::Receiver<(AgentId, Result<GenerateResponse, String>)>,
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
        generate_tx: mpsc::Sender<(AgentId, GenerateRequest)>,
        generate_rx: mpsc::Receiver<(AgentId, Result<GenerateResponse, String>)>,
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
            grid: if USE_FS_LAYOUT {
                config.layout.clone()
            } else {
                Grid::default()
            },
            widgets,
            default_display_mode: match config.display_mode.as_str() {
                "provider" => DisplayMode::Provider,
                _ => DisplayMode::Native,
            },
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
            skill_registry: SkillRegistry::new(),
            doctor_mcp: None,
            sessions: HashMap::new(),
            generate_tx,
            generate_rx,
            running: true,
        }
    }

    /// Register built-in default skills.
    pub fn register_default_skills(&mut self) {
        self.skill_registry
            .register(Box::new(orchestrator_skills::CodeReviewSkill));
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

    /// Build the default AgentConfig, injecting system prompt + repository context.
    pub fn default_agent_config(&self) -> AgentConfig {
        let prompts = load_system_prompts();
        let prompt_content = prompts
            .iter()
            .find(|p| p.enabled && p.provider == "claude")
            .map(|p| p.content.clone());

        // Combine: system prompt + Layer 0 map context
        let map_content = crate::context::load_map_content();
        let system_prompt = build_system_prompt(prompt_content.as_deref(), &map_content);

        AgentConfig {
            name: "agent".into(),
            provider: "claude".into(),
            model: String::new(),
            max_tokens: 0,
            system_prompt,
            temperature: None,
            mode: AgentMode::NativeCli,
            provider_config: serde_json::json!({}),
        }
    }

    pub fn spawn_agent(&mut self, config: &AgentConfig) -> Result<AgentId> {
        let id = self.agents.create(&config.provider);
        if let Some(agent) = self.agents.get_mut(id) {
            agent.display_mode = self.default_display_mode;
            // Track which system prompt was used (derive from config name or explicit)
            if config.system_prompt.is_some() {
                // Check if this matches a named prompt file
                let prompts = load_system_prompts();
                if let Some(p) = prompts.iter().find(|p| {
                    p.enabled && Some(p.content.as_str()) == config.system_prompt.as_deref()
                }) {
                    agent.system_prompt_name = Some(p.name.clone());
                }
            }
        }

        match self.default_display_mode {
            DisplayMode::Native => {
                // Managed mode: create a Session, no PTY
                let session = Session::new(config.clone(), vec![]);
                self.sessions.insert(id, session);
            }
            DisplayMode::Provider => {
                // Provider mode: spawn PTY + terminal emulator
                let provider = resolve_provider(&config.provider);
                let spec = provider.build_command(config).ok_or_else(|| {
                    anyhow::anyhow!("provider '{}' cannot build CLI command", config.provider)
                })?;

                let agent_name = self
                    .agents
                    .get(id)
                    .expect("agent just created")
                    .name
                    .clone();
                let cwd = std::env::current_dir()?;
                let (cols, rows) = self.pane_size();

                let mut env: Vec<(&str, &str)> = vec![("ORCHESTRATOR_AGENT_NAME", &agent_name)];
                let env_owned: Vec<(String, String)> = spec.env.clone();
                for (k, v) in &env_owned {
                    env.push((k.as_str(), v.as_str()));
                }

                let args: Vec<&str> = spec.args.iter().map(|s| s.as_str()).collect();
                let pty = PtyHandle::spawn_with_args(&spec.program, &args, &cwd, cols, rows, &env)?;
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
            }
        }

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
        use std::time::{Duration, Instant};

        const DEBOUNCE: Duration = Duration::from_secs(2);
        let now = Instant::now();

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
                    if got_data {
                        agent.last_active_at = Some(now);
                        agent.status = orchestrator_core::AgentStatus::Working;
                    } else {
                        let still_active = agent
                            .last_active_at
                            .is_some_and(|t| now.duration_since(t) < DEBOUNCE);
                        if !still_active {
                            agent.status = orchestrator_core::AgentStatus::Idle;
                        }
                    }
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

    /// Send the input buffer contents as a user message to the active native agent.
    pub fn send_native_message(&mut self) {
        let text = std::mem::take(&mut self.widgets.agent_pane.input_buffer);
        if text.trim().is_empty() {
            return;
        }
        let Some(agent_id) = self.focus.active_agent else {
            return;
        };
        let Some(session) = self.sessions.get_mut(&agent_id) else {
            return;
        };

        let content = orchestrator_core::MessageContent::Text(text.clone());
        if let Some(request) = session.send_message(content.clone()) {
            // Insert user turn into TurnStore via bridge
            let now = chrono_now();
            let turn_builder = orchestrator_agent::user_message_to_turn(
                &orchestrator_agent::Message {
                    role: orchestrator_agent::Role::User,
                    content,
                },
                &session.id,
                &now,
            );
            self.turns.insert(agent_id, turn_builder);

            // Ensure trace exists
            if self.traces.active_trace(agent_id).is_none() {
                let provider = self
                    .agents
                    .get(agent_id)
                    .map(|a| a.provider.clone())
                    .unwrap_or_default();
                self.traces
                    .create(agent_id, session.id.0.to_string(), provider, now);
            }

            // Send to generation thread
            let _ = self.generate_tx.send((agent_id, request));
            self.widgets.agent_pane.generating = true;
        }
    }

    /// Drain generation responses from the background thread.
    pub fn poll_generate_responses(&mut self) {
        while let Ok((agent_id, result)) = self.generate_rx.try_recv() {
            match result {
                Ok(response) => {
                    // Insert assistant turn via bridge
                    let now = chrono_now();
                    if let Some(session) = self.sessions.get(&agent_id) {
                        let turn_builder =
                            orchestrator_agent::response_to_turn(&response, &session.id, &now);
                        self.turns.insert(agent_id, turn_builder);

                        // Update trace
                        if let Some(trace) = self.traces.active_trace_mut(agent_id) {
                            trace.turn_count += 1;
                            trace.token_usage.input_tokens += response.usage.input_tokens;
                            trace.token_usage.output_tokens += response.usage.output_tokens;
                            if trace.model.is_none() {
                                trace.model = Some(response.model.clone());
                            }
                        }

                        // Store provider session ID for conversation resumption
                        let provider_sid = response.provider_session_id.clone();

                        // Process session state machine
                        if let Some(session) = self.sessions.get_mut(&agent_id) {
                            let _tool_calls = session.receive_response(response);
                            // Store provider session ID if this is the first response
                            if session.provider_session_id.is_none() {
                                session.provider_session_id = provider_sid;
                            }
                        }
                    }
                    self.widgets.agent_pane.generating = false;
                }
                Err(err) => {
                    tracing::warn!(agent = ?agent_id, error = %err, "generation failed");
                    self.widgets.agent_pane.generating = false;
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
            WidgetAction::ToggleSkill(name) => {
                let currently_enabled = self.skill_registry.is_enabled(&name);
                self.skill_registry.set_enabled(&name, !currently_enabled);
                self.refresh_prompts();
            }
            WidgetAction::ToggleSystemPrompt(name) => {
                self.toggle_system_prompt(&name);
                self.refresh_prompts();
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

        // Spawn doctor agent via AgentConfig
        let config_str = config_path.to_string_lossy().to_string();
        let doctor_config = AgentConfig {
            name: "doctor".into(),
            provider: "claude".into(),
            model: "claude-sonnet-4-20250514".into(),
            max_tokens: 8192,
            system_prompt: Some(concat!(
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
            ).into()),
            temperature: None,
            mode: AgentMode::NativeCli,
            provider_config: serde_json::json!({
                "mcp_config_path": config_str,
            }),
        };
        let id = self.spawn_agent(&doctor_config)?;

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

    pub fn refresh_context(&mut self) {
        let store = crate::context::load();

        let map_view = if store.map.content.is_empty() {
            None
        } else {
            Some(orchestrator_widgets::ContextMapView {
                content: store.map.content,
                last_verified: store.map.last_verified,
            })
        };
        self.widgets.context_page.set_map(map_view);

        let annotations: Vec<orchestrator_widgets::AnnotationView> = store
            .annotations
            .iter()
            .map(|(path, ann)| orchestrator_widgets::AnnotationView {
                path: path.clone(),
                purpose: ann.purpose.clone(),
            })
            .collect();
        self.widgets.context_page.set_annotations(annotations);

        let deep: Vec<orchestrator_widgets::DeepContextView> = store
            .deep_contexts
            .iter()
            .map(|d| orchestrator_widgets::DeepContextView {
                name: d.name.clone(),
                last_verified: d.last_verified.clone(),
            })
            .collect();
        self.widgets.context_page.set_deep_contexts(deep);
    }

    pub fn refresh_prompts(&mut self) {
        let skills = self.skill_registry.skills_info();
        self.widgets.prompts_page.set_skills(skills);

        let prompts = load_system_prompts();
        self.widgets.prompts_page.set_system_prompts(prompts);
    }

    fn toggle_system_prompt(&self, name: &str) {
        let prompts_dir = crate::config::dir_path().join("prompts");
        let path = prompts_dir.join(format!("{}.json", name));
        if let Ok(contents) = std::fs::read_to_string(&path) {
            if let Ok(mut value) = serde_json::from_str::<serde_json::Value>(&contents) {
                let currently = value.get("enabled").and_then(|v| v.as_bool()).unwrap_or(true);
                value["enabled"] = serde_json::Value::Bool(!currently);
                if let Ok(json) = serde_json::to_string_pretty(&value) {
                    let _ = std::fs::write(&path, json);
                }
            }
        }
    }
}

/// Load system prompt JSON files from `.agent-orchestrator/prompts/`.
/// Each file: `{ "name": "...", "description": "...", "content": "...", "provider": "...", "enabled": true }`
fn load_system_prompts() -> Vec<orchestrator_widgets::SystemPromptEntry> {
    let prompts_dir = crate::config::dir_path().join("prompts");
    let Ok(entries) = std::fs::read_dir(&prompts_dir) else {
        return Vec::new();
    };

    let mut prompts = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }
        let Ok(contents) = std::fs::read_to_string(&path) else {
            continue;
        };
        let Ok(value) = serde_json::from_str::<serde_json::Value>(&contents) else {
            continue;
        };
        let name = value
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown")
            .to_string();
        let description = value
            .get("description")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let content = value
            .get("content")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let provider = value
            .get("provider")
            .and_then(|v| v.as_str())
            .unwrap_or("claude")
            .to_string();
        let enabled = value
            .get("enabled")
            .and_then(|v| v.as_bool())
            .unwrap_or(true);

        prompts.push(orchestrator_widgets::SystemPromptEntry {
            name,
            description,
            content,
            provider,
            enabled,
        });
    }
    prompts.sort_by(|a, b| a.name.cmp(&b.name));
    prompts
}

/// Combine a system prompt and repository context map into a single system prompt string.
/// Returns None if both are empty.
fn build_system_prompt(prompt: Option<&str>, map_content: &str) -> Option<String> {
    let has_prompt = prompt.is_some_and(|p| !p.is_empty());
    let has_map = !map_content.is_empty();

    match (has_prompt, has_map) {
        (true, true) => Some(format!(
            "{}\n\n---\n\n# Repository Context\n\n{}",
            prompt.unwrap(),
            map_content
        )),
        (true, false) => Some(prompt.unwrap().to_string()),
        (false, true) => Some(format!("# Repository Context\n\n{}", map_content)),
        (false, false) => None,
    }
}

fn resolve_provider(name: &str) -> Box<dyn Provider> {
    match name {
        "claude" => Box::new(ClaudeProvider),
        other => panic!("unknown provider: {}", other),
    }
}

fn config_to_fields(config: &crate::config::Config) -> Vec<SettingsField> {
    vec![
        SettingsField {
            label: "MCP Port",
            key: "mcp_port",
            value: config.mcp_port.to_string(),
            description: "Port for the MCP JSON-RPC server",
            kind: FieldKind::Text,
        },
        SettingsField {
            label: "Default Provider",
            key: "default_provider",
            value: config.default_provider.clone(),
            description: "Provider for new agents (ctrl+n)",
            kind: FieldKind::Toggle(vec!["claude".into()]),
        },
        SettingsField {
            label: "Claude",
            key: "claude_enabled",
            value: config.claude_enabled.to_string(),
            description: "Enable Claude provider",
            kind: FieldKind::Toggle(vec!["true".into(), "false".into()]),
        },
        SettingsField {
            label: "Display Mode",
            key: "display_mode",
            value: config.display_mode.clone(),
            description: "native = our TUI, provider = provider's own TUI",
            kind: FieldKind::Toggle(vec!["native".into(), "provider".into()]),
        },
    ]
}

fn fields_to_config(values: &[(&str, &str)]) -> crate::config::Config {
    let mut config = crate::config::Config::default();
    for &(key, val) in values {
        match key {
            "mcp_port" => {
                if let Ok(port) = val.parse::<u16>() {
                    config.mcp_port = port;
                }
            }
            "default_provider" => {
                config.default_provider = val.to_string();
            }
            "claude_enabled" => {
                config.claude_enabled = val == "true";
            }
            "display_mode" => {
                config.display_mode = val.to_string();
            }
            _ => {}
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
