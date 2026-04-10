use serde::{Deserialize, Serialize};

use crate::AgentId;

// ---------------------------------------------------------------------------
// IDs
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TurnId(pub usize);

// ---------------------------------------------------------------------------
// Enums
// ---------------------------------------------------------------------------

/// Envelope-level message type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MessageType {
    User,
    Assistant,
    System,
}

/// API-level conversation role.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TurnRole {
    User,
    Assistant,
}

/// Why the model stopped generating.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum StopReason {
    EndTurn,
    ToolUse,
    MaxTokens,
    StopSequence,
}

/// Turn lifecycle status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TurnStatus {
    Active,
    Complete,
}

/// Image encoding kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ImageSourceType {
    Base64,
}

// ---------------------------------------------------------------------------
// Content blocks
// ---------------------------------------------------------------------------

/// A single content block within a message.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ContentBlock {
    Text {
        text: String,
    },
    Thinking {
        thinking: String,
        signature: Option<String>,
    },
    ToolUse {
        id: String,
        name: String,
        input: serde_json::Value,
    },
    ToolResult {
        tool_use_id: String,
        content: ToolResultContent,
        is_error: bool,
    },
    Image {
        source: ImageSource,
    },
}

/// Tool result content: plain text or nested content blocks.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ToolResultContent {
    Text(String),
    Blocks(Vec<ContentBlock>),
}

/// Message body content: plain string or structured content blocks.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum MessageContent {
    Text(String),
    Blocks(Vec<ContentBlock>),
}

// ---------------------------------------------------------------------------
// Value types
// ---------------------------------------------------------------------------

/// Token usage from a model response.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TokenUsage {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_creation_input_tokens: Option<u64>,
    pub cache_read_input_tokens: Option<u64>,
}

impl TokenUsage {
    pub fn total(&self) -> u64 {
        self.input_tokens + self.output_tokens
    }
}

/// Embedded image source data.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImageSource {
    pub source_type: ImageSourceType,
    pub media_type: String,
    pub data: String,
}

// ---------------------------------------------------------------------------
// Per-MessageType metadata
// ---------------------------------------------------------------------------

/// Assistant-specific metadata from the model response.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AssistantMeta {
    pub model: Option<String>,
    pub stop_reason: Option<StopReason>,
    pub stop_sequence: Option<String>,
    pub usage: Option<TokenUsage>,
    pub request_id: Option<String>,
}

/// User message metadata.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UserMeta {
    pub is_meta: bool,
    pub is_compact_summary: bool,
    pub source_tool_assistant_uuid: Option<String>,
}

/// System message metadata.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SystemMeta {
    pub subtype: Option<String>,
    pub level: Option<String>,
}

/// Message-type-specific metadata.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum TurnMeta {
    User(UserMeta),
    Assistant(AssistantMeta),
    System(SystemMeta),
}

// ---------------------------------------------------------------------------
// Turn entity
// ---------------------------------------------------------------------------

/// A single conversation turn — one message in an agent's conversation history.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Turn {
    // Internal identity (assigned by TurnStore)
    pub id: TurnId,
    pub agent_id: AgentId,
    pub number: u32,
    pub status: TurnStatus,

    // External identity (from log)
    pub uuid: String,
    pub parent_uuid: Option<String>,
    pub session_id: Option<String>,
    pub timestamp: String,

    // Envelope context
    pub message_type: MessageType,
    pub cwd: Option<String>,
    pub git_branch: Option<String>,

    // Message body
    pub role: TurnRole,
    pub content: MessageContent,

    // Type-specific metadata
    pub meta: TurnMeta,

    // Catch-all for unmodeled fields (lossless)
    pub extra: serde_json::Value,
}

// ---------------------------------------------------------------------------
// TurnBuilder
// ---------------------------------------------------------------------------

/// Builder for constructing a Turn before handing it to the store.
/// The store assigns `id`, `agent_id`, and `number`.
#[derive(Debug, Clone)]
pub struct TurnBuilder {
    pub uuid: String,
    pub parent_uuid: Option<String>,
    pub session_id: Option<String>,
    pub timestamp: String,
    pub message_type: MessageType,
    pub cwd: Option<String>,
    pub git_branch: Option<String>,
    pub role: TurnRole,
    pub content: MessageContent,
    pub meta: TurnMeta,
    pub status: TurnStatus,
    pub extra: serde_json::Value,
}

impl TurnBuilder {
    pub fn build(self, id: TurnId, agent_id: AgentId, number: u32) -> Turn {
        Turn {
            id,
            agent_id,
            number,
            status: self.status,
            uuid: self.uuid,
            parent_uuid: self.parent_uuid,
            session_id: self.session_id,
            timestamp: self.timestamp,
            message_type: self.message_type,
            cwd: self.cwd,
            git_branch: self.git_branch,
            role: self.role,
            content: self.content,
            meta: self.meta,
            extra: self.extra,
        }
    }
}
