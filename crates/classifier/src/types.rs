use orchestrator_core::{ContentBlock, MessageContent, TraceId, Turn};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

// ---------------------------------------------------------------------------
// ClassificationId
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ClassificationId(pub Uuid);

impl ClassificationId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    pub fn from_u128(val: u128) -> Self {
        Self(Uuid::from_u128(val))
    }
}

impl Default for ClassificationId {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// TriggerPhase — when the classifier runs
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TriggerPhase {
    Incremental,
    OnComplete,
    Both,
}

// ---------------------------------------------------------------------------
// TurnFilter — which turns the classifier sees
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct TurnFilter {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tools: Vec<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_error: Option<bool>,
}

impl TurnFilter {
    pub fn has_filter(&self) -> bool {
        !self.tools.is_empty() || self.tool_error.is_some()
    }

    /// Check if a single turn matches all filter criteria.
    pub fn matches(&self, turn: &Turn) -> bool {
        let blocks = match &turn.content {
            MessageContent::Blocks(b) => b.as_slice(),
            MessageContent::Text(_) => {
                // Text-only turns have no tool blocks — they only match
                // if there are no tool-related filters.
                return self.tools.is_empty() && self.tool_error.is_none();
            }
        };

        if !self.tools.is_empty()
            && !blocks.iter().any(|b| {
                matches!(b, ContentBlock::ToolUse { name, .. } if self.tools.iter().any(|t| t == name))
            })
        {
            return false;
        }

        if self.tool_error == Some(true)
            && !blocks
                .iter()
                .any(|b| matches!(b, ContentBlock::ToolResult { is_error: true, .. }))
        {
            return false;
        }

        true
    }
}

// ---------------------------------------------------------------------------
// ClassifierTrigger — phase + filter
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClassifierTrigger {
    pub phase: TriggerPhase,
    pub filter: TurnFilter,
}

impl ClassifierTrigger {
    pub fn new(phase: TriggerPhase, filter: TurnFilter) -> Self {
        Self { phase, filter }
    }

    pub fn incremental() -> Self {
        Self::new(TriggerPhase::Incremental, TurnFilter::default())
    }

    pub fn on_complete() -> Self {
        Self::new(TriggerPhase::OnComplete, TurnFilter::default())
    }

    pub fn runs_incremental(&self) -> bool {
        matches!(self.phase, TriggerPhase::Incremental | TriggerPhase::Both)
    }

    pub fn runs_on_complete(&self) -> bool {
        matches!(self.phase, TriggerPhase::OnComplete | TriggerPhase::Both)
    }

    pub fn has_filter(&self) -> bool {
        self.filter.has_filter()
    }

    /// Filter turns, returning only those matching the trigger's criteria.
    pub fn filter_turns<'a>(&self, turns: &'a [Turn]) -> Vec<&'a Turn> {
        if !self.has_filter() {
            return turns.iter().collect();
        }
        turns.iter().filter(|t| self.filter.matches(t)).collect()
    }

    /// Human-readable summary for display.
    pub fn display_name(&self) -> String {
        let phase = match self.phase {
            TriggerPhase::Incremental => "Incremental",
            TriggerPhase::OnComplete => "OnComplete",
            TriggerPhase::Both => "Both",
        };
        if !self.has_filter() {
            return phase.to_string();
        }
        let mut parts = vec![phase.to_string()];
        if !self.filter.tools.is_empty() {
            parts.push(format!("tools={}", self.filter.tools.join(",")));
        }
        if self.filter.tool_error == Some(true) {
            parts.push("errors".to_string());
        }
        parts.join(", ")
    }
}

// ---------------------------------------------------------------------------
// Notification
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Notification {
    pub message: String,
}

impl Notification {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }

    pub fn format_xml(&self, classifier: &str, trace_id: &str) -> String {
        format!(
            "<aorch-notification classifier=\"{}\" trace=\"{}\">\n{}\n</aorch-notification>\n",
            classifier, trace_id, self.message,
        )
    }
}

// ---------------------------------------------------------------------------
// ClassificationResult
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClassificationResult {
    pub id: ClassificationId,
    pub trace_id: TraceId,
    pub classifier_name: String,
    pub timestamp: String,
    pub payload: serde_json::Value,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notification: Option<Notification>,
}

