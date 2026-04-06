use orchestrator_core::{ContentBlock, MessageContent, StopReason, TokenUsage};

use crate::tool::ToolDefinition;

/// Role in a conversation for request construction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    User,
    Assistant,
}

/// A single message in a conversation, used to build requests.
#[derive(Debug, Clone)]
pub struct Message {
    pub role: Role,
    pub content: MessageContent,
}

/// A single-shot LLM generation request.
/// This is the foundational primitive. Managed agent loops
/// and classifier LLM calls are built on top of this.
#[derive(Debug, Clone)]
pub struct GenerateRequest {
    /// System prompt for this request.
    pub system: Option<String>,
    /// The conversation messages.
    pub messages: Vec<Message>,
    /// Maximum tokens to generate.
    pub max_tokens: u32,
    /// Model to use.
    pub model: String,
    /// Temperature override. None = provider default.
    pub temperature: Option<f64>,
    /// Tool definitions available to the model.
    pub tools: Vec<ToolDefinition>,
    /// Stop sequences.
    pub stop_sequences: Vec<String>,
    /// Provider session ID for resuming conversations (e.g. Claude CLI --resume).
    pub resume_session_id: Option<String>,
}

/// Response from a single LLM generation call.
#[derive(Debug, Clone)]
pub struct GenerateResponse {
    /// The response content blocks.
    pub content: Vec<ContentBlock>,
    /// The model that actually responded.
    pub model: String,
    /// Why generation stopped.
    pub stop_reason: StopReason,
    /// Token usage for this call.
    pub usage: TokenUsage,
    /// Provider-assigned request ID for debugging.
    pub request_id: Option<String>,
    /// Provider session ID for resuming this conversation.
    pub provider_session_id: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generate_request_with_simple_text() {
        let req = GenerateRequest {
            system: Some("You are helpful.".into()),
            messages: vec![Message {
                role: Role::User,
                content: MessageContent::Text("Hello".into()),
            }],
            max_tokens: 1024,
            model: "test-model".into(),
            temperature: None,
            tools: vec![],
            stop_sequences: vec![],
            resume_session_id: None,
        };
        assert_eq!(req.messages.len(), 1);
        assert_eq!(req.messages[0].role, Role::User);
    }

    #[test]
    fn generate_response_with_tool_use() {
        let resp = GenerateResponse {
            content: vec![
                ContentBlock::Text {
                    text: "Let me check.".into(),
                },
                ContentBlock::ToolUse {
                    id: "tc_1".into(),
                    name: "bash".into(),
                    input: serde_json::json!({"command": "ls"}),
                },
            ],
            model: "test-model".into(),
            stop_reason: StopReason::ToolUse,
            usage: TokenUsage {
                input_tokens: 100,
                output_tokens: 50,
                cache_creation_input_tokens: None,
                cache_read_input_tokens: None,
            },
            request_id: Some("req-123".into()),
            provider_session_id: None,
        };
        assert_eq!(resp.content.len(), 2);
        assert_eq!(resp.stop_reason, StopReason::ToolUse);
        assert_eq!(resp.usage.total(), 150);
    }
}
