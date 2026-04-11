use std::path::{Path, PathBuf};

use crate::runtime::ClassifierRuntime;
use crate::Classifier;
use auriga_types::{
    ClassificationId, ClassificationResult, ClassifierConfig, ClassifierTrigger, Notification,
    Trace, Turn,
};

// ---------------------------------------------------------------------------
// Loading configs from a directory
// ---------------------------------------------------------------------------

/// Load all classifier configs from a directory of .json files.
pub fn load_configs(dir: &Path) -> Vec<(PathBuf, ClassifierConfig)> {
    let mut configs = Vec::new();

    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return configs,
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }
        match load_config(&path) {
            Ok(config) => configs.push((path, config)),
            Err(e) => {
                tracing::warn!(path = %path.display(), error = %e, "failed to load classifier config");
            }
        }
    }

    configs
}

/// Load a single classifier config from a .json file.
pub fn load_config(path: &Path) -> anyhow::Result<ClassifierConfig> {
    let contents = std::fs::read_to_string(path)?;
    let config: ClassifierConfig = serde_json::from_str(&contents)?;
    Ok(config)
}

/// Save a classifier config to a .json file.
pub fn save_config(path: &Path, config: &ClassifierConfig) -> anyhow::Result<()> {
    let json = serde_json::to_string_pretty(config)?;
    std::fs::write(path, json)?;
    Ok(())
}

// ---------------------------------------------------------------------------
// ConfigClassifier — wraps a config + runtime, implements the Classifier trait
// ---------------------------------------------------------------------------

/// A classifier driven by a JSON config and a pluggable runtime.
///
/// Without a runtime attached, `classify()` returns empty results.
/// The app layer resolves the runtime based on the config's `type` field
/// and attaches it via `with_runtime()`.
pub struct ConfigClassifier {
    config: ClassifierConfig,
    runtime: Option<Box<dyn ClassifierRuntime>>,
}

impl ConfigClassifier {
    pub fn new(config: ClassifierConfig) -> Self {
        Self {
            config,
            runtime: None,
        }
    }

    /// Attach a runtime. Called by the app layer after construction.
    pub fn with_runtime(mut self, runtime: Box<dyn ClassifierRuntime>) -> Self {
        self.runtime = Some(runtime);
        self
    }

    pub fn config(&self) -> &ClassifierConfig {
        &self.config
    }

    /// Look up a label config by name, returning its notification if found.
    pub fn label_notification(&self, label: &str) -> Option<Notification> {
        self.config
            .labels
            .iter()
            .find(|l| l.label == label)
            .map(|l| Notification::new(&l.notification.message))
    }
}

impl Classifier for ConfigClassifier {
    fn name(&self) -> &str {
        &self.config.name
    }

    fn trigger(&self) -> ClassifierTrigger {
        self.config.trigger.clone().into()
    }

    fn config(&self) -> Option<&ClassifierConfig> {
        Some(&self.config)
    }

