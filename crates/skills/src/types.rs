use serde::{Deserialize, Serialize};
use uuid::Uuid;

use orchestrator_core::AgentId;

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

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn skill_id_unique() {
        let a = SkillId::new();
        let b = SkillId::new();
        assert_ne!(a, b);
    }

    #[test]
    fn skill_id_from_u128_roundtrip() {
        let id = SkillId::from_u128(42);
        assert_eq!(id.0, Uuid::from_u128(42));
    }

    #[test]
    fn trigger_on_demand() {
        assert!(SkillTrigger::OnDemand.is_on_demand());
        assert!(!SkillTrigger::OnDemand.runs_on_session_start());
        assert!(!SkillTrigger::OnDemand.runs_on_session_end());
    }

    #[test]
    fn trigger_on_session_start() {
        assert!(!SkillTrigger::OnSessionStart.is_on_demand());
        assert!(SkillTrigger::OnSessionStart.runs_on_session_start());
        assert!(!SkillTrigger::OnSessionStart.runs_on_session_end());
    }

    #[test]
    fn trigger_on_session_end() {
        assert!(!SkillTrigger::OnSessionEnd.is_on_demand());
        assert!(!SkillTrigger::OnSessionEnd.runs_on_session_start());
        assert!(SkillTrigger::OnSessionEnd.runs_on_session_end());
    }

    #[test]
    fn result_serializes_and_deserializes() {
        let result = SkillResult {
            id: SkillId::from_u128(1),
            skill_name: "test-skill".into(),
            agent_id: AgentId::from_u128(2),
            timestamp: "2026-01-01T00:00:00Z".into(),
            success: true,
            payload: serde_json::json!({"output": "done"}),
        };
        let json = serde_json::to_string(&result).unwrap();
        let parsed: SkillResult = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.id, result.id);
        assert_eq!(parsed.skill_name, "test-skill");
        assert!(parsed.success);
        assert_eq!(parsed.payload["output"], "done");
    }
}
