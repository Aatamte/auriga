use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::AgentId;

// ---------------------------------------------------------------------------
// SkillId
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SkillId(pub Uuid);

impl SkillId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    pub fn from_u128(val: u128) -> Self {
        Self(Uuid::from_u128(val))
    }
}

impl Default for SkillId {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// SkillTrigger
// ---------------------------------------------------------------------------

/// When a skill can be invoked.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SkillTrigger {
    /// Manually invoked by an agent or user.
    OnDemand,
    /// Automatically invoked when an agent session starts.
    OnSessionStart,
    /// Automatically invoked when an agent session ends.
    OnSessionEnd,
}

impl SkillTrigger {
    pub fn is_on_demand(&self) -> bool {
        matches!(self, Self::OnDemand)
    }

    pub fn runs_on_session_start(&self) -> bool {
        matches!(self, Self::OnSessionStart)
    }

    pub fn runs_on_session_end(&self) -> bool {
        matches!(self, Self::OnSessionEnd)
    }
}

// ---------------------------------------------------------------------------
// SkillContext
// ---------------------------------------------------------------------------

/// Input provided to a skill when executed.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillContext {
    pub agent_id: AgentId,
    pub arguments: serde_json::Value,
}

// ---------------------------------------------------------------------------
// SkillResult
// ---------------------------------------------------------------------------

/// Output of a skill execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillResult {
    pub id: SkillId,
    pub skill_name: String,
    pub agent_id: AgentId,
    pub timestamp: String,
    pub success: bool,
    pub payload: serde_json::Value,
}

// ---------------------------------------------------------------------------
// SkillStatus
// ---------------------------------------------------------------------------

/// Summary of a registered skill for UI display.
#[derive(Debug, Clone)]
pub struct SkillStatus {
    pub name: String,
    pub description: String,
    pub trigger: SkillTrigger,
    pub enabled: bool,
}
