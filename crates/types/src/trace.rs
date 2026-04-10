use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{AgentId, TokenUsage};

// ---------------------------------------------------------------------------
// IDs
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TraceId(pub Uuid);

impl TraceId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    pub fn from_u128(val: u128) -> Self {
        Self(Uuid::from_u128(val))
    }
}

impl Default for TraceId {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Enums
// ---------------------------------------------------------------------------

/// Trace lifecycle status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TraceStatus {
    Active,
    Complete,
    Aborted,
}

// ---------------------------------------------------------------------------
// Trace entity
// ---------------------------------------------------------------------------

/// A session-level grouping of turns for one agent conversation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Trace {
    pub id: TraceId,
    pub agent_id: AgentId,
    pub session_id: String,
    pub status: TraceStatus,
    pub started_at: String,
    pub completed_at: Option<String>,
    pub turn_count: u32,
    pub token_usage: TokenUsage,
    pub provider: String,
    pub model: Option<String>,
}
