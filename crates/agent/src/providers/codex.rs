//! Provider for the OpenAI Codex CLI (`codex` v0.117+).
//!
//! Shape mirrors `claude.rs`:
//! - `build_command` emits argv for launching an interactive `codex` session.
//! - `generate` runs `codex exec --json`, parses the JSONL event stream,
//!   and reduces it into a `GenerateResponse`.

use orchestrator_types::{
    AgentConfig, AgentMode, CodexCliConfig, CommandSpec, ContentBlock, GenerateError,
    GenerateRequest, GenerateResponse, StopReason, TokenUsage,
};
use serde::Deserialize;
use std::io::Write;
use std::process::{Command, Stdio};

use crate::provider::Provider;

pub struct CodexProvider;

impl Provider for CodexProvider {
    fn name(&self) -> &str {
        "codex"
    }

    fn generate(&self, request: &GenerateRequest) -> Result<GenerateResponse, GenerateError> {
        let prompt = request
            .messages
            .iter()
            .rev()
            .find(|m| matches!(m.role, orchestrator_types::Role::User))
            .map(|m| match &m.content {
                orchestrator_types::MessageContent::Text(t) => t.clone(),
                orchestrator_types::MessageContent::Blocks(blocks) => blocks
                    .iter()
                    .filter_map(|b| match b {
                        ContentBlock::Text { text } => Some(text.as_str()),
                        _ => None,
                    })
                    .collect::<Vec<_>>()
                    .join(" "),
            })
            .unwrap_or_default();

        if prompt.is_empty() {
            return Err(GenerateError::Other("no user message in request".into()));
        }

        // Always ephemeral + skip-git-repo-check for one-shot generate calls.
        // This is a stateless API-style invocation, not a persisted session.
        let mut cmd = Command::new("codex");
        cmd.arg("exec");
        if let Some(ref sid) = request.resume_session_id {
            cmd.arg("resume").arg(sid);
        }
        cmd.arg("--json")
            .arg("--ephemeral")
            .arg("--skip-git-repo-check");
        if !request.model.is_empty() {
            cmd.arg("--model").arg(&request.model);
        }
        // Read prompt from stdin via `-`
        cmd.arg("-");

        cmd.stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        let mut child = cmd
            .spawn()
            .map_err(|e| GenerateError::Network(format!("failed to spawn codex CLI: {}", e)))?;

        if let Some(mut stdin) = child.stdin.take() {
            stdin.write_all(prompt.as_bytes()).map_err(|e| {
                GenerateError::Network(format!("failed to write prompt to codex stdin: {}", e))
            })?;
        }

        let output = child.wait_with_output().map_err(|e| {
            GenerateError::Network(format!("failed to read codex CLI output: {}", e))
        })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(GenerateError::Api {
                status: output.status.code().unwrap_or(1) as u16,
                message: stderr.to_string(),
            });
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut events = Vec::new();
        for line in stdout.lines() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            let ev: CodexEvent = serde_json::from_str(line).map_err(|e| {
                GenerateError::Serialization(format!("failed to parse codex event: {}", e))
            })?;
            events.push(ev);
        }

        parse_events(&events, &request.model)
    }

    fn build_command(&self, config: &AgentConfig) -> Option<CommandSpec> {
        if config.mode != AgentMode::NativeCli {
            return None;
        }

        let mut cli_config: CodexCliConfig =
            serde_json::from_value(config.provider_config.clone()).unwrap_or_default();

        // If model not set on CodexCliConfig, inherit from AgentConfig
        if cli_config.model.is_none() && !config.model.is_empty() {
            cli_config.model = Some(config.model.clone());
        }

        let args = cli_config.to_interactive_args();
        let env = cli_config.env.clone();

        Some(CommandSpec {
            program: "codex".into(),
            args,
            env,
        })
    }
}

