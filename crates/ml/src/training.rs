use anyhow::{bail, Result};
use linfa::prelude::*;
use linfa_trees::DecisionTree;
use ndarray::{Array1, Array2};
use auriga_core::{Trace, Turn};
use std::collections::HashMap;
use uuid::Uuid;

use crate::features::{extract_features, feature_count, FEATURE_NAMES};
use crate::model::{DecisionTreeClassifier, SavedModel};

/// Parameters for decision tree training.
pub struct TrainParams {
    /// Maximum tree depth. None = unlimited.
    pub max_depth: Option<usize>,
    /// Minimum number of samples required to split a node.
    pub min_weight_split: f32,
    /// Fraction of data held out for testing (0.0–1.0).
    pub test_ratio: f64,
}

impl Default for TrainParams {
    fn default() -> Self {
        Self {
            max_depth: Some(10),
            min_weight_split: 2.0_f32,
            test_ratio: 0.2,
        }
    }
}

/// Result of a training run.
pub struct TrainResult {
    pub model: SavedModel,
    pub accuracy: f64,
    pub n_train: usize,
    pub n_test: usize,
    pub label_distribution: HashMap<String, usize>,
}

/// Train a decision tree on labeled trace data.
///
/// Each entry is (trace, turns, label_string).
/// Returns a `TrainResult` with a `SavedModel` ready for persistence.
pub fn train_decision_tree(
    data: &[(Trace, Vec<Turn>, String)],
    classifier_name: &str,
    version: i64,
    params: &TrainParams,
) -> Result<TrainResult> {
    if data.is_empty() {
        bail!("no training data provided");
    }

    let n_features = feature_count();
    let n_samples = data.len();

    // Extract features
    let mut features_flat: Vec<f64> = Vec::with_capacity(n_samples * n_features);
    let mut label_strings: Vec<String> = Vec::with_capacity(n_samples);

    for (trace, turns, label) in data {
        let feats = extract_features(trace, turns);
        assert_eq!(feats.len(), n_features);
        features_flat.extend_from_slice(&feats);
        label_strings.push(label.clone());
    }

    // Encode labels: string → usize
    let mut label_map: Vec<String> = Vec::new();
    let mut label_indices: HashMap<String, usize> = HashMap::new();
    let mut encoded_labels: Vec<usize> = Vec::with_capacity(n_samples);

    for label in &label_strings {
        let idx = if let Some(&idx) = label_indices.get(label) {
            idx
        } else {
            let idx = label_map.len();
            label_map.push(label.clone());
            label_indices.insert(label.clone(), idx);
            idx
        };
        encoded_labels.push(idx);
    }

    let features = Array2::from_shape_vec((n_samples, n_features), features_flat)?;
    let targets = Array1::from_vec(encoded_labels);

    let dataset = DatasetBase::new(features, targets);

    // Split train/test
    let (train, test) = if params.test_ratio > 0.0 && n_samples > 1 {
        let n_test = (n_samples as f64 * params.test_ratio).max(1.0) as usize;
        let n_train = n_samples - n_test;
        let (train_set, test_set) = dataset.split_with_ratio(n_train as f32 / n_samples as f32);
        (train_set, Some(test_set))
    } else {
        (dataset, None)
    };

    let n_train = train.nsamples();

    // Build and fit decision tree
    let mut tree_params = DecisionTree::params();
    if let Some(depth) = params.max_depth {
        tree_params = tree_params.max_depth(Some(depth));
    }
    tree_params = tree_params.min_weight_split(params.min_weight_split);

    let tree = tree_params.fit(&train)?;

    // Evaluate on test set
    let (accuracy, n_test) = if let Some(ref test_set) = test {
        let predictions = tree.predict(test_set);
        let correct = predictions
            .iter()
            .zip(test_set.as_targets().iter())
            .filter(|(p, t)| p == t)
            .count();
        let n = test_set.nsamples();
        (correct as f64 / n as f64, n)
    } else {
        (1.0, 0)
    };

    // Serialize
    let model_data = DecisionTreeClassifier::serialize(&tree, &label_map)?;

    let model = SavedModel {
        id: Uuid::new_v4().to_string(),
        classifier_name: classifier_name.to_string(),
        version,
        created_at: now_iso8601(),
        feature_names: FEATURE_NAMES.iter().map(|s| s.to_string()).collect(),
        model_data,
        accuracy: Some(accuracy),
    };

    let mut label_distribution = HashMap::new();
    for label in &label_strings {
        *label_distribution.entry(label.clone()).or_insert(0) += 1;
    }

    Ok(TrainResult {
        model,
        accuracy,
        n_train,
        n_test,
        label_distribution,
    })
}

