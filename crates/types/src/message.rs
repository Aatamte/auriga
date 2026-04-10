use crate::{ContentBlock, MessageContent, StopReason, TokenUsage, ToolDefinition};

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
