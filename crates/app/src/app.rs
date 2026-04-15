use crate::helpers::walk_directory;
use crate::input::{handle_key, handle_mouse};
use crate::types::{DiffResult, EventProxy, FileEvent, TermSize};
use alacritty_terminal::grid::Scroll;
use alacritty_terminal::term::{Config as TermConfig, Term};
use alacritty_terminal::vte;
use anyhow::Result;
use auriga_agent::{AgentConfig, AgentMode, GenerateRequest, GenerateResponse, Provider, Session};
use auriga_claude_log::{to_turn_builder, ClaudeWatchEvent, ClaudeWatchHandle};
use auriga_core::{AgentId, AgentStore, DisplayMode, FileTree, FocusState, TraceStore, TurnStore};
use auriga_grid::{CellRect, Grid};
use auriga_mcp::{AgentInfo, McpEvent, McpRequest, McpResponse};
use auriga_pty::PtyHandle;
use auriga_skills::SkillRegistry;
use auriga_storage::{Database, StorageHandle};
use auriga_widgets::{
    DbMetadataView, FieldKind, QueryResultView, SettingsField, SettingsSection, TableInfoView,
    WidgetAction, WidgetRegistry,
};
use crossterm::event::Event;
use ratatui::layout::Rect;
use std::collections::HashMap;
use std::path::PathBuf;

/// When false, always use the built-in default layout instead of reading from config.json.
const USE_FS_LAYOUT: bool = false;

