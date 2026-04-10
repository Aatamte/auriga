use uuid::Uuid;

/// Unique identifier for a managed agent session.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SessionId(pub Uuid);

impl SessionId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    pub fn from_u128(val: u128) -> Self {
        Self(Uuid::from_u128(val))
    }
}

impl Default for SessionId {
    fn default() -> Self {
        Self::new()
    }
}

/// Lifecycle status of a managed session.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionStatus {
    /// Waiting for a user message to start a turn.
    Ready,
    /// A generation request is in flight.
    Generating,
    /// The model returned tool calls; waiting for tool results.
    ToolPending,
    /// The session has ended normally.
    Complete,
    /// The session was cancelled or errored.
    Aborted,
}
