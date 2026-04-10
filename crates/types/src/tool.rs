use serde::{Deserialize, Serialize};

use crate::ContentBlock;

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
