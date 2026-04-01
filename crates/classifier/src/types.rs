use serde::{Deserialize, Serialize};
use uuid::Uuid;

use orchestrator_core::TraceId;

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
// ClassifierTrigger
// ---------------------------------------------------------------------------

/// When a classifier should be invoked.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ClassifierTrigger {
    /// Run on each new turn arrival.
    Incremental,
    /// Run once when the trace completes.
    OnComplete,
    /// Run on each new turn AND on completion.
    Both,
}

impl ClassifierTrigger {
    pub fn runs_incremental(&self) -> bool {
        matches!(self, Self::Incremental | Self::Both)
    }

    pub fn runs_on_complete(&self) -> bool {
        matches!(self, Self::OnComplete | Self::Both)
    }
}

// ---------------------------------------------------------------------------
// ClassificationResult
// ---------------------------------------------------------------------------

/// A single classification detection produced by a classifier.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClassificationResult {
    pub id: ClassificationId,
    pub trace_id: TraceId,
    pub classifier_name: String,
    pub timestamp: String,
    pub payload: serde_json::Value,
}

// ---------------------------------------------------------------------------
// ClassifierStatus
// ---------------------------------------------------------------------------

/// Summary of a registered classifier's current state.
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

    #[test]
    fn classification_id_unique() {
        let a = ClassificationId::new();
        let b = ClassificationId::new();
        assert_ne!(a, b);
    }

    #[test]
    fn classification_id_from_u128_roundtrip() {
        let id = ClassificationId::from_u128(42);
        assert_eq!(id.0, Uuid::from_u128(42));
    }

    #[test]
    fn trigger_incremental_runs_incremental() {
        assert!(ClassifierTrigger::Incremental.runs_incremental());
        assert!(!ClassifierTrigger::Incremental.runs_on_complete());
    }

    #[test]
    fn trigger_on_complete_runs_on_complete() {
        assert!(!ClassifierTrigger::OnComplete.runs_incremental());
        assert!(ClassifierTrigger::OnComplete.runs_on_complete());
    }

    #[test]
    fn trigger_both_runs_both() {
        assert!(ClassifierTrigger::Both.runs_incremental());
        assert!(ClassifierTrigger::Both.runs_on_complete());
    }

    #[test]
    fn result_serializes_and_deserializes() {
        let result = ClassificationResult {
            id: ClassificationId::from_u128(1),
            trace_id: TraceId::from_u128(2),
            classifier_name: "test".into(),
            timestamp: "2026-01-01T00:00:00Z".into(),
            payload: serde_json::json!({"label": "looping", "confidence": 0.9}),
        };
        let json = serde_json::to_string(&result).unwrap();
        let parsed: ClassificationResult = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.id, result.id);
        assert_eq!(parsed.classifier_name, "test");
        assert_eq!(parsed.payload["label"], "looping");
    }
}
