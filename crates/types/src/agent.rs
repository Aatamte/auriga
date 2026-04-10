use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct AgentId(pub Uuid);

impl AgentId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    /// Create an AgentId from a raw u128. Useful for tests with deterministic IDs.
    pub fn from_u128(val: u128) -> Self {
        Self(Uuid::from_u128(val))
    }
}

impl Default for AgentId {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AgentStatus {
    Idle,
    Working,
}

/// How this agent's output is displayed in the agent pane.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DisplayMode {
    /// Our own timeline UI showing turns, tool calls, thinking, tokens.
    Native,
    /// The provider's own TUI rendered via PTY terminal.
    Provider,
}

#[derive(Debug)]
pub struct Agent {
    pub id: AgentId,
    pub name: String,
    pub provider: String,
    pub status: AgentStatus,
    pub display_mode: DisplayMode,
    pub session_id: Option<String>,
    pub child_pid: Option<u32>,
    /// Name of the system prompt applied to this agent (from Prompts page).
    pub system_prompt_name: Option<String>,
    /// Timestamp of last PTY activity, for debouncing the Working status.
    pub last_active_at: Option<std::time::Instant>,
}

impl Agent {
    pub fn new(id: AgentId, name: String, provider: String) -> Self {
        Self {
            id,
            name,
            provider,
            status: AgentStatus::Idle,
            display_mode: DisplayMode::Native,
            session_id: None,
            child_pid: None,
            system_prompt_name: None,
            last_active_at: None,
        }
    }
}
