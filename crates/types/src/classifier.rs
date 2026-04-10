use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{ContentBlock, MessageContent, TraceId, Turn};

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
