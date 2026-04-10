use orchestrator_types::{
    extract_tool_calls, AgentConfig, ContentBlock, GenerateRequest, GenerateResponse, Message,
    MessageContent, Role, SessionId, SessionStatus, TokenUsage, ToolCall, ToolDefinition,
    ToolOutput, ToolResultContent,
};

/// A managed agent conversation session.
///
/// Tracks the conversation history and state for orchestrator-driven
/// agent loops. The orchestrator creates a Session, sends messages,
/// processes tool calls, and repeats until the model stops.
///
/// This is NOT used for native CLI mode (where the CLI owns the loop).
pub struct Session {
    pub id: SessionId,
    pub config: AgentConfig,
    pub status: SessionStatus,
    pub messages: Vec<Message>,
    pub tools: Vec<ToolDefinition>,
    pub total_usage: TokenUsage,
    pub turn_count: u32,
    pub pending_tool_calls: Vec<ToolCall>,
    /// Provider-side session ID for resuming conversations (e.g. Claude CLI session).
    pub provider_session_id: Option<String>,
}

impl Session {
    /// Create a new session from an agent config.
    pub fn new(config: AgentConfig, tools: Vec<ToolDefinition>) -> Self {
        Self {
            id: SessionId::new(),
            config,
            status: SessionStatus::Ready,
            messages: Vec::new(),
            tools,
            total_usage: TokenUsage {
                input_tokens: 0,
                output_tokens: 0,
                cache_creation_input_tokens: None,
                cache_read_input_tokens: None,
            },
            turn_count: 0,
            pending_tool_calls: Vec::new(),
            provider_session_id: None,
        }
    }

    /// Add a user message and prepare a GenerateRequest.
    /// Transitions: Ready -> Generating.
    /// Returns None if the session is not in Ready state.
    pub fn send_message(&mut self, content: MessageContent) -> Option<GenerateRequest> {
        if self.status != SessionStatus::Ready {
            return None;
        }
        self.messages.push(Message {
            role: Role::User,
            content,
        });
        self.status = SessionStatus::Generating;
        Some(self.build_request())
    }

    /// Process a generation response.
    /// If the model returned tool calls: Generating -> ToolPending.
    /// If the model stopped normally: Generating -> Ready.
    /// Returns the list of tool calls to execute (empty if none).
    pub fn receive_response(&mut self, response: GenerateResponse) -> Vec<ToolCall> {
        if self.status != SessionStatus::Generating {
            return Vec::new();
        }

        self.total_usage.input_tokens += response.usage.input_tokens;
        self.total_usage.output_tokens += response.usage.output_tokens;
        if let Some(v) = response.usage.cache_creation_input_tokens {
            *self
                .total_usage
                .cache_creation_input_tokens
                .get_or_insert(0) += v;
        }
        if let Some(v) = response.usage.cache_read_input_tokens {
            *self.total_usage.cache_read_input_tokens.get_or_insert(0) += v;
        }
        self.turn_count += 1;

        self.messages.push(Message {
            role: Role::Assistant,
            content: MessageContent::Blocks(response.content.clone()),
        });

        let tool_calls = extract_tool_calls(&response.content);

        if tool_calls.is_empty() {
            self.status = SessionStatus::Ready;
        } else {
            self.pending_tool_calls = tool_calls.clone();
            self.status = SessionStatus::ToolPending;
        }

        tool_calls
    }

    /// Submit tool results and prepare the next GenerateRequest.
    /// Transitions: ToolPending -> Generating.
    /// Returns None if the session is not in ToolPending state.
    pub fn submit_tool_results(&mut self, results: Vec<ToolOutput>) -> Option<GenerateRequest> {
        if self.status != SessionStatus::ToolPending {
            return None;
        }

        let blocks: Vec<ContentBlock> = results
            .into_iter()
            .map(|r| ContentBlock::ToolResult {
                tool_use_id: r.tool_call_id,
                content: ToolResultContent::Text(r.content),
                is_error: r.is_error,
            })
            .collect();

        self.messages.push(Message {
            role: Role::User,
            content: MessageContent::Blocks(blocks),
        });

        self.pending_tool_calls.clear();
        self.status = SessionStatus::Generating;
        Some(self.build_request())
    }

    /// Mark the session as complete.
    pub fn complete(&mut self) {
        self.status = SessionStatus::Complete;
    }

    /// Mark the session as aborted.
    pub fn abort(&mut self) {
        self.status = SessionStatus::Aborted;
    }

