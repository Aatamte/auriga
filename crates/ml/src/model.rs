use anyhow::Result;
use linfa::prelude::*;
use linfa_trees::DecisionTree;
use ndarray::Array2;
use auriga_classifier::{
    ClassificationId, ClassificationResult, Classifier, ClassifierTrigger,
};
use auriga_core::{Trace, Turn};
use serde::{Deserialize, Serialize};

use crate::features::extract_features;

/// A trained model persisted to/from SQLite.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SavedModel {
    pub id: String,
    pub classifier_name: String,
    pub version: i64,
    pub created_at: String,
    pub feature_names: Vec<String>,
    pub model_data: String,
    pub accuracy: Option<f64>,
}

/// Serialized representation of a decision tree + label mapping.
#[derive(Serialize, Deserialize)]
struct SerializedTree {
    tree: DecisionTree<f64, usize>,
    label_map: Vec<String>,
}

/// A classifier backed by a trained decision tree.
pub struct DecisionTreeClassifier {
    name: String,
    tree: DecisionTree<f64, usize>,
    /// Maps predicted usize index back to label string.
    label_map: Vec<String>,
}

impl DecisionTreeClassifier {
    /// Construct from a saved model loaded from the database.
    pub fn from_saved_model(model: &SavedModel) -> Result<Self> {
        let serialized: SerializedTree = serde_json::from_str(&model.model_data)?;
        Ok(Self {
            name: model.classifier_name.clone(),
            tree: serialized.tree,
            label_map: serialized.label_map,
        })
    }

    pub fn tree(&self) -> &DecisionTree<f64, usize> {
        &self.tree
    }

    pub fn label_for_index(&self, idx: usize) -> String {
        self.label_map
            .get(idx)
            .cloned()
            .unwrap_or_else(|| format!("class_{}", idx))
    }

    /// Serialize the tree + label map to a JSON string for storage.
    pub fn serialize(tree: &DecisionTree<f64, usize>, label_map: &[String]) -> Result<String> {
        let s = SerializedTree {
            tree: tree.clone(),
            label_map: label_map.to_vec(),
        };
        Ok(serde_json::to_string(&s)?)
    }
}

impl Classifier for DecisionTreeClassifier {
    fn name(&self) -> &str {
        &self.name
    }

    fn trigger(&self) -> ClassifierTrigger {
        ClassifierTrigger::on_complete()
    }

    fn classify(&self, trace: &Trace, turns: &[Turn]) -> Vec<ClassificationResult> {
        let features = extract_features(trace, turns);
        let n_features = features.len();

        let Ok(input) = Array2::from_shape_vec((1, n_features), features) else {
            return vec![];
        };

        let dataset = linfa::DatasetBase::from(input);
        let predictions = self.tree.predict(&dataset);

        let predicted_idx = predictions[0];
        let predicted_label = self
            .label_map
            .get(predicted_idx)
            .cloned()
            .unwrap_or_else(|| format!("class_{}", predicted_idx));

        vec![ClassificationResult {
            id: ClassificationId::new(),
            trace_id: trace.id,
            classifier_name: self.name.clone(),
            timestamp: trace.completed_at.clone().unwrap_or_default(),
            payload: serde_json::json!({
                "predicted_label": predicted_label,
                "predicted_index": predicted_idx,
            }),
            notification: None,
        }]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn saved_model_serializes() {
        let model = SavedModel {
            id: "m1".into(),
            classifier_name: "test".into(),
            version: 1,
            created_at: "2026-01-01T00:00:00Z".into(),
            feature_names: vec!["turn_count".into(), "tokens".into()],
            model_data: "{}".into(),
            accuracy: Some(0.95),
        };
        let json = serde_json::to_string(&model).unwrap();
        let parsed: SavedModel = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.classifier_name, "test");
        assert_eq!(parsed.accuracy, Some(0.95));
    }
}