// ---------------------------------------------------------------------------
// ClassifierStatus
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct ClassifierStatus {
    pub name: String,
    pub trigger: ClassifierTrigger,
    pub enabled: bool,
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use orchestrator_core::{
        AgentId, MessageType, TurnBuilder, TurnMeta, TurnRole, TurnStatus, TurnStore, UserMeta,
    };
    use serde_json::json;

    fn make_text_turn() -> Turn {
        let mut store = TurnStore::new();
        let agent = AgentId::from_u128(1);
        store.insert(
            agent,
            TurnBuilder {
                uuid: "u1".into(),
                parent_uuid: None,
                session_id: None,
                timestamp: "2026-01-01T00:00:00Z".into(),
                message_type: MessageType::User,
                cwd: None,
                git_branch: None,
                role: TurnRole::User,
                content: MessageContent::Text("hello".into()),
                meta: TurnMeta::User(UserMeta {
                    is_meta: false,
                    is_compact_summary: false,
                    source_tool_assistant_uuid: None,
                }),
                status: TurnStatus::Complete,
                extra: json!({}),
            },
        );
        store.turns_for(agent)[0].clone()
    }

    fn make_tool_turn(tool_name: &str, is_error: bool) -> Turn {
        let mut store = TurnStore::new();
        let agent = AgentId::from_u128(1);
        let mut blocks = vec![ContentBlock::ToolUse {
            id: "t1".into(),
            name: tool_name.into(),
            input: json!({"cmd": "ls"}),
        }];
        if is_error {
            blocks.push(ContentBlock::ToolResult {
                tool_use_id: "t1".into(),
                content: orchestrator_core::ToolResultContent::Text("error".into()),
                is_error: true,
            });
        } else {
            blocks.push(ContentBlock::ToolResult {
                tool_use_id: "t1".into(),
                content: orchestrator_core::ToolResultContent::Text("ok".into()),
                is_error: false,
            });
        }
        store.insert(
            agent,
            TurnBuilder {
                uuid: "a1".into(),
                parent_uuid: None,
                session_id: None,
                timestamp: "2026-01-01T00:00:01Z".into(),
                message_type: MessageType::Assistant,
                cwd: None,
                git_branch: None,
                role: TurnRole::Assistant,
                content: MessageContent::Blocks(blocks),
                meta: TurnMeta::User(UserMeta {
                    is_meta: false,
                    is_compact_summary: false,
                    source_tool_assistant_uuid: None,
                }),
                status: TurnStatus::Complete,
                extra: json!({}),
            },
        );
        store.turns_for(agent)[0].clone()
    }

    #[test]
    fn phase_incremental() {
        let t = ClassifierTrigger::incremental();
        assert!(t.runs_incremental());
        assert!(!t.runs_on_complete());
    }

    #[test]
    fn phase_on_complete() {
        let t = ClassifierTrigger::on_complete();
        assert!(!t.runs_incremental());
        assert!(t.runs_on_complete());
    }

    #[test]
    fn phase_both() {
        let t = ClassifierTrigger::new(TriggerPhase::Both, TurnFilter::default());
        assert!(t.runs_incremental());
        assert!(t.runs_on_complete());
    }

    #[test]
    fn default_filter_matches_all() {
        let f = TurnFilter::default();
        assert!(!f.has_filter());
        assert!(f.matches(&make_text_turn()));
        assert!(f.matches(&make_tool_turn("Bash", false)));
    }

    #[test]
    fn tools_filter_matches_listed_tools() {
        let f = TurnFilter {
            tools: vec!["Bash".into()],
            ..Default::default()
        };
        assert!(f.has_filter());
        assert!(f.matches(&make_tool_turn("Bash", false)));
        assert!(!f.matches(&make_tool_turn("Edit", false)));
        assert!(!f.matches(&make_text_turn()));
    }

    #[test]
    fn tools_filter_multiple_tools() {
        let f = TurnFilter {
            tools: vec!["Bash".into(), "Edit".into()],
            ..Default::default()
        };
        assert!(f.matches(&make_tool_turn("Bash", false)));
        assert!(f.matches(&make_tool_turn("Edit", false)));
        assert!(!f.matches(&make_tool_turn("Read", false)));
    }

    #[test]
    fn tool_error_filter() {
        let f = TurnFilter {
            tool_error: Some(true),
            ..Default::default()
        };
        assert!(f.matches(&make_tool_turn("Bash", true)));
        assert!(!f.matches(&make_tool_turn("Bash", false)));
        assert!(!f.matches(&make_text_turn()));
    }

    #[test]
    fn combined_filters_and_logic() {
        let f = TurnFilter {
            tools: vec!["Bash".into()],
            tool_error: Some(true),
        };
        // Bash + error → match
        assert!(f.matches(&make_tool_turn("Bash", true)));
        // Bash + no error → no match (tool_error filter fails)
        assert!(!f.matches(&make_tool_turn("Bash", false)));
        // Edit + error → no match (tools filter fails)
        assert!(!f.matches(&make_tool_turn("Edit", true)));
    }

    #[test]
    fn filter_turns_returns_matching_only() {
        let trigger = ClassifierTrigger::new(
            TriggerPhase::Incremental,
            TurnFilter {
                tools: vec!["Bash".into()],
                ..Default::default()
            },
        );
        let turns = vec![
            make_text_turn(),
            make_tool_turn("Bash", false),
            make_tool_turn("Edit", false),
            make_tool_turn("Bash", true),
        ];
        let filtered = trigger.filter_turns(&turns);
        assert_eq!(filtered.len(), 2);
    }

    #[test]
    fn filter_turns_no_filter_returns_all() {
        let trigger = ClassifierTrigger::incremental();
        let turns = vec![make_text_turn(), make_tool_turn("Bash", false)];
        let filtered = trigger.filter_turns(&turns);
        assert_eq!(filtered.len(), 2);
    }

    #[test]
    fn display_name_simple() {
        assert_eq!(
            ClassifierTrigger::incremental().display_name(),
            "Incremental"
        );
        assert_eq!(
            ClassifierTrigger::on_complete().display_name(),
            "OnComplete"
        );
    }

    #[test]
    fn display_name_with_filter() {
        let t = ClassifierTrigger::new(
            TriggerPhase::Incremental,
            TurnFilter {
                tools: vec!["Bash".into(), "Edit".into()],
                tool_error: Some(true),
            },
        );
        let name = t.display_name();
        assert!(name.contains("Incremental"));
        assert!(name.contains("tools=Bash,Edit"));
        assert!(name.contains("errors"));
    }

    #[test]
    fn classification_id_unique() {
        assert_ne!(ClassificationId::new(), ClassificationId::new());
    }

    #[test]
    fn classification_id_from_u128_roundtrip() {
        let id = ClassificationId::from_u128(42);
        assert_eq!(id.0, Uuid::from_u128(42));
    }

    #[test]
    fn result_serializes_and_deserializes() {
        let result = ClassificationResult {
            id: ClassificationId::from_u128(1),
            trace_id: TraceId::from_u128(2),
            classifier_name: "test".into(),
            timestamp: "2026-01-01T00:00:00Z".into(),
            payload: json!({"label": "looping"}),
            notification: None,
        };
        let json_str = serde_json::to_string(&result).unwrap();
        let parsed: ClassificationResult = serde_json::from_str(&json_str).unwrap();
        assert_eq!(parsed.id, result.id);
        assert_eq!(parsed.payload["label"], "looping");
    }

    #[test]
    fn result_with_notification_roundtrips() {
        let result = ClassificationResult {
            id: ClassificationId::from_u128(1),
            trace_id: TraceId::from_u128(2),
            classifier_name: "budget".into(),
            timestamp: "2026-01-01T00:00:00Z".into(),
            payload: json!({}),
            notification: Some(Notification::new("You're over budget.")),
        };
        let json_str = serde_json::to_string(&result).unwrap();
        let parsed: ClassificationResult = serde_json::from_str(&json_str).unwrap();
        assert_eq!(parsed.notification.unwrap().message, "You're over budget.");
    }

    #[test]
    fn notification_format_xml() {
        let n = Notification::new("Stop looping.");
        let xml = n.format_xml("loop-detector", "abc123");
        assert!(xml.contains("classifier=\"loop-detector\""));
        assert!(xml.contains("trace=\"abc123\""));
        assert!(xml.contains("Stop looping."));
    }
}