fn now_iso8601() -> String {
    use std::time::SystemTime;
    let d = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default();
    let secs = d.as_secs();
    let days = secs / 86400;
    let years = 1970 + days / 365;
    let rem_days = days % 365;
    let months = rem_days / 30 + 1;
    let day = rem_days % 30 + 1;
    let rem_secs = secs % 86400;
    let hours = rem_secs / 3600;
    let minutes = (rem_secs % 3600) / 60;
    let seconds = rem_secs % 60;
    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
        years, months, day, hours, minutes, seconds
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use auriga_classifier::Classifier;
    use auriga_core::*;

    fn make_sample(turn_count: u32, tokens: u64, label: &str) -> (Trace, Vec<Turn>, String) {
        let trace = Trace {
            id: TraceId::new(),
            agent_id: AgentId::from_u128(1),
            session_id: "s1".into(),
            status: TraceStatus::Complete,
            started_at: "2026-01-01T00:00:00Z".into(),
            completed_at: Some("2026-01-01T00:05:00Z".into()),
            turn_count,
            token_usage: TokenUsage {
                input_tokens: tokens,
                output_tokens: tokens / 2,
                cache_creation_input_tokens: None,
                cache_read_input_tokens: None,
            },
            provider: "claude".into(),
            model: None,
        };
        (trace, vec![], label.to_string())
    }

    #[test]
    fn train_on_small_dataset() {
        let data: Vec<_> = (0..20)
            .map(|i| {
                if i < 10 {
                    make_sample(5, 1000, "success")
                } else {
                    make_sample(50, 50000, "failure")
                }
            })
            .collect();

        let result = train_decision_tree(&data, "test-classifier", 1, &TrainParams::default());
        assert!(result.is_ok());

        let result = result.unwrap();
        assert!(result.accuracy >= 0.0 && result.accuracy <= 1.0);
        assert_eq!(result.model.classifier_name, "test-classifier");
        assert_eq!(result.model.version, 1);
        assert!(!result.model.model_data.is_empty());
    }

    #[test]
    fn train_empty_data_fails() {
        let result = train_decision_tree(&[], "test", 1, &TrainParams::default());
        assert!(result.is_err());
    }

    #[test]
    fn model_round_trips_through_serialization() {
        let data: Vec<_> = (0..10)
            .map(|i| {
                if i < 5 {
                    make_sample(5, 1000, "a")
                } else {
                    make_sample(50, 50000, "b")
                }
            })
            .collect();

        let params = TrainParams {
            test_ratio: 0.0,
            ..Default::default()
        };
        let result = train_decision_tree(&data, "round-trip", 1, &params).unwrap();

        // Deserialize back into a classifier
        let classifier = DecisionTreeClassifier::from_saved_model(&result.model);
        assert!(classifier.is_ok());

        let classifier = classifier.unwrap();
        assert_eq!(classifier.name(), "round-trip");
    }

    #[test]
    fn label_distribution_tracked() {
        let data = vec![
            make_sample(5, 1000, "a"),
            make_sample(5, 1000, "a"),
            make_sample(50, 50000, "b"),
        ];

        let params = TrainParams {
            test_ratio: 0.0,
            ..Default::default()
        };
        let result = train_decision_tree(&data, "test", 1, &params).unwrap();
        assert_eq!(result.label_distribution["a"], 2);
        assert_eq!(result.label_distribution["b"], 1);
    }
}
