use anyhow::Result;
use linfa::prelude::*;
use ndarray::Array2;
use orchestrator_classifier::runtime::{ClassifierRuntime, RuntimePrediction};
use orchestrator_core::{Trace, Turn};
use serde::Deserialize;

use crate::features::extract_features;
use crate::model::{DecisionTreeClassifier, SavedModel};

/// Runtime config parsed from the JSON `runtime` field in a classifier config.
#[derive(Debug, Deserialize)]
pub struct MlRuntimeConfig {
    pub model: String,
}

/// ML runtime backed by a trained decision tree.
pub struct MlRuntime {
    classifier: DecisionTreeClassifier,
}

impl MlRuntime {
    pub fn new(classifier: DecisionTreeClassifier) -> Self {
        Self { classifier }
    }

    /// Create from a saved model loaded from the database.
    pub fn from_saved_model(model: &SavedModel) -> Result<Self> {
        let classifier = DecisionTreeClassifier::from_saved_model(model)?;
        Ok(Self::new(classifier))
    }
}

impl ClassifierRuntime for MlRuntime {
    fn runtime_type(&self) -> &str {
        "ml"
    }

    fn classify(&self, trace: &Trace, turns: &[Turn]) -> Vec<RuntimePrediction> {
        let features = extract_features(trace, turns);
        let n_features = features.len();

        let Ok(input) = Array2::from_shape_vec((1, n_features), features) else {
            return vec![];
        };

        let dataset = linfa::DatasetBase::from(input);
        let predictions = self.classifier.tree().predict(&dataset);
        let predicted_idx = predictions[0];
        let label = self.classifier.label_for_index(predicted_idx);

        vec![RuntimePrediction {
            label,
            metadata: serde_json::json!({ "predicted_index": predicted_idx }),
        }]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::training::{train_decision_tree, TrainParams};
    use orchestrator_core::{
        AgentId, AssistantMeta, MessageContent, MessageType, StopReason, TokenUsage, Trace,
        TraceId, TraceStatus, TurnBuilder, TurnMeta, TurnRole, TurnStatus, TurnStore, UserMeta,
    };
    use serde_json::json;

    fn make_trace(input_tokens: u64, output_tokens: u64, turn_count: u32) -> Trace {
        Trace {
            id: TraceId::from_u128(1),
            agent_id: AgentId::from_u128(1),
            session_id: "s1".into(),
            status: TraceStatus::Complete,
            started_at: "2026-01-01T00:00:00Z".into(),
            completed_at: Some("2026-01-01T00:05:00Z".into()),
            turn_count,
            token_usage: TokenUsage {
                input_tokens,
                output_tokens,
                cache_creation_input_tokens: None,
                cache_read_input_tokens: None,
            },
            provider: "claude".into(),
            model: Some("claude-opus-4-6".into()),
        }
    }

    fn make_turns(agent_id: AgentId, count: usize) -> Vec<Turn> {
        let mut store = TurnStore::new();
        for i in 0..count {
            if i % 2 == 0 {
                store.insert(
                    agent_id,
                    TurnBuilder {
                        uuid: format!("u-{}", i),
                        parent_uuid: None,
                        session_id: Some("s1".into()),
                        timestamp: format!("2026-01-01T00:0{}:00Z", i),
                        message_type: MessageType::User,
                        cwd: None,
                        git_branch: None,
                        role: TurnRole::User,
                        content: MessageContent::Text("do something".into()),
                        meta: TurnMeta::User(UserMeta {
                            is_meta: false,
                            is_compact_summary: false,
                            source_tool_assistant_uuid: None,
                        }),
                        status: TurnStatus::Complete,
                        extra: json!({}),
                    },
                );
            } else {
                store.insert(
                    agent_id,
                    TurnBuilder {
                        uuid: format!("a-{}", i),
                        parent_uuid: Some(format!("u-{}", i - 1)),
                        session_id: Some("s1".into()),
                        timestamp: format!("2026-01-01T00:0{}:01Z", i),
                        message_type: MessageType::Assistant,
                        cwd: None,
                        git_branch: None,
                        role: TurnRole::Assistant,
                        content: MessageContent::Text("done".into()),
                        meta: TurnMeta::Assistant(AssistantMeta {
                            model: Some("claude-opus-4-6".into()),
                            stop_reason: Some(StopReason::EndTurn),
                            stop_sequence: None,
                            usage: Some(TokenUsage {
                                input_tokens: 100,
                                output_tokens: 50,
                                cache_creation_input_tokens: None,
                                cache_read_input_tokens: None,
                            }),
                            request_id: None,
                        }),
                        status: TurnStatus::Complete,
                        extra: json!({}),
                    },
                );
            }
        }
        store.turns_for(agent_id).into_iter().cloned().collect()
    }

    #[test]
    fn ml_runtime_end_to_end() {
        let agent = AgentId::from_u128(1);

        // Create labeled training data: "healthy" (low tokens) vs "excessive" (high tokens)
        let mut data = Vec::new();
        for i in 0..20 {
            let (tokens, label) = if i < 10 {
                (500u64, "healthy")
            } else {
                (50_000u64, "excessive")
            };
            let trace = make_trace(tokens, tokens / 2, 4);
            let turns = make_turns(agent, 4);
            data.push((trace, turns, label.to_string()));
        }

        // Train
        let result = train_decision_tree(&data, "test-classifier", 1, &TrainParams::default())
            .expect("training should succeed");

        // Build runtime from saved model
        let runtime =
            MlRuntime::from_saved_model(&result.model).expect("should load from saved model");
        assert_eq!(runtime.runtime_type(), "ml");

        // Classify a low-token trace → should predict "healthy"
        let trace = make_trace(500, 250, 4);
        let turns = make_turns(agent, 4);
        let predictions = runtime.classify(&trace, &turns);
        assert_eq!(predictions.len(), 1);
        // The label should be one of the training labels
        assert!(
            predictions[0].label == "healthy" || predictions[0].label == "excessive",
            "unexpected label: {}",
            predictions[0].label
        );
    }
}