    fn classify(&self, trace: &Trace, turns: &[Turn]) -> Vec<ClassificationResult> {
        let Some(runtime) = &self.runtime else {
            return Vec::new();
        };

        let predictions = runtime.classify(trace, turns);

        predictions
            .into_iter()
            .map(|pred| {
                let notification = self.label_notification(&pred.label);
                ClassificationResult {
                    id: ClassificationId::new(),
                    trace_id: trace.id,
                    classifier_name: self.config.name.clone(),
                    timestamp: trace
                        .completed_at
                        .clone()
                        .unwrap_or_else(|| trace.started_at.clone()),
                    payload: serde_json::json!({
                        "predicted_label": pred.label,
                        "metadata": pred.metadata,
                    }),
                    notification,
                }
            })
            .collect()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::RuntimePrediction;
    use auriga_types::{
        AgentId, ClassifierType, ConfigTrigger, LabelConfig, NotificationConfig, TokenUsage,
        TraceId, TraceStatus, TriggerConfig, TriggerPhase,
    };
    use std::io::Write;
    use tempfile::TempDir;

    fn sample_config() -> ClassifierConfig {
        ClassifierConfig {
            name: "trace-health".into(),
            description: "Evaluates overall trace health patterns".into(),
            version: "1.0".into(),
            enabled: true,
            trigger: ConfigTrigger::Simple(TriggerPhase::OnComplete),
            classifier_type: ClassifierType::Ml,
            runtime: serde_json::json!({"model": "trace-health-v1"}),
            labels: vec![
                LabelConfig {
                    label: "looping".into(),
                    notification: NotificationConfig {
                        message: "Agent is repeating the same actions".into(),
                    },
                },
                LabelConfig {
                    label: "excessive-tokens".into(),
                    notification: NotificationConfig {
                        message: "Token usage exceeded expected threshold".into(),
                    },
                },
                LabelConfig {
                    label: "healthy".into(),
                    notification: NotificationConfig {
                        message: "Trace completed normally".into(),
                    },
                },
            ],
        }
    }

    fn test_trace() -> Trace {
        Trace {
            id: TraceId::from_u128(1),
            agent_id: AgentId::from_u128(1),
            session_id: "s1".into(),
            status: TraceStatus::Complete,
            started_at: "2026-01-01T00:00:00Z".into(),
            completed_at: Some("2026-01-01T00:05:00Z".into()),
            turn_count: 5,
            token_usage: TokenUsage {
                input_tokens: 1000,
                output_tokens: 500,
                cache_creation_input_tokens: None,
                cache_read_input_tokens: None,
            },
            provider: "claude".into(),
            model: None,
        }
    }

    /// Mock runtime that returns fixed predictions for testing.
    struct MockRuntime {
        predictions: Vec<RuntimePrediction>,
    }

    impl ClassifierRuntime for MockRuntime {
        fn runtime_type(&self) -> &str {
            "mock"
        }

        fn classify(&self, _trace: &Trace, _turns: &[Turn]) -> Vec<RuntimePrediction> {
            self.predictions
                .iter()
                .map(|p| RuntimePrediction {
                    label: p.label.clone(),
                    metadata: p.metadata.clone(),
                })
                .collect()
        }
    }

    #[test]
    fn config_with_type_serializes() {
        let config = sample_config();
        let json = serde_json::to_string_pretty(&config).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["type"], "ml");
        assert_eq!(parsed["runtime"]["model"], "trace-health-v1");
        assert_eq!(parsed["labels"].as_array().unwrap().len(), 3);
    }