/// Pages hidden based on feature flags.
pub fn hidden_pages() -> Vec<auriga_core::Page> {
    Vec::new()
}
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
    pub grid: auriga_grid::Grid,
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
    pub skill_registry: SkillRegistry,
    pub sessions: HashMap<AgentId, Session>,
    pub generate_tx: mpsc::Sender<(AgentId, String, GenerateRequest)>,
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
        generate_tx: mpsc::Sender<(AgentId, String, GenerateRequest)>,
        generate_rx: mpsc::Receiver<(AgentId, Result<GenerateResponse, String>)>,
        config: &crate::config::Config,
    ) -> Self {
        let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        let mut file_tree = FileTree::new(cwd.clone());
        file_tree.set_entries(walk_directory(&cwd));

        let mut widgets = WidgetRegistry::new();
        widgets.settings_page.force_reload(
            config_to_fields(config),
            crate::config::file_path().display().to_string(),
            crate::config::modified_at(),
        );

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
            skill_registry: SkillRegistry::new(),
            sessions: HashMap::new(),
            generate_tx,
            generate_rx,
            running: true,
        }
    }

    /// Register built-in default skills.
    pub fn register_default_skills(&mut self) {
        self.skill_registry
            .register(Box::new(auriga_skills::CodeReviewSkill));
    }

    pub fn pane_size(&self) -> (u16, u16) {
        for cell in &self.last_cell_rects {
            if cell.widget == auriga_grid::WidgetId::AgentPane {
                let cols = cell.rect.width.saturating_sub(2).max(1);
                let rows = cell.rect.height.saturating_sub(2).max(1);
                return (cols, rows);
            }
        }
        let (cols, rows) = crossterm::terminal::size().unwrap_or((120, 40));
        (cols, rows)
    }

    /// Build the default AgentConfig for a given provider.
    pub fn default_agent_config(&self, provider_name: &str) -> AgentConfig {
        use auriga_agent::SystemPromptBuilder;

        let prompts = load_system_prompts();
        let prompt_content = prompts
            .iter()
            .find(|p| p.enabled && p.provider == provider_name)
            .map(|p| p.content.as_str())
            .unwrap_or("");

        let system_prompt = SystemPromptBuilder::new().section(prompt_content).build();

        let provider_config = crate::config::load_claude_config();

        AgentConfig {
            name: "agent".into(),
            provider: provider_name.into(),
            model: String::new(),
            max_tokens: 0,
            system_prompt,
            temperature: None,
            mode: AgentMode::NativeCli,
            provider_config,
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
                // Provider mode: spawn an interactive shell, then pipe the
                // provider's CLI command into it as the first line. When the
                // AI tool exits, the user is left at a live shell — the pane
                // is an actual terminal that happens to auto-launch claude/codex.
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

                let mut env: Vec<(&str, &str)> = vec![("AURIGA_AGENT_NAME", &agent_name)];
                let env_owned: Vec<(String, String)> = spec.env.clone();
                for (k, v) in &env_owned {
                    env.push((k.as_str(), v.as_str()));
                }

                let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/zsh".into());
                let mut pty = PtyHandle::spawn_with_args(&shell, &["-i"], &cwd, cols, rows, &env)?;

                // Build the command line to inject.
                let mut cmd_line = shell_quote(&spec.program);
                for arg in &spec.args {
                    cmd_line.push(' ');
                    cmd_line.push_str(&shell_quote(arg));
                }
                cmd_line.push('\n');
                let _ = pty.write_input(cmd_line.as_bytes());
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

    /// Spawn a plain interactive shell as a new agent, with no AI tool attached.
    /// Mirrors `spawn_agent`'s Provider-mode branch but skips the command injection.
    pub fn spawn_shell(&mut self) -> Result<AgentId> {
        let id = self.agents.create("shell");
        if let Some(agent) = self.agents.get_mut(id) {
            agent.display_mode = DisplayMode::Provider;
        }

        let agent_name = self
            .agents
            .get(id)
            .expect("agent just created")
            .name
            .clone();
        let cwd = std::env::current_dir()?;
        let (cols, rows) = self.pane_size();

        let env: Vec<(&str, &str)> = vec![("AURIGA_AGENT_NAME", &agent_name)];
        let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/zsh".into());
        let pty = PtyHandle::spawn_with_args(&shell, &["-i"], &cwd, cols, rows, &env)?;

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

        // Clean up maps that reference this agent (prevents unbounded growth)
        if let Some(agent) = self.agents.get(id) {
            if let Some(pid) = agent.child_pid {
                self.pid_map.remove(&pid);
            }
        }
        self.session_map.retain(|_, &mut aid| aid != id);
        self.sessions.remove(&id);

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
                        agent.status = auriga_core::AgentStatus::Working;
                    } else {
                        let still_active = agent
                            .last_active_at
                            .is_some_and(|t| now.duration_since(t) < DEBOUNCE);
                        if !still_active {
                            agent.status = auriga_core::AgentStatus::Idle;
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
                            if let auriga_core::TurnMeta::Assistant(ref meta) = turn.meta {
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

                    // Immediately flush to SQLite
                    if let Some(trace) = self.traces.active_trace(agent_id) {
                        let trace_clone = trace.clone();
                        let turns: Vec<_> = self
                            .turns
                            .turns_for(agent_id)
                            .into_iter()
                            .filter(|t| t.session_id.as_deref() == Some(&entry.session_id))
                            .cloned()
                            .collect();
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
            self.scroll_term_to_bottom(id);
            if let Some(pty) = self.ptys.get_mut(&id) {
                if let Err(e) = pty.write_input(data) {
                    tracing::warn!(error = %e, "PTY write failed");
                }
            }
        }
    }

    fn scroll_term_to_bottom(&mut self, id: AgentId) {
        if let Some(term) = self.terms.get_mut(&id) {
            if term.grid().display_offset() > 0 {
                term.scroll_display(Scroll::Bottom);
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

        let content = auriga_core::MessageContent::Text(text.clone());
        if let Some(request) = session.send_message(content.clone()) {
            // Insert user turn into TurnStore via bridge
            let now = chrono_now();
            let turn_builder = auriga_agent::user_message_to_turn(
                &auriga_agent::Message {
                    role: auriga_agent::Role::User,
                    content,
                },
                &session.id,
                &now,
            );
            self.turns.insert(agent_id, turn_builder);

            let provider_name = self
                .agents
                .get(agent_id)
                .map(|a| a.provider.clone())
                .unwrap_or_default();

            // Ensure trace exists
            if self.traces.active_trace(agent_id).is_none() {
                self.traces.create(
                    agent_id,
                    session.id.0.to_string(),
                    provider_name.clone(),
                    now,
                );
            }

            // Send to generation thread
            let _ = self.generate_tx.send((agent_id, provider_name, request));
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
                            auriga_agent::response_to_turn(&response, &session.id, &now);
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

    pub fn hit_test(&self, col: u16, row: u16) -> Option<(auriga_grid::WidgetId, u16, u16)> {
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
        use auriga_widgets::agent_pane::{compute_grid_rects, terminal_size_from_rect, PaneMode};

        // Get the AgentPane rect
        let pane_rect = self
            .last_cell_rects
            .iter()
            .find(|c| c.widget == auriga_grid::WidgetId::AgentPane)
            .map(|c| c.rect);

        let Some(pane_rect) = pane_rect else {
            return;
        };

        let agents = self.agents.list();
        let agent_count = agents.len().min(6); // Grid mode shows max 6
        let mode = self.widgets.agent_pane.mode;

        // Compute per-agent terminal sizes
        let agent_sizes: Vec<(AgentId, u16, u16)> = match mode {
            PaneMode::Focused => {
                // In focused mode, active agent gets full pane
                if let Some(id) = self.focus.active_agent {
                    let (cols, rows) = terminal_size_from_rect(pane_rect);
                    vec![(id, cols, rows)]
                } else {
                    vec![]
                }
            }
            PaneMode::Grid => {
                if agent_count == 0 {
                    vec![]
                } else {
                    let sub_rects = compute_grid_rects(pane_rect, agent_count);
                    agents
                        .iter()
                        .take(agent_count)
                        .zip(sub_rects.iter())
                        .map(|(agent, rect)| {
                            let (cols, rows) = terminal_size_from_rect(*rect);
                            (agent.id, cols, rows)
                        })
                        .collect()
                }
            }
        };

        // Resize PTYs and terminals to their computed sizes
        for (id, cols, rows) in agent_sizes {
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
                    .set_mode(auriga_widgets::agent_pane::PaneMode::Grid);
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
                let base = crate::config::load().unwrap_or_default();
                let config = fields_to_config(base, &self.widgets.settings_page.field_values());
                if crate::config::save(&config).is_ok() {
                    self.widgets
                        .settings_page
                        .mark_saved(crate::config::modified_at());
                }
            }
            WidgetAction::DownloadSkill(name) => {
                if let Err(e) = self.download_skill(&name) {
                    tracing::error!(error = %e, skill = %name, "download_skill failed");
                }
                self.refresh_prompts();
            }
            WidgetAction::DeleteSkill(name) => {
                if let Err(e) = self.delete_skill(&name) {
                    tracing::error!(error = %e, skill = %name, "delete_skill failed");
                }
                self.refresh_prompts();
            }
            WidgetAction::ToggleSystemPrompt(name) => {
                self.toggle_system_prompt(&name);
                self.refresh_prompts();
            }
        }
    }

    pub fn toggle_pane_mode(&mut self) {
        let new_mode = match self.widgets.agent_pane.mode {
            auriga_widgets::agent_pane::PaneMode::Grid => {
                auriga_widgets::agent_pane::PaneMode::Focused
            }
            auriga_widgets::agent_pane::PaneMode::Focused => {
                auriga_widgets::agent_pane::PaneMode::Grid
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

    pub fn refresh_prompts(&mut self) {
        let root = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        let is_downloaded = |name: &str| crate::skills_storage::is_downloaded(&root, name);
        let skills = self.skill_registry.skills_info(&is_downloaded);
        self.widgets.prompts_page.set_skills(skills);

        let prompts = load_system_prompts();
        self.widgets.prompts_page.set_system_prompts(prompts);
    }

    pub fn refresh_settings(&mut self) {
        let config = crate::config::load().unwrap_or_default();
        self.widgets.settings_page.sync_from_disk(
            config_to_fields(&config),
            crate::config::file_path().display().to_string(),
            crate::config::modified_at(),
        );
    }

    /// Write the named skill's `SKILL.md` to the current project's
    /// `.claude/skills/` and `.agents/skills/` directories.
    fn download_skill(&self, name: &str) -> anyhow::Result<()> {
        let skill = self
            .skill_registry
            .get(name)
            .ok_or_else(|| anyhow::anyhow!("unknown skill: '{name}'"))?;
        let root = std::env::current_dir()?;
        crate::skills_storage::write_skill(&root, skill)?;
        Ok(())
    }

    /// Remove the named skill from both on-disk locations.
    fn delete_skill(&self, name: &str) -> anyhow::Result<()> {
        let root = std::env::current_dir()?;
        crate::skills_storage::delete_skill(&root, name)?;
        Ok(())
    }

    fn toggle_system_prompt(&self, name: &str) {
        let prompts_dir = crate::config::dir_path().join("prompts");
        let path = prompts_dir.join(format!("{}.json", name));
        if let Ok(contents) = std::fs::read_to_string(&path) {
            if let Ok(mut value) = serde_json::from_str::<serde_json::Value>(&contents) {
                let currently = value
                    .get("enabled")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(true);
                value["enabled"] = serde_json::Value::Bool(!currently);
                if let Ok(json) = serde_json::to_string_pretty(&value) {
                    let _ = std::fs::write(&path, json);
                }
            }
        }
    }
}

/// Load system prompt JSON files from `.auriga/prompts/`.
/// Each file: `{ "name": "...", "description": "...", "content": "...", "provider": "...", "enabled": true }`
fn load_system_prompts() -> Vec<auriga_widgets::SystemPromptEntry> {
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

        prompts.push(auriga_widgets::SystemPromptEntry {
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

fn resolve_provider(name: &str) -> Box<dyn Provider> {
    auriga_agent::providers::resolve(name)
}

/// Quote a single argument for safe injection into an interactive shell.
/// Bare words of `[A-Za-z0-9_\-./=:]` pass through; anything else is
/// single-quoted with `'` → `'\''` escaping.
fn shell_quote(s: &str) -> String {
    if s.is_empty() {
        return "''".into();
    }
    let safe = s
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || "-_.=:/".contains(c));
    if safe {
        return s.to_string();
    }
    let escaped = s.replace('\'', "'\\''");
    format!("'{}'", escaped)
}

#[cfg(test)]
mod shell_quote_tests {
    use super::shell_quote;

    #[test]
    fn bare_words_pass_through() {
        assert_eq!(shell_quote("claude"), "claude");
        assert_eq!(shell_quote("--model"), "--model");
        assert_eq!(shell_quote("gpt-5"), "gpt-5");
        assert_eq!(shell_quote("/tmp/mcp.json"), "/tmp/mcp.json");
        assert_eq!(shell_quote("key=value"), "key=value");
    }

    #[test]
    fn empty_becomes_empty_quotes() {
        assert_eq!(shell_quote(""), "''");
    }

    #[test]
    fn spaces_get_quoted() {
        assert_eq!(shell_quote("hello world"), "'hello world'");
    }

    #[test]
    fn single_quotes_escaped() {
        assert_eq!(shell_quote("it's"), "'it'\\''s'");
    }

    #[test]
    fn special_chars_quoted() {
        assert_eq!(shell_quote("a$b"), "'a$b'");
        assert_eq!(shell_quote("a;b"), "'a;b'");
        assert_eq!(shell_quote("a&b"), "'a&b'");
    }
}

const ON: &str = "on";
const OFF: &str = "off";
const EMPTY: &str = "—";
const DEFAULT: &str = "default";

fn toggle(options: &[&str]) -> FieldKind {
    FieldKind::Toggle(options.iter().map(|s| (*s).to_string()).collect())
}

fn opt_str(value: &Option<String>) -> String {
    value.clone().unwrap_or_else(|| EMPTY.into())
}

fn effort_str(value: &Option<auriga_core::EffortLevel>) -> String {
    use auriga_core::EffortLevel;
    match value {
        None => EMPTY.into(),
        Some(EffortLevel::Low) => "low".into(),
        Some(EffortLevel::Medium) => "medium".into(),
        Some(EffortLevel::High) => "high".into(),
        Some(EffortLevel::Max) => "max".into(),
    }
}

fn permission_mode_str(value: &Option<auriga_core::PermissionMode>) -> String {
    use auriga_core::PermissionMode;
    match value {
        None => EMPTY.into(),
        Some(PermissionMode::Default) => "default".into(),
        Some(PermissionMode::AcceptEdits) => "acceptEdits".into(),
        Some(PermissionMode::Plan) => "plan".into(),
        Some(PermissionMode::Auto) => "auto".into(),
        Some(PermissionMode::DontAsk) => "dontAsk".into(),
        Some(PermissionMode::BypassPermissions) => "bypassPermissions".into(),
    }
}

fn tri_bool_str(value: Option<bool>) -> String {
    match value {
        None => EMPTY.into(),
        Some(true) => ON.into(),
        Some(false) => OFF.into(),
    }
}

fn bool_str(value: bool) -> String {
    if value {
        ON.into()
    } else {
        OFF.into()
    }
}

fn budget_str(value: Option<f64>) -> String {
    value.map(|v| v.to_string()).unwrap_or_else(|| EMPTY.into())
}

fn nested_setting(
    config: &crate::config::Config,
    field: fn(&auriga_core::ClaudeSettings) -> Option<bool>,
) -> Option<bool> {
    config.claude.settings.as_ref().and_then(field)
}

fn config_to_fields(config: &crate::config::Config) -> Vec<SettingsField> {
    vec![
        SettingsField {
            section: SettingsSection::General,
            label: "MCP Port",
            key: "mcp_port",
            value: config.mcp_port.to_string(),
            description: "Port for the MCP JSON-RPC server",
            kind: FieldKind::Text,
            detail: vec![],
        },
        SettingsField {
            section: SettingsSection::General,
            label: "Font Size",
            key: "font_size",
            value: config.font_size.to_string(),
            description: "Global font size (8-32)",
            kind: FieldKind::Text,
            detail: vec![],
        },
        SettingsField {
            section: SettingsSection::ClaudeSettings,
            label: "Model",
            key: "claude.model",
            value: opt_str(&config.claude.model),
            description: "Model override for new Claude agents",
            kind: toggle(&[EMPTY, "sonnet", "opus", "haiku"]),
            detail: vec![],
        },
        SettingsField {
            section: SettingsSection::ClaudeSettings,
            label: "Thinking Effort",
            key: "claude.effort",
            value: effort_str(&config.claude.effort),
            description: "Reasoning effort level",
            kind: toggle(&[EMPTY, "low", "medium", "high", "max"]),
            detail: vec![],
        },
        SettingsField {
            section: SettingsSection::ClaudeSettings,
            label: "Permission Mode",
            key: "claude.permission_mode",
            value: permission_mode_str(&config.claude.permission_mode),
            description: "How the CLI asks for tool permissions",
            kind: toggle(&[
                DEFAULT,
                "auto",
                "acceptEdits",
                "plan",
                "dontAsk",
                "bypassPermissions",
            ]),
            detail: vec![],
        },
        SettingsField {
            section: SettingsSection::ClaudeSettings,
            label: "Max Budget (USD)",
            key: "claude.max_budget_usd",
            value: budget_str(config.claude.max_budget_usd),
            description: "Cap spend per session; empty disables",
            kind: FieldKind::Text,
            detail: vec![],
        },
        SettingsField {
            section: SettingsSection::ClaudeSettings,
            label: "Auto Memory",
            key: "claude.settings.auto_memory_enabled",
            value: tri_bool_str(nested_setting(config, |s| s.auto_memory_enabled)),
            description: "Let Claude build persistent memory across sessions",
            kind: toggle(&[DEFAULT, ON, OFF]),
            detail: vec![],
        },
        SettingsField {
            section: SettingsSection::ClaudeSettings,
            label: "Include Git Instructions",
            key: "claude.settings.include_git_instructions",
            value: tri_bool_str(nested_setting(config, |s| s.include_git_instructions)),
            description: "Inject git usage guidance into the system prompt",
            kind: toggle(&[DEFAULT, ON, OFF]),
            detail: vec![],
        },
        SettingsField {
            section: SettingsSection::ClaudeSettings,
            label: "Respect .gitignore",
            key: "claude.settings.respect_gitignore",
            value: tri_bool_str(nested_setting(config, |s| s.respect_gitignore)),
            description: "Hide gitignored files from file tools",
            kind: toggle(&[DEFAULT, ON, OFF]),
            detail: vec![],
        },
        SettingsField {
            section: SettingsSection::ClaudeSettings,
            label: "Verbose Output",
            key: "claude.verbose",
            value: bool_str(config.claude.verbose),
            description: "Pass --verbose to the CLI",
            kind: toggle(&[ON, OFF]),
            detail: vec![],
        },
        SettingsField {
            section: SettingsSection::ClaudeSettings,
            label: "Disable Slash Commands",
            key: "claude.disable_slash_commands",
            value: bool_str(config.claude.disable_slash_commands),
            description: "Turn off all slash commands and skills",
            kind: toggle(&[ON, OFF]),
            detail: vec![],
        },
    ]
}

fn parse_effort(val: &str) -> Option<auriga_core::EffortLevel> {
    use auriga_core::EffortLevel;
    match val {
        "low" => Some(EffortLevel::Low),
        "medium" => Some(EffortLevel::Medium),
        "high" => Some(EffortLevel::High),
        "max" => Some(EffortLevel::Max),
        _ => None,
    }
}

fn parse_permission_mode(val: &str) -> Option<auriga_core::PermissionMode> {
    use auriga_core::PermissionMode;
    match val {
        "default" => Some(PermissionMode::Default),
        "acceptEdits" => Some(PermissionMode::AcceptEdits),
        "plan" => Some(PermissionMode::Plan),
        "auto" => Some(PermissionMode::Auto),
        "dontAsk" => Some(PermissionMode::DontAsk),
        "bypassPermissions" => Some(PermissionMode::BypassPermissions),
        _ => None,
    }
}

fn parse_tri_bool(val: &str) -> Option<bool> {
    match val {
        ON => Some(true),
        OFF => Some(false),
        _ => None,
    }
}

fn ensure_settings(config: &mut crate::config::Config) -> &mut auriga_core::ClaudeSettings {
    config
        .claude
        .settings
        .get_or_insert_with(auriga_core::ClaudeSettings::default)
}

fn fields_to_config(
    mut config: crate::config::Config,
    values: &[(&str, &str)],
) -> crate::config::Config {
    for &(key, val) in values {
        match key {
            "mcp_port" => {
                if let Ok(port) = val.parse::<u16>() {
                    config.mcp_port = port;
                }
            }
            "display_mode" => {
                config.display_mode = val.to_string();
            }
            "font_size" => {
                if let Ok(size) = val.parse::<u16>() {
                    config.font_size = size.clamp(8, 32);
                }
            }
            "claude.model" => {
                config.claude.model = if val == DEFAULT {
                    None
                } else {
                    Some(val.to_string())
                };
            }
            "claude.effort" => {
                config.claude.effort = parse_effort(val);
            }
            "claude.permission_mode" => {
                config.claude.permission_mode = parse_permission_mode(val);
            }
            "claude.max_budget_usd" => {
                let trimmed = val.trim();
                config.claude.max_budget_usd = if trimmed.is_empty() || trimmed == EMPTY {
                    None
                } else {
                    trimmed.parse::<f64>().ok()
                };
            }
            "claude.settings.auto_memory_enabled" => {
                ensure_settings(&mut config).auto_memory_enabled = parse_tri_bool(val);
            }
            "claude.settings.include_git_instructions" => {
                ensure_settings(&mut config).include_git_instructions = parse_tri_bool(val);
            }
            "claude.settings.respect_gitignore" => {
                ensure_settings(&mut config).respect_gitignore = parse_tri_bool(val);
            }
            "claude.verbose" => {
                config.claude.verbose = val == ON;
            }
            "claude.disable_slash_commands" => {
                config.claude.disable_slash_commands = val == ON;
            }
            _ => {}
        }
    }
    config
}

#[cfg(test)]
mod kill_agent_tests {
    use super::*;
    use alacritty_terminal::grid::Scroll;
    use alacritty_terminal::term::{Config as TermConfig, Term};
    use alacritty_terminal::vte::ansi::Processor;
    use auriga_core::{
        MessageContent, MessageType, TurnBuilder, TurnMeta, TurnRole, TurnStatus, UserMeta,
    };
    use serde_json::json;

    fn test_app() -> App {
        let (_input_tx, input_rx) = mpsc::channel::<Event>();
        let (_file_tx, file_rx) = mpsc::channel::<FileEvent>();
        let (_diff_tx, diff_rx) = mpsc::channel::<DiffResult>();
        let (real_diff_tx, _real_diff_rx) = mpsc::channel::<PathBuf>();
        let (_mcp_tx, mcp_rx) = mpsc::channel::<McpEvent>();
        let (gen_req_tx, _gen_req_rx) = mpsc::channel();
        let (_gen_resp_tx, gen_resp_rx) = mpsc::channel();

        let tmp = std::env::temp_dir().join(format!("auriga_test_{}.db", std::process::id()));
        let storage =
            auriga_storage::start_storage_thread(tmp.clone()).expect("storage thread failed");
        let db_reader = auriga_storage::Database::open_in_memory().expect("in-memory db failed");
        let config = crate::config::Config::default();

        App::new(
            input_rx,
            file_rx,
            real_diff_tx,
            diff_rx,
            mcp_rx,
            storage,
            db_reader,
            tmp,
            None,
            gen_req_tx,
            gen_resp_rx,
            &config,
        )
    }

    fn test_term() -> Term<EventProxy> {
        let term_config = TermConfig {
            scrolling_history: 10_000,
            ..Default::default()
        };
        let term_size = TermSize {
            cols: 80,
            lines: 24,
        };
        Term::new(term_config, &term_size, EventProxy)
    }

    fn user_turn(uuid: &str) -> TurnBuilder {
        TurnBuilder {
            uuid: uuid.to_string(),
            parent_uuid: None,
            session_id: None,
            timestamp: "2026-01-01T00:00:00Z".to_string(),
            message_type: MessageType::User,
            cwd: None,
            git_branch: None,
            role: TurnRole::User,
            content: MessageContent::Text("hello".to_string()),
            meta: TurnMeta::User(UserMeta {
                is_meta: false,
                is_compact_summary: false,
                source_tool_assistant_uuid: None,
            }),
            status: TurnStatus::Complete,
            extra: json!({}),
        }
    }

    #[test]
    fn kill_agent_cleans_session_map() {
        let mut app = test_app();
        let id = app.agents.create("claude");
        app.session_map.insert("sess-1".into(), id);
        app.session_map.insert("sess-2".into(), id);

        app.kill_agent(id);

        assert!(app.session_map.is_empty());
    }

    #[test]
    fn kill_agent_cleans_pid_map() {
        let mut app = test_app();
        let id = app.agents.create("claude");
        if let Some(agent) = app.agents.get_mut(id) {
            agent.child_pid = Some(12345);
        }
        app.pid_map.insert(12345, id);

        app.kill_agent(id);

        assert!(app.pid_map.is_empty());
    }

    #[test]
    fn kill_agent_cleans_sessions() {
        let mut app = test_app();
        let id = app.agents.create("claude");
        let agent_config = auriga_agent::AgentConfig {
            name: "test".into(),
            provider: "claude".into(),
            model: "test".into(),
            max_tokens: 1024,
            system_prompt: None,
            temperature: None,
            mode: auriga_agent::AgentMode::Generate,
            provider_config: serde_json::json!({}),
        };
        app.sessions
            .insert(id, Session::new(agent_config, Vec::new()));

        app.kill_agent(id);

        assert!(!app.sessions.contains_key(&id));
    }

    #[test]
    fn kill_agent_cleans_turns_and_traces() {
        let mut app = test_app();
        let id = app.agents.create("claude");
        app.turns.insert(id, user_turn("u1"));
        app.traces.create(
            id,
            "s1".into(),
            "claude".into(),
            "2026-01-01T00:00:00Z".into(),
        );

        app.kill_agent(id);

        assert_eq!(app.turns.count(), 0);
        assert_eq!(app.traces.count(), 0);
    }

    #[test]
    fn kill_agent_preserves_other_agents() {
        let mut app = test_app();
        let a = app.agents.create("claude");
        let b = app.agents.create("claude");

        app.session_map.insert("sess-a".into(), a);
        app.session_map.insert("sess-b".into(), b);
        app.pid_map.insert(111, a);
        app.pid_map.insert(222, b);
        if let Some(agent) = app.agents.get_mut(a) {
            agent.child_pid = Some(111);
        }
        if let Some(agent) = app.agents.get_mut(b) {
            agent.child_pid = Some(222);
        }
        app.turns.insert(a, user_turn("ua"));
        app.turns.insert(b, user_turn("ub"));

        app.kill_agent(a);

        assert!(app.agents.get(b).is_some());
        assert_eq!(app.session_map.len(), 1);
        assert_eq!(*app.session_map.get("sess-b").unwrap(), b);
        assert_eq!(app.pid_map.len(), 1);
        assert_eq!(app.turns.count(), 1);
    }

    #[test]
    fn spawn_kill_cycle_no_map_growth() {
        let mut app = test_app();

        for i in 0..100 {
            let id = app.agents.create("claude");
            let pid = 1000 + i;
            if let Some(agent) = app.agents.get_mut(id) {
                agent.child_pid = Some(pid);
            }
            app.pid_map.insert(pid, id);
            app.session_map.insert(format!("sess-{}", i), id);
            app.kill_agent(id);
        }

        assert!(app.pid_map.is_empty());
        assert!(app.session_map.is_empty());
        assert_eq!(app.agents.count(), 0);
    }

    #[test]
    fn write_to_active_snaps_scrolled_terminal_to_bottom() {
        let mut app = test_app();
        let id = app.agents.create("shell");
        app.focus.set_active_agent(id);

        let mut term = test_term();
        let mut parser: Processor<alacritty_terminal::vte::ansi::StdSyncHandler> = Processor::new();
        let history = (0..40).map(|i| format!("line-{i}\r\n")).collect::<String>();
        parser.advance(&mut term, history.as_bytes());
        term.scroll_display(Scroll::Top);
        assert!(term.grid().display_offset() > 0);
        app.terms.insert(id, term);

        app.write_to_active(b"x");

        let term = app.terms.get(&id).expect("term should exist");
        assert_eq!(term.grid().display_offset(), 0);
    }
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