// ---------------------------------------------------------------------------
// Codex `exec --json` event parsing
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
enum CodexEvent {
    #[serde(rename = "thread.started")]
    ThreadStarted { thread_id: String },
    #[serde(rename = "turn.started")]
    TurnStarted,
    #[serde(rename = "turn.completed")]
    TurnCompleted { usage: Option<CodexUsage> },
    #[serde(rename = "turn.failed")]
    TurnFailed {
        #[serde(default)]
        error: Option<serde_json::Value>,
    },
    #[serde(rename = "item.started")]
    ItemStarted {
        #[allow(dead_code)]
        item: CodexItem,
    },
    #[serde(rename = "item.completed")]
    ItemCompleted { item: CodexItem },
    #[serde(rename = "error")]
    Error {
        #[serde(default)]
        message: Option<String>,
    },
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
#[allow(dead_code)] // fields populated by serde; some only used in specific variants
enum CodexItem {
    #[serde(rename = "agent_message")]
    AgentMessage {
        #[serde(default)]
        id: String,
        #[serde(default)]
        text: String,
    },
    #[serde(rename = "command_execution")]
    CommandExecution {
        #[serde(default)]
        id: String,
        #[serde(default)]
        command: String,
        #[serde(default)]
        aggregated_output: String,
        #[serde(default)]
        exit_code: Option<i64>,
        #[serde(default)]
        status: Option<String>,
    },
    #[serde(other)]
    Other,
}

#[derive(Debug, Deserialize)]
struct CodexUsage {
    #[serde(default)]
    input_tokens: Option<u64>,
    #[serde(default)]
    output_tokens: Option<u64>,
    #[serde(default)]
    cached_input_tokens: Option<u64>,
}

fn parse_events(
    events: &[CodexEvent],
    request_model: &str,
) -> Result<GenerateResponse, GenerateError> {
    let mut thread_id: Option<String> = None;
    let mut content: Vec<ContentBlock> = Vec::new();
    let mut turn_usage: Option<&CodexUsage> = None;
    let mut turn_completed = false;

    for ev in events {
        match ev {
            CodexEvent::ThreadStarted { thread_id: tid } => {
                thread_id = Some(tid.clone());
            }
            CodexEvent::TurnStarted => {}
            CodexEvent::TurnCompleted { usage } => {
                turn_completed = true;
                turn_usage = usage.as_ref();
            }
            CodexEvent::TurnFailed { error } => {
                let msg = error
                    .as_ref()
                    .map(|v| v.to_string())
                    .unwrap_or_else(|| "codex turn failed".into());
                return Err(GenerateError::Api {
                    status: 0,
                    message: msg,
                });
            }
            CodexEvent::Error { message } => {
                return Err(GenerateError::Api {
                    status: 0,
                    message: message.clone().unwrap_or_else(|| "codex error".into()),
                });
            }
            CodexEvent::ItemStarted { .. } => {
                // The final state is delivered in ItemCompleted; skip intermediates.
            }
            CodexEvent::ItemCompleted { item } => match item {
                CodexItem::AgentMessage { text, .. } => {
                    content.push(ContentBlock::Text { text: text.clone() });
                }
                CodexItem::CommandExecution {
                    id,
                    command,
                    aggregated_output,
                    exit_code,
                    status,
                } => {
                    let input = serde_json::json!({
                        "command": command,
                        "output": aggregated_output,
                        "exit_code": exit_code,
                        "status": status,
                    });
                    content.push(ContentBlock::ToolUse {
                        id: id.clone(),
                        name: "shell".into(),
                        input,
                    });
                }
                CodexItem::Other => {}
            },
            CodexEvent::Unknown => {}
        }
    }

    if !turn_completed {
        return Err(GenerateError::Other(
            "codex exec produced no turn.completed event".into(),
        ));
    }

    if content.is_empty() {
        content.push(ContentBlock::Text {
            text: "(empty response)".into(),
        });
    }

    let usage = turn_usage
        .map(|u| TokenUsage {
            input_tokens: u.input_tokens.unwrap_or(0),
            output_tokens: u.output_tokens.unwrap_or(0),
            cache_creation_input_tokens: None,
            cache_read_input_tokens: u.cached_input_tokens,
        })
        .unwrap_or(TokenUsage {
            input_tokens: 0,
            output_tokens: 0,
            cache_creation_input_tokens: None,
            cache_read_input_tokens: None,
        });

    let model = if request_model.is_empty() {
        "codex".into()
    } else {
        request_model.to_string()
    };

    Ok(GenerateResponse {
        content,
        model,
        stop_reason: StopReason::EndTurn,
        usage,
        request_id: None,
        provider_session_id: thread_id,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use orchestrator_types::GenerateRequest;

    fn base_config() -> AgentConfig {
        AgentConfig {
            name: "test".into(),
            provider: "codex".into(),
            model: "gpt-5".into(),
            max_tokens: 4096,
            system_prompt: None,
            temperature: None,
            mode: AgentMode::NativeCli,
            provider_config: serde_json::json!({}),
        }
    }

    #[test]
    fn name_is_codex() {
        assert_eq!(CodexProvider.name(), "codex");
    }

    #[test]
    fn build_command_inherits_model_from_agent_config() {
        let provider = CodexProvider;
        let config = base_config();
        let spec = provider.build_command(&config).unwrap();
        assert_eq!(spec.program, "codex");
        assert_eq!(spec.args, vec!["--model", "gpt-5"]);
    }

    #[test]
    fn build_command_cli_config_model_wins_over_agent_config() {
        let provider = CodexProvider;
        let mut config = base_config();
        config.provider_config = serde_json::json!({"model": "o3"});
        let spec = provider.build_command(&config).unwrap();
        assert!(spec.args.contains(&"--model".into()));
        assert!(spec.args.contains(&"o3".into()));
        assert!(!spec.args.contains(&"gpt-5".into()));
    }

    #[test]
    fn build_command_with_full_cli_config() {
        let provider = CodexProvider;
        let mut config = base_config();
        config.provider_config = serde_json::json!({
            "sandbox": "workspace-write",
            "approval": "on-request",
            "full_auto": true,
            "add_dirs": ["/tmp/extra"],
        });
        let spec = provider.build_command(&config).unwrap();
        assert!(spec.args.contains(&"--sandbox".into()));
        assert!(spec.args.contains(&"workspace-write".into()));
        assert!(spec.args.contains(&"--ask-for-approval".into()));
        assert!(spec.args.contains(&"on-request".into()));
        assert!(spec.args.contains(&"--full-auto".into()));
        assert!(spec.args.contains(&"--add-dir".into()));
        assert!(spec.args.contains(&"/tmp/extra".into()));
    }

    #[test]
    fn build_command_returns_none_for_generate_mode() {
        let provider = CodexProvider;
        let mut config = base_config();
        config.mode = AgentMode::Generate;
        assert!(provider.build_command(&config).is_none());
    }

    #[test]
    fn generate_empty_messages_returns_error() {
        let provider = CodexProvider;
        let req = GenerateRequest {
            system: None,
            messages: vec![],
            max_tokens: 1024,
            model: "gpt-5".into(),
            temperature: None,
            tools: vec![],
            stop_sequences: vec![],
            resume_session_id: None,
        };
        assert!(provider.generate(&req).is_err());
    }

    // ---- parse_events tests ----

    fn parse(lines: &[&str]) -> Vec<CodexEvent> {
        lines
            .iter()
            .map(|l| serde_json::from_str(l).unwrap())
            .collect()
    }

    #[test]
    fn parse_simple_text_response() {
        let events = parse(&[
            r#"{"type":"thread.started","thread_id":"tid-1"}"#,
            r#"{"type":"turn.started"}"#,
            r#"{"type":"item.completed","item":{"id":"item_0","type":"agent_message","text":"hello"}}"#,
            r#"{"type":"turn.completed","usage":{"input_tokens":100,"cached_input_tokens":10,"output_tokens":5}}"#,
        ]);
        let resp = parse_events(&events, "gpt-5").unwrap();
        assert_eq!(resp.content.len(), 1);
        assert!(matches!(&resp.content[0], ContentBlock::Text { text } if text == "hello"));
        assert_eq!(resp.stop_reason, StopReason::EndTurn);
        assert_eq!(resp.usage.input_tokens, 100);
        assert_eq!(resp.usage.output_tokens, 5);
        assert_eq!(resp.usage.cache_read_input_tokens, Some(10));
        assert_eq!(resp.usage.cache_creation_input_tokens, None);
        assert_eq!(resp.provider_session_id.as_deref(), Some("tid-1"));
        assert_eq!(resp.model, "gpt-5");
    }

    #[test]
    fn parse_text_then_command_execution() {
        let events = parse(&[
            r#"{"type":"thread.started","thread_id":"tid-2"}"#,
            r#"{"type":"turn.started"}"#,
            r#"{"type":"item.completed","item":{"id":"item_0","type":"agent_message","text":"running"}}"#,
            r#"{"type":"item.started","item":{"id":"item_1","type":"command_execution","command":"echo hi","aggregated_output":"","exit_code":null,"status":"in_progress"}}"#,
            r#"{"type":"item.completed","item":{"id":"item_1","type":"command_execution","command":"echo hi","aggregated_output":"hi\n","exit_code":0,"status":"completed"}}"#,
            r#"{"type":"item.completed","item":{"id":"item_2","type":"agent_message","text":"printed hi"}}"#,
            r#"{"type":"turn.completed","usage":{"input_tokens":200,"output_tokens":40}}"#,
        ]);
        let resp = parse_events(&events, "gpt-5").unwrap();
        assert_eq!(resp.content.len(), 3);
        assert!(matches!(&resp.content[0], ContentBlock::Text { text } if text == "running"));
        assert!(matches!(
            &resp.content[1],
            ContentBlock::ToolUse { name, .. } if name == "shell"
        ));
        if let ContentBlock::ToolUse { input, .. } = &resp.content[1] {
            assert_eq!(input["command"], "echo hi");
            assert_eq!(input["output"], "hi\n");
            assert_eq!(input["exit_code"], 0);
        }
        assert!(matches!(&resp.content[2], ContentBlock::Text { text } if text == "printed hi"));
    }

    #[test]
    fn parse_turn_failed_returns_error() {
        let events = parse(&[
            r#"{"type":"thread.started","thread_id":"tid-3"}"#,
            r#"{"type":"turn.failed","error":{"kind":"boom"}}"#,
        ]);
        assert!(parse_events(&events, "gpt-5").is_err());
    }

    #[test]
    fn parse_error_event_returns_error() {
        let events = parse(&[r#"{"type":"error","message":"auth failed"}"#]);
        let err = parse_events(&events, "gpt-5").unwrap_err();
        match err {
            GenerateError::Api { message, .. } => assert!(message.contains("auth failed")),
            _ => panic!("expected Api error"),
        }
    }

    #[test]
    fn parse_missing_turn_completed_returns_error() {
        let events = parse(&[
            r#"{"type":"thread.started","thread_id":"tid-4"}"#,
            r#"{"type":"turn.started"}"#,
            r#"{"type":"item.completed","item":{"id":"item_0","type":"agent_message","text":"hi"}}"#,
        ]);
        assert!(parse_events(&events, "gpt-5").is_err());
    }

    #[test]
    fn parse_unknown_event_and_item_types_are_skipped() {
        let events = parse(&[
            r#"{"type":"thread.started","thread_id":"tid-5"}"#,
            r#"{"type":"some.future.event","foo":"bar"}"#,
            r#"{"type":"item.completed","item":{"id":"item_x","type":"future_item_type","data":"whatever"}}"#,
            r#"{"type":"item.completed","item":{"id":"item_0","type":"agent_message","text":"ok"}}"#,
            r#"{"type":"turn.completed","usage":{"input_tokens":1,"output_tokens":1}}"#,
        ]);
        let resp = parse_events(&events, "gpt-5").unwrap();
        assert_eq!(resp.content.len(), 1);
    }

    #[test]
    fn parse_empty_agent_message_produces_fallback() {
        let events = parse(&[
            r#"{"type":"thread.started","thread_id":"tid-6"}"#,
            r#"{"type":"turn.completed","usage":{"input_tokens":1,"output_tokens":0}}"#,
        ]);
        let resp = parse_events(&events, "").unwrap();
        assert_eq!(resp.content.len(), 1);
        assert!(
            matches!(&resp.content[0], ContentBlock::Text { text } if text == "(empty response)")
        );
        assert_eq!(resp.model, "codex");
    }
}