    #[test]
    fn config_with_type_round_trips() {
        let config = sample_config();
        let json = serde_json::to_string(&config).unwrap();
        let parsed: ClassifierConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.classifier_type, ClassifierType::Ml);
        assert_eq!(parsed.runtime["model"], "trace-health-v1");
        assert_eq!(parsed.labels.len(), 3);
    }

    #[test]
    fn config_without_type_defaults_to_ml() {
        let json = r#"{
            "name": "old-config",
            "description": "Legacy config without type",
            "version": "1.0",
            "enabled": true,
            "trigger": "on_complete",
            "labels": []
        }"#;
        let config: ClassifierConfig = serde_json::from_str(json).unwrap();
        assert_eq!(config.classifier_type, ClassifierType::Ml);
        assert!(config.runtime.is_null());
    }

    #[test]
    fn simple_trigger_converts() {
        let t = ClassifierTrigger::from(ConfigTrigger::Simple(TriggerPhase::OnComplete));
        assert_eq!(t.phase, TriggerPhase::OnComplete);
        assert!(!t.has_filter());
    }

    #[test]
    fn rich_trigger_converts() {
        let t = ClassifierTrigger::from(ConfigTrigger::Rich(TriggerConfig {
            on: TriggerPhase::Incremental,
            tools: vec!["Bash".into()],
            tool_error: Some(true),
        }));
        assert_eq!(t.phase, TriggerPhase::Incremental);
        assert_eq!(t.filter.tools, vec!["Bash"]);
        assert_eq!(t.filter.tool_error, Some(true));
    }

    #[test]
    fn string_trigger_deserializes() {
        let json = r#""on_complete""#;
        let t: ConfigTrigger = serde_json::from_str(json).unwrap();
        assert!(matches!(t, ConfigTrigger::Simple(TriggerPhase::OnComplete)));
    }

    #[test]
    fn object_trigger_deserializes() {
        let json = r#"{"on": "incremental", "tools": ["Bash", "Edit"]}"#;
        let t: ConfigTrigger = serde_json::from_str(json).unwrap();
        match t {
            ConfigTrigger::Rich(cfg) => {
                assert_eq!(cfg.on, TriggerPhase::Incremental);
                assert_eq!(cfg.tools, vec!["Bash", "Edit"]);
                assert!(cfg.tool_error.is_none());
            }
            _ => panic!("expected Rich"),
        }
    }

    #[test]
    fn object_trigger_no_filters_equivalent_to_simple() {
        let simple = ClassifierTrigger::from(ConfigTrigger::Simple(TriggerPhase::Both));
        let rich = ClassifierTrigger::from(ConfigTrigger::Rich(TriggerConfig {
            on: TriggerPhase::Both,
            tools: vec![],
            tool_error: None,
        }));
        assert_eq!(simple, rich);
    }

    #[test]
    fn save_and_load_config() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("test.json");
        let config = sample_config();
        save_config(&path, &config).unwrap();
        let loaded = load_config(&path).unwrap();
        assert_eq!(loaded.name, "trace-health");
        assert_eq!(loaded.classifier_type, ClassifierType::Ml);
        assert_eq!(loaded.labels.len(), 3);
    }

    #[test]
    fn load_configs_from_directory() {
        let dir = TempDir::new().unwrap();

        let c1 = sample_config();
        save_config(&dir.path().join("health.json"), &c1).unwrap();

        let mut c2 = sample_config();
        c2.name = "budget".into();
        save_config(&dir.path().join("budget.json"), &c2).unwrap();

        // Non-json file should be ignored
        let mut f = std::fs::File::create(dir.path().join("readme.txt")).unwrap();
        f.write_all(b"not a config").unwrap();

        let configs = load_configs(dir.path());
        assert_eq!(configs.len(), 2);
        let names: Vec<&str> = configs.iter().map(|(_, c)| c.name.as_str()).collect();
        assert!(names.contains(&"trace-health"));
        assert!(names.contains(&"budget"));
    }

    #[test]
    fn load_configs_empty_dir() {
        let dir = TempDir::new().unwrap();
        assert!(load_configs(dir.path()).is_empty());
    }

    #[test]
    fn load_configs_missing_dir() {
        assert!(load_configs(Path::new("/nonexistent/path")).is_empty());
    }

    #[test]
    fn classifier_without_runtime_returns_empty() {
        let classifier = ConfigClassifier::new(sample_config());
        assert_eq!(classifier.name(), "trace-health");
        assert_eq!(classifier.trigger(), ClassifierTrigger::on_complete());
        assert!(classifier.classify(&test_trace(), &[]).is_empty());
    }

    #[test]
    fn classifier_with_runtime_produces_results() {
        let runtime = MockRuntime {
            predictions: vec![RuntimePrediction {
                label: "looping".into(),
                metadata: serde_json::json!({"confidence": 0.95}),
            }],
        };
        let classifier = ConfigClassifier::new(sample_config()).with_runtime(Box::new(runtime));

        let results = classifier.classify(&test_trace(), &[]);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].classifier_name, "trace-health");
        assert_eq!(results[0].payload["predicted_label"], "looping");
        assert_eq!(results[0].payload["metadata"]["confidence"], 0.95);
        let notif = results[0].notification.as_ref().unwrap();
        assert_eq!(notif.message, "Agent is repeating the same actions");
    }

    #[test]
    fn classifier_with_multiple_predictions() {
        let runtime = MockRuntime {
            predictions: vec![
                RuntimePrediction {
                    label: "looping".into(),
                    metadata: serde_json::json!({}),
                },
                RuntimePrediction {
                    label: "excessive-tokens".into(),
                    metadata: serde_json::json!({}),
                },
            ],
        };
        let classifier = ConfigClassifier::new(sample_config()).with_runtime(Box::new(runtime));

        let results = classifier.classify(&test_trace(), &[]);
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].payload["predicted_label"], "looping");
        assert_eq!(
            results[0].notification.as_ref().unwrap().message,
            "Agent is repeating the same actions"
        );
        assert_eq!(results[1].payload["predicted_label"], "excessive-tokens");
        assert_eq!(
            results[1].notification.as_ref().unwrap().message,
            "Token usage exceeded expected threshold"
        );
    }

    #[test]
    fn prediction_with_unknown_label_has_no_notification() {
        let runtime = MockRuntime {
            predictions: vec![RuntimePrediction {
                label: "unknown-label".into(),
                metadata: serde_json::json!({}),
            }],
        };
        let classifier = ConfigClassifier::new(sample_config()).with_runtime(Box::new(runtime));

        let results = classifier.classify(&test_trace(), &[]);
        assert_eq!(results.len(), 1);
        assert!(results[0].notification.is_none());
    }

    #[test]
    fn label_notification_lookup() {
        let classifier = ConfigClassifier::new(sample_config());
        let notif = classifier.label_notification("looping").unwrap();
        assert_eq!(notif.message, "Agent is repeating the same actions");
        assert!(classifier.label_notification("unknown").is_none());
    }
}
