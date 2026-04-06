use orchestrator_core::ContentBlock;
use serde::{Deserialize, Serialize};

/// A tool the model is allowed to invoke.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDefinition {
    /// Tool name (e.g. "bash", "read_file").
    pub name: String,
    /// Human-readable description for the model.
    pub description: String,
    /// JSON Schema describing the tool's input parameters.
    pub input_schema: serde_json::Value,
}

/// A tool invocation requested by the model.
/// Extracted from ContentBlock::ToolUse in the response.
#[derive(Debug, Clone)]
pub struct ToolCall {
    /// Provider-assigned tool call ID.
    pub id: String,
    /// Name of the tool to invoke.
    pub name: String,
    /// Arguments for the tool (JSON object).
    pub input: serde_json::Value,
}

/// The result of executing a tool call.
#[derive(Debug, Clone)]
pub struct ToolOutput {
    /// Must match the ToolCall.id this is responding to.
    pub tool_call_id: String,
    /// The tool's output content.
    pub content: String,
    /// Whether the tool execution failed.
    pub is_error: bool,
}

/// Extract tool calls from a slice of content blocks.
pub fn extract_tool_calls(blocks: &[ContentBlock]) -> Vec<ToolCall> {
    blocks
        .iter()
        .filter_map(|block| match block {
            ContentBlock::ToolUse { id, name, input } => Some(ToolCall {
                id: id.clone(),
                name: name.clone(),
                input: input.clone(),
            }),
            _ => None,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tool_definition_round_trips() {
        let def = ToolDefinition {
            name: "bash".into(),
            description: "Run a shell command".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "command": {"type": "string"}
                },
                "required": ["command"]
            }),
        };
        let json = serde_json::to_string(&def).unwrap();
        let parsed: ToolDefinition = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.name, "bash");
        assert_eq!(
            parsed.input_schema["properties"]["command"]["type"],
            "string"
        );
    }

    #[test]
    fn extract_tool_calls_from_mixed_blocks() {
        let blocks = vec![
            ContentBlock::Text {
                text: "Let me check.".into(),
            },
            ContentBlock::ToolUse {
                id: "tc_1".into(),
                name: "bash".into(),
                input: serde_json::json!({"command": "ls"}),
            },
            ContentBlock::Thinking {
                thinking: "hmm".into(),
                signature: None,
            },
            ContentBlock::ToolUse {
                id: "tc_2".into(),
                name: "read_file".into(),
                input: serde_json::json!({"path": "/tmp/x"}),
            },
        ];
        let calls = extract_tool_calls(&blocks);
        assert_eq!(calls.len(), 2);
        assert_eq!(calls[0].id, "tc_1");
        assert_eq!(calls[0].name, "bash");
        assert_eq!(calls[1].id, "tc_2");
        assert_eq!(calls[1].name, "read_file");
    }

    #[test]
    fn extract_tool_calls_empty_on_no_tool_use() {
        let blocks = vec![ContentBlock::Text {
            text: "done".into(),
        }];
        assert!(extract_tool_calls(&blocks).is_empty());
    }
}
