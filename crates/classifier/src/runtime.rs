use auriga_core::{Trace, Turn};
use serde::{Deserialize, Serialize};
use std::io::Write;
use std::process::{Command, Stdio};

/// A raw prediction from a runtime — just a label and optional metadata.
/// The ConfigClassifier maps this to a full ClassificationResult with
/// notification lookup from the config's labels array.
pub struct RuntimePrediction {
    pub label: String,
    pub metadata: serde_json::Value,
}

/// A runtime that can classify traces.
/// Implementations live in separate crates (ml, llm, etc.) to keep
/// the classifier crate agnostic to runtime internals.
pub trait ClassifierRuntime: Send + Sync {
    /// The type key this runtime handles (e.g. "ml", "llm", "cli").
    fn runtime_type(&self) -> &str;

    /// Classify a trace and its turns, returning zero or more predicted labels.
    fn classify(&self, trace: &Trace, turns: &[Turn]) -> Vec<RuntimePrediction>;
}

/// Stub LLM runtime. Returns empty predictions until a real LLM
/// integration is implemented.
pub struct LlmRuntimeStub;

impl ClassifierRuntime for LlmRuntimeStub {
    fn runtime_type(&self) -> &str {
        "llm"
    }

    fn classify(&self, _trace: &Trace, _turns: &[Turn]) -> Vec<RuntimePrediction> {
        Vec::new()
    }
}

// ---------------------------------------------------------------------------
// CLI Runtime — invokes an external process via stdin/stdout
// ---------------------------------------------------------------------------

/// Config parsed from the `runtime` field when `type` is `cli`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CliRuntimeConfig {
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
}

/// The JSON payload written to the child process's stdin.
#[derive(Serialize)]
struct CliInput<'a> {
    trace: &'a Trace,
    turns: &'a [Turn],
    labels: Vec<&'a str>,
}

/// CLI runtime that spawns an external process, pipes trace data via stdin,
/// and reads predicted labels from stdout.
///
/// Protocol:
///   stdin:  `{ "trace": {...}, "turns": [...], "labels": [...] }`
///   stdout: `["label1", "label2"]`  (JSON array of predicted label strings)
pub struct CliRuntime {
    config: CliRuntimeConfig,
    label_names: Vec<String>,
}

impl CliRuntime {
    pub fn new(config: CliRuntimeConfig, label_names: Vec<String>) -> Self {
        Self {
            config,
            label_names,
        }
    }
}

impl ClassifierRuntime for CliRuntime {
    fn runtime_type(&self) -> &str {
        "cli"
    }