    fn build_request(&self) -> GenerateRequest {
        GenerateRequest {
            system: self.config.system_prompt.clone(),
            messages: self.messages.clone(),
            max_tokens: self.config.max_tokens,
            model: self.config.model.clone(),
            temperature: self.config.temperature,
            tools: self.tools.clone(),
            stop_sequences: Vec::new(),
            resume_session_id: self.provider_session_id.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use orchestrator_types::{AgentMode, StopReason};

    fn test_config() -> AgentConfig {
        AgentConfig {
            name: "test".into(),
            provider: "test".into(),
            model: "test-model".into(),
            max_tokens: 1024,
            system_prompt: Some("Be helpful.".into()),
            temperature: None,
            mode: AgentMode::Managed,
            provider_config: serde_json::json!({}),
        }
    }

    fn text_response(text: &str) -> GenerateResponse {
        GenerateResponse {
            content: vec![ContentBlock::Text { text: text.into() }],
            model: "test-model".into(),
            stop_reason: StopReason::EndTurn,
            usage: TokenUsage {
                input_tokens: 100,
                output_tokens: 50,
                cache_creation_input_tokens: None,
                cache_read_input_tokens: None,
            },
            request_id: None,
            provider_session_id: None,
        }
    }

    fn tool_response() -> GenerateResponse {
        GenerateResponse {
            content: vec![
                ContentBlock::Text {
                    text: "Let me check.".into(),
                },
                ContentBlock::ToolUse {
                    id: "tc_1".into(),
                    name: "bash".into(),
                    input: serde_json::json!({"command": "ls"}),
                },
            ],
            model: "test-model".into(),
            stop_reason: StopReason::ToolUse,
            usage: TokenUsage {
                input_tokens: 200,
                output_tokens: 100,
                cache_creation_input_tokens: None,
                cache_read_input_tokens: None,
            },
            request_id: None,
            provider_session_id: None,
        }
    }

    #[test]
    fn new_session_is_ready() {
        let session = Session::new(test_config(), vec![]);
        assert_eq!(session.status, SessionStatus::Ready);
        assert!(session.messages.is_empty());
        assert_eq!(session.turn_count, 0);
    }

    #[test]
    fn send_message_transitions_to_generating() {
        let mut session = Session::new(test_config(), vec![]);
        let req = session.send_message(MessageContent::Text("Hello".into()));
        assert!(req.is_some());
        assert_eq!(session.status, SessionStatus::Generating);
        assert_eq!(session.messages.len(), 1);

        let req = req.unwrap();
        assert_eq!(req.system.as_deref(), Some("Be helpful."));
        assert_eq!(req.model, "test-model");
    }

    #[test]
    fn send_message_fails_when_not_ready() {
        let mut session = Session::new(test_config(), vec![]);
        session.send_message(MessageContent::Text("Hello".into()));
        // Now in Generating state
        let req = session.send_message(MessageContent::Text("Again".into()));
        assert!(req.is_none());
    }

    #[test]
    fn receive_text_response_returns_to_ready() {
        let mut session = Session::new(test_config(), vec![]);
        session.send_message(MessageContent::Text("Hello".into()));

        let tool_calls = session.receive_response(text_response("Hi there!"));
        assert!(tool_calls.is_empty());
        assert_eq!(session.status, SessionStatus::Ready);
        assert_eq!(session.turn_count, 1);
        assert_eq!(session.messages.len(), 2); // user + assistant
        assert_eq!(session.total_usage.input_tokens, 100);
        assert_eq!(session.total_usage.output_tokens, 50);
    }

    #[test]
    fn receive_tool_response_transitions_to_tool_pending() {
        let mut session = Session::new(test_config(), vec![]);
        session.send_message(MessageContent::Text("List files".into()));

        let tool_calls = session.receive_response(tool_response());
        assert_eq!(tool_calls.len(), 1);
        assert_eq!(tool_calls[0].name, "bash");
        assert_eq!(session.status, SessionStatus::ToolPending);
        assert_eq!(session.pending_tool_calls.len(), 1);
    }

    #[test]
    fn submit_tool_results_transitions_to_generating() {
        let mut session = Session::new(test_config(), vec![]);
        session.send_message(MessageContent::Text("List files".into()));
        session.receive_response(tool_response());

        let results = vec![ToolOutput {
            tool_call_id: "tc_1".into(),
            content: "file1.rs\nfile2.rs".into(),
            is_error: false,
        }];
        let req = session.submit_tool_results(results);
        assert!(req.is_some());
        assert_eq!(session.status, SessionStatus::Generating);
        assert!(session.pending_tool_calls.is_empty());
        assert_eq!(session.messages.len(), 3); // user + assistant + tool results
    }

    #[test]
    fn submit_tool_results_fails_when_not_tool_pending() {
        let mut session = Session::new(test_config(), vec![]);
        let req = session.submit_tool_results(vec![]);
        assert!(req.is_none());
    }

    #[test]
    fn full_tool_loop() {
        let mut session = Session::new(test_config(), vec![]);

        // User sends message
        let req = session.send_message(MessageContent::Text("List files".into()));
        assert!(req.is_some());

        // Model wants to use a tool
        let tool_calls = session.receive_response(tool_response());
        assert_eq!(tool_calls.len(), 1);

        // Execute tool and submit results
        let req = session.submit_tool_results(vec![ToolOutput {
            tool_call_id: "tc_1".into(),
            content: "file1.rs".into(),
            is_error: false,
        }]);
        assert!(req.is_some());

        // Model responds with text
        let tool_calls = session.receive_response(text_response("Found file1.rs"));
        assert!(tool_calls.is_empty());
        assert_eq!(session.status, SessionStatus::Ready);
        assert_eq!(session.turn_count, 2);
        assert_eq!(session.total_usage.input_tokens, 300);
        assert_eq!(session.total_usage.output_tokens, 150);
    }

    #[test]
    fn complete_and_abort() {
        let mut session = Session::new(test_config(), vec![]);
        session.complete();
        assert_eq!(session.status, SessionStatus::Complete);

        let mut session = Session::new(test_config(), vec![]);
        session.abort();
        assert_eq!(session.status, SessionStatus::Aborted);
    }

    #[test]
    fn usage_accumulates_across_turns() {
        let mut session = Session::new(test_config(), vec![]);
        session.send_message(MessageContent::Text("Hello".into()));
        session.receive_response(text_response("Hi"));

        session.send_message(MessageContent::Text("Again".into()));
        session.receive_response(text_response("Hello again"));

        assert_eq!(session.turn_count, 2);
        assert_eq!(session.total_usage.input_tokens, 200);
        assert_eq!(session.total_usage.output_tokens, 100);
    }
}
