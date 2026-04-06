use orchestrator_core::{ContentBlock, StopReason, TokenUsage};
use serde::Deserialize;
use std::process::{Command, Stdio};

use crate::command::CommandSpec;
use crate::config::{AgentConfig, AgentMode};
use crate::message::{GenerateRequest, GenerateResponse, Role};
use crate::provider::{GenerateError, Provider};

/// Claude-specific provider configuration.
/// Parsed from AgentConfig.provider_config.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct ClaudeProviderConfig {
    /// Path to an MCP config JSON file to pass via --mcp-config.
    #[serde(default)]
    pub mcp_config_path: Option<String>,
    /// Permission mode (e.g. "auto", "plan", "bypassPermissions").
    #[serde(default)]
    pub permission_mode: Option<String>,
    /// Allowed tools list.
    #[serde(default)]
    pub allowed_tools: Vec<String>,
}

/// Provider for Claude Code CLI.
pub struct ClaudeProvider;

impl Provider for ClaudeProvider {
    fn name(&self) -> &str {
        "claude"
    }

    fn generate(&self, request: &GenerateRequest) -> Result<GenerateResponse, GenerateError> {
        // Extract the last user message as the prompt
        let prompt = request
            .messages
            .iter()
            .rev()
            .find(|m| matches!(m.role, Role::User))
            .map(|m| match &m.content {
                orchestrator_core::MessageContent::Text(t) => t.clone(),
                orchestrator_core::MessageContent::Blocks(blocks) => blocks
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

        // Build the claude CLI command
        let mut cmd = Command::new("claude");
        cmd.arg("-p").arg(&prompt);
        cmd.arg("--output-format").arg("json");

        if let Some(ref system) = request.system {
            cmd.arg("--system-prompt").arg(system);
        }

        if !request.model.is_empty() {
            cmd.arg("--model").arg(&request.model);
        }

        if let Some(ref session_id) = request.resume_session_id {
            cmd.arg("--resume").arg(session_id);
        }

        cmd.stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        let child = cmd
            .spawn()
            .map_err(|e| GenerateError::Network(format!("failed to spawn claude CLI: {}", e)))?;

        let output = child.wait_with_output().map_err(|e| {
            GenerateError::Network(format!("failed to read claude CLI output: {}", e))
        })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(GenerateError::Api {
                status: output.status.code().unwrap_or(1) as u16,
                message: stderr.to_string(),
            });
        }

        let entries: Vec<CliEntry> = serde_json::from_slice(&output.stdout).map_err(|e| {
            GenerateError::Serialization(format!("failed to parse claude CLI JSON: {}", e))
        })?;

        parse_cli_entries(&entries)
    }

    fn build_command(&self, config: &AgentConfig) -> Option<CommandSpec> {
        if config.mode != AgentMode::NativeCli {
            return None;
        }

        let mut args = Vec::new();

        // Parse provider-specific config
        let claude_config: ClaudeProviderConfig =
            serde_json::from_value(config.provider_config.clone()).unwrap_or_default();

        if let Some(ref mcp_path) = claude_config.mcp_config_path {
            args.push("--mcp-config".into());
            args.push(mcp_path.clone());
        }

        if let Some(ref prompt) = config.system_prompt {
            args.push("--system-prompt".into());
            args.push(prompt.clone());
        }

        if let Some(ref perm) = claude_config.permission_mode {
            args.push("--permission-mode".into());
            args.push(perm.clone());
        }

        for tool in &claude_config.allowed_tools {
            args.push("--allowedTools".into());
            args.push(tool.clone());
        }

        Some(CommandSpec {
            program: "claude".into(),
            args,
            env: vec![],
        })
    }
}

// ---------------------------------------------------------------------------
// Claude CLI JSON output parsing
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
struct CliEntry {
    #[serde(rename = "type")]
    entry_type: String,
    session_id: Option<String>,
    message: Option<CliMessage>,
    stop_reason: Option<String>,
    is_error: Option<bool>,
    usage: Option<CliUsage>,
}

#[derive(Deserialize)]
struct CliMessage {
    model: Option<String>,
    content: Option<Vec<CliContentBlock>>,
    usage: Option<CliUsage>,
}

#[derive(Deserialize)]
struct CliContentBlock {
    #[serde(rename = "type")]
    block_type: String,
    text: Option<String>,
    thinking: Option<String>,
    id: Option<String>,
    name: Option<String>,
    input: Option<serde_json::Value>,
}

#[derive(Deserialize)]
struct CliUsage {
    input_tokens: Option<u64>,
    output_tokens: Option<u64>,
    cache_creation_input_tokens: Option<u64>,
    cache_read_input_tokens: Option<u64>,
}

fn parse_cli_entries(entries: &[CliEntry]) -> Result<GenerateResponse, GenerateError> {
    // Find session_id from system init entry
    let session_id = entries
        .iter()
        .find(|e| e.entry_type == "system")
        .and_then(|e| e.session_id.clone());

    // Find the last assistant entry for content blocks and model
    let assistant = entries
        .iter()
        .rev()
        .find(|e| e.entry_type == "assistant");

    // Find the result entry for stop_reason and aggregated usage
    let result = entries.iter().find(|e| e.entry_type == "result");

    // Check for error
    if let Some(r) = result {
        if r.is_error == Some(true) {
            return Err(GenerateError::Api {
                status: 0,
                message: "claude CLI returned an error".into(),
            });
        }
    }

    // Extract content blocks from assistant message
    let content = assistant
        .and_then(|a| a.message.as_ref())
        .and_then(|m| m.content.as_ref())
        .map(|blocks| {
            blocks
                .iter()
                .filter_map(map_content_block)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    // If no structured content but result exists, fallback to placeholder
    let content = if content.is_empty() {
        if result.is_some() {
            vec![ContentBlock::Text {
                text: "(empty response)".into(),
            }]
        } else {
            return Err(GenerateError::Other(
                "no assistant message in claude CLI output".into(),
            ));
        }
    } else {
        content
    };

    // Model from assistant message
    let model = assistant
        .and_then(|a| a.message.as_ref())
        .and_then(|m| m.model.clone())
        .unwrap_or_else(|| "claude".into());

    // Stop reason from result entry
    let stop_reason = result
        .and_then(|r| r.stop_reason.as_deref())
        .map(parse_stop_reason)
        .unwrap_or(StopReason::EndTurn);

    // Usage: prefer result entry (aggregated), fall back to assistant message usage
    let usage = result
        .and_then(|r| r.usage.as_ref())
        .or_else(|| {
            assistant
                .and_then(|a| a.message.as_ref())
                .and_then(|m| m.usage.as_ref())
        })
        .map(|u| TokenUsage {
            input_tokens: u.input_tokens.unwrap_or(0),
            output_tokens: u.output_tokens.unwrap_or(0),
            cache_creation_input_tokens: u.cache_creation_input_tokens,
            cache_read_input_tokens: u.cache_read_input_tokens,
        })
        .unwrap_or(TokenUsage {
            input_tokens: 0,
            output_tokens: 0,
            cache_creation_input_tokens: None,
            cache_read_input_tokens: None,
        });

    Ok(GenerateResponse {
        content,
        model,
        stop_reason,
        usage,
        request_id: None,
        provider_session_id: session_id,
    })
}

fn map_content_block(block: &CliContentBlock) -> Option<ContentBlock> {
    match block.block_type.as_str() {
        "text" => Some(ContentBlock::Text {
            text: block.text.clone().unwrap_or_default(),
        }),
        "thinking" => Some(ContentBlock::Thinking {
            thinking: block.thinking.clone().unwrap_or_default(),
            signature: None,
        }),
        "tool_use" => Some(ContentBlock::ToolUse {
            id: block.id.clone().unwrap_or_default(),
            name: block.name.clone().unwrap_or_default(),
            input: block.input.clone().unwrap_or(serde_json::Value::Null),
        }),
        _ => None,
    }
}

fn parse_stop_reason(s: &str) -> StopReason {
    match s {
        "end_turn" => StopReason::EndTurn,
        "max_tokens" => StopReason::MaxTokens,
        "tool_use" => StopReason::ToolUse,
        "stop_sequence" => StopReason::StopSequence,
        _ => StopReason::EndTurn,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base_config() -> AgentConfig {
        AgentConfig {
            name: "test".into(),
            provider: "claude".into(),
            model: "claude-sonnet-4-20250514".into(),
            max_tokens: 4096,
            system_prompt: None,
            temperature: None,
            mode: AgentMode::NativeCli,
            provider_config: serde_json::json!({}),
        }
    }

    #[test]
    fn build_command_basic() {
        let provider = ClaudeProvider;
        let config = base_config();
        let spec = provider.build_command(&config).unwrap();
        assert_eq!(spec.program, "claude");
        assert!(spec.args.is_empty());
    }

    #[test]
    fn build_command_with_system_prompt() {
        let provider = ClaudeProvider;
        let mut config = base_config();
        config.system_prompt = Some("Be helpful.".into());
        let spec = provider.build_command(&config).unwrap();
        assert_eq!(spec.args, vec!["--system-prompt", "Be helpful."]);
    }

    #[test]
    fn build_command_with_mcp_config() {
        let provider = ClaudeProvider;
        let mut config = base_config();
        config.provider_config = serde_json::json!({"mcp_config_path": "/tmp/mcp.json"});
        let spec = provider.build_command(&config).unwrap();
        assert_eq!(spec.args, vec!["--mcp-config", "/tmp/mcp.json"]);
    }

    #[test]
    fn build_command_with_permission_mode() {
        let provider = ClaudeProvider;
        let mut config = base_config();
        config.provider_config = serde_json::json!({"permission_mode": "auto"});
        let spec = provider.build_command(&config).unwrap();
        assert_eq!(spec.args, vec!["--permission-mode", "auto"]);
    }

    #[test]
    fn build_command_returns_none_for_generate_mode() {
        let provider = ClaudeProvider;
        let mut config = base_config();
        config.mode = AgentMode::Generate;
        assert!(provider.build_command(&config).is_none());
    }

    #[test]
    fn generate_empty_messages_returns_error() {
        let provider = ClaudeProvider;
        let req = GenerateRequest {
            system: None,
            messages: vec![],
            max_tokens: 1024,
            model: "test".into(),
            temperature: None,
            tools: vec![],
            stop_sequences: vec![],
            resume_session_id: None,
        };
        assert!(provider.generate(&req).is_err());
    }

    #[test]
    fn parse_simple_text_response() {
        let json = r#"[
            {"type":"system","subtype":"init","session_id":"sess-1","model":"claude-opus-4-6"},
            {"type":"assistant","message":{"model":"claude-opus-4-6","content":[{"type":"text","text":"hello world"}],"usage":{"input_tokens":10,"output_tokens":5}},"session_id":"sess-1"},
            {"type":"result","subtype":"success","is_error":false,"stop_reason":"end_turn","usage":{"input_tokens":10,"output_tokens":5},"session_id":"sess-1"}
        ]"#;
        let entries: Vec<CliEntry> = serde_json::from_str(json).unwrap();
        let resp = parse_cli_entries(&entries).unwrap();

        assert_eq!(resp.content.len(), 1);
        assert!(matches!(&resp.content[0], ContentBlock::Text { text } if text == "hello world"));
        assert_eq!(resp.model, "claude-opus-4-6");
        assert_eq!(resp.stop_reason, StopReason::EndTurn);
        assert_eq!(resp.usage.input_tokens, 10);
        assert_eq!(resp.usage.output_tokens, 5);
        assert_eq!(resp.provider_session_id.as_deref(), Some("sess-1"));
    }

    #[test]
    fn parse_thinking_and_text_response() {
        let json = r#"[
            {"type":"system","subtype":"init","session_id":"sess-2"},
            {"type":"assistant","message":{"model":"claude-opus-4-6","content":[{"type":"thinking","thinking":"let me think..."},{"type":"text","text":"the answer is 42"}],"usage":{"input_tokens":20,"output_tokens":15}},"session_id":"sess-2"},
            {"type":"result","subtype":"success","is_error":false,"stop_reason":"end_turn","usage":{"input_tokens":20,"output_tokens":15},"session_id":"sess-2"}
        ]"#;
        let entries: Vec<CliEntry> = serde_json::from_str(json).unwrap();
        let resp = parse_cli_entries(&entries).unwrap();

        assert_eq!(resp.content.len(), 2);
        assert!(
            matches!(&resp.content[0], ContentBlock::Thinking { thinking, .. } if thinking == "let me think...")
        );
        assert!(
            matches!(&resp.content[1], ContentBlock::Text { text } if text == "the answer is 42")
        );
    }

    #[test]
    fn parse_tool_use_response() {
        let json = r#"[
            {"type":"system","subtype":"init","session_id":"sess-3"},
            {"type":"assistant","message":{"model":"claude-opus-4-6","content":[{"type":"text","text":"Let me check."},{"type":"tool_use","id":"tc_1","name":"bash","input":{"command":"ls"}}],"usage":{"input_tokens":30,"output_tokens":20}},"session_id":"sess-3"},
            {"type":"result","subtype":"success","is_error":false,"stop_reason":"tool_use","usage":{"input_tokens":30,"output_tokens":20},"session_id":"sess-3"}
        ]"#;
        let entries: Vec<CliEntry> = serde_json::from_str(json).unwrap();
        let resp = parse_cli_entries(&entries).unwrap();

        assert_eq!(resp.content.len(), 2);
        assert_eq!(resp.stop_reason, StopReason::ToolUse);
        assert!(matches!(&resp.content[1], ContentBlock::ToolUse { name, .. } if name == "bash"));
    }

    #[test]
    fn parse_error_response() {
        let json = r#"[
            {"type":"system","subtype":"init","session_id":"sess-4"},
            {"type":"result","subtype":"error","is_error":true,"stop_reason":"error","usage":{"input_tokens":0,"output_tokens":0},"session_id":"sess-4"}
        ]"#;
        let entries: Vec<CliEntry> = serde_json::from_str(json).unwrap();
        assert!(parse_cli_entries(&entries).is_err());
    }

    #[test]
    fn parse_malformed_json_returns_error() {
        let bad = b"this is not json";
        let result: Result<Vec<CliEntry>, _> = serde_json::from_slice(bad);
        assert!(result.is_err());
    }

    #[test]
    fn parse_with_cache_tokens() {
        let json = r#"[
            {"type":"system","subtype":"init","session_id":"sess-5"},
            {"type":"assistant","message":{"model":"claude-opus-4-6","content":[{"type":"text","text":"hi"}],"usage":{"input_tokens":2,"output_tokens":1,"cache_creation_input_tokens":5000,"cache_read_input_tokens":10000}},"session_id":"sess-5"},
            {"type":"result","subtype":"success","is_error":false,"stop_reason":"end_turn","usage":{"input_tokens":2,"output_tokens":1,"cache_creation_input_tokens":5000,"cache_read_input_tokens":10000},"session_id":"sess-5"}
        ]"#;
        let entries: Vec<CliEntry> = serde_json::from_str(json).unwrap();
        let resp = parse_cli_entries(&entries).unwrap();

        assert_eq!(resp.usage.cache_creation_input_tokens, Some(5000));
        assert_eq!(resp.usage.cache_read_input_tokens, Some(10000));
    }
}