    fn classify(&self, trace: &Trace, turns: &[Turn]) -> Vec<RuntimePrediction> {
        let input = CliInput {
            trace,
            turns,
            labels: self.label_names.iter().map(|s| s.as_str()).collect(),
        };

        let input_json = match serde_json::to_string(&input) {
            Ok(j) => j,
            Err(e) => {
                tracing::warn!(error = %e, "failed to serialize CLI classifier input");
                return Vec::new();
            }
        };

        let mut child = match Command::new(&self.config.command)
            .args(&self.config.args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
        {
            Ok(c) => c,
            Err(e) => {
                tracing::warn!(
                    command = %self.config.command,
                    error = %e,
                    "failed to spawn CLI classifier"
                );
                return Vec::new();
            }
        };

        // Write input to stdin
        if let Some(mut stdin) = child.stdin.take() {
            if let Err(e) = stdin.write_all(input_json.as_bytes()) {
                tracing::warn!(error = %e, "failed to write to CLI classifier stdin");
                return Vec::new();
            }
            // stdin is dropped here, closing the pipe
        }

        // Read stdout
        let output = match child.wait_with_output() {
            Ok(o) => o,
            Err(e) => {
                tracing::warn!(error = %e, "failed to read CLI classifier output");
                return Vec::new();
            }
        };

        if !output.status.success() {
            tracing::warn!(
                command = %self.config.command,
                status = %output.status,
                "CLI classifier exited with error"
            );
            return Vec::new();
        }

        // Parse stdout as JSON array of label strings
        let predicted_labels: Vec<String> = match serde_json::from_slice(&output.stdout) {
            Ok(labels) => labels,
            Err(e) => {
                tracing::warn!(error = %e, "failed to parse CLI classifier output as JSON");
                return Vec::new();
            }
        };

        predicted_labels
            .into_iter()
            .map(|label| RuntimePrediction {
                label,
                metadata: serde_json::json!({}),
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use auriga_core::{AgentId, TokenUsage, TraceId, TraceStatus};

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

    #[test]
    fn llm_stub_returns_empty() {
        let stub = LlmRuntimeStub;
        assert_eq!(stub.runtime_type(), "llm");
        assert!(stub.classify(&test_trace(), &[]).is_empty());
    }

    #[test]
    fn cli_runtime_with_echo_script() {
        // Use a shell command that reads stdin and outputs a JSON array
        let config = CliRuntimeConfig {
            command: "sh".into(),
            args: vec!["-c".into(), r#"cat > /dev/null; echo '["healthy"]'"#.into()],
        };
        let runtime = CliRuntime::new(config, vec!["healthy".into(), "looping".into()]);
        assert_eq!(runtime.runtime_type(), "cli");

        let predictions = runtime.classify(&test_trace(), &[]);
        assert_eq!(predictions.len(), 1);
        assert_eq!(predictions[0].label, "healthy");
    }

    #[test]
    fn cli_runtime_multiple_labels() {
        let config = CliRuntimeConfig {
            command: "sh".into(),
            args: vec![
                "-c".into(),
                r#"cat > /dev/null; echo '["looping", "excessive-tokens"]'"#.into(),
            ],
        };
        let runtime = CliRuntime::new(config, vec!["looping".into(), "excessive-tokens".into()]);

        let predictions = runtime.classify(&test_trace(), &[]);
        assert_eq!(predictions.len(), 2);
        assert_eq!(predictions[0].label, "looping");
        assert_eq!(predictions[1].label, "excessive-tokens");
    }

    #[test]
    fn cli_runtime_bad_command_returns_empty() {
        let config = CliRuntimeConfig {
            command: "/nonexistent/binary".into(),
            args: vec![],
        };
        let runtime = CliRuntime::new(config, vec![]);

        let predictions = runtime.classify(&test_trace(), &[]);
        assert!(predictions.is_empty());
    }

    #[test]
    fn cli_runtime_bad_output_returns_empty() {
        let config = CliRuntimeConfig {
            command: "sh".into(),
            args: vec!["-c".into(), "echo 'not json'".into()],
        };
        let runtime = CliRuntime::new(config, vec![]);

        let predictions = runtime.classify(&test_trace(), &[]);
        assert!(predictions.is_empty());
    }

    #[test]
    fn cli_runtime_nonzero_exit_returns_empty() {
        let config = CliRuntimeConfig {
            command: "sh".into(),
            args: vec!["-c".into(), "exit 1".into()],
        };
        let runtime = CliRuntime::new(config, vec![]);

        let predictions = runtime.classify(&test_trace(), &[]);
        assert!(predictions.is_empty());
    }

    #[test]
    fn cli_runtime_config_round_trips() {
        let config = CliRuntimeConfig {
            command: "python".into(),
            args: vec!["scripts/classify.py".into(), "--verbose".into()],
        };
        let json = serde_json::to_value(&config).unwrap();
        assert_eq!(json["command"], "python");
        assert_eq!(json["args"][0], "scripts/classify.py");

        let parsed: CliRuntimeConfig = serde_json::from_value(json).unwrap();
        assert_eq!(parsed.command, "python");
        assert_eq!(parsed.args.len(), 2);
    }
}
