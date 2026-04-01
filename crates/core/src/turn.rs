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
    fn build(self, id: TurnId, agent_id: AgentId, number: u32) -> Turn {
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

// ---------------------------------------------------------------------------
// TurnStore
// ---------------------------------------------------------------------------

#[derive(Debug)]
pub struct TurnStore {
    turns: Vec<Turn>,
    next_id: usize,
}

impl TurnStore {
    pub fn new() -> Self {
        Self {
            turns: Vec::new(),
            next_id: 1,
        }
    }

    /// Insert a fully constructed turn. Assigns a TurnId and per-agent number.
    pub fn insert(&mut self, agent_id: AgentId, builder: TurnBuilder) -> TurnId {
        let number = self.agent_turn_count(agent_id) as u32 + 1;
        let id = TurnId(self.next_id);
        self.next_id += 1;
        self.turns.push(builder.build(id, agent_id, number));
        id
    }

    pub fn get(&self, id: TurnId) -> Option<&Turn> {
        self.turns.iter().find(|t| t.id == id)
    }

    pub fn get_mut(&mut self, id: TurnId) -> Option<&mut Turn> {
        self.turns.iter_mut().find(|t| t.id == id)
    }

    /// Find a turn by its external UUID.
    pub fn find_by_uuid(&self, uuid: &str) -> Option<&Turn> {
        self.turns.iter().find(|t| t.uuid == uuid)
    }

    /// All turns for a given agent, in insertion order.
    pub fn turns_for(&self, agent_id: AgentId) -> Vec<&Turn> {
        self.turns
            .iter()
            .filter(|t| t.agent_id == agent_id)
            .collect()
    }

    /// The currently active turn for an agent, if any.
    pub fn active_turn(&self, agent_id: AgentId) -> Option<&Turn> {
        self.turns
            .iter()
            .rfind(|t| t.agent_id == agent_id && t.status == TurnStatus::Active)
    }

    /// Mark a turn as complete. Returns false if not found or already complete.
    pub fn complete_turn(&mut self, id: TurnId) -> bool {
        if let Some(turn) = self.turns.iter_mut().find(|t| t.id == id) {
            if turn.status == TurnStatus::Active {
                turn.status = TurnStatus::Complete;
                return true;
            }
        }
        false
    }

    /// Total number of turns for an agent.
    pub fn agent_turn_count(&self, agent_id: AgentId) -> usize {
        self.turns.iter().filter(|t| t.agent_id == agent_id).count()
    }

    /// Total number of turns across all agents.
    pub fn count(&self) -> usize {
        self.turns.len()
    }

    /// Remove all turns for an agent. Prevents orphaned state.
    pub fn remove_agent_turns(&mut self, agent_id: AgentId) {
        self.turns.retain(|t| t.agent_id != agent_id);
    }

    /// Accumulated token usage for an agent across all its assistant turns.
    pub fn agent_token_usage(&self, agent_id: AgentId) -> TokenUsage {
        let mut total = TokenUsage {
            input_tokens: 0,
            output_tokens: 0,
            cache_creation_input_tokens: None,
            cache_read_input_tokens: None,
        };
        for turn in self.turns.iter().filter(|t| t.agent_id == agent_id) {
            if let TurnMeta::Assistant(ref meta) = turn.meta {
                if let Some(ref usage) = meta.usage {
                    total.input_tokens += usage.input_tokens;
                    total.output_tokens += usage.output_tokens;
                    if let Some(v) = usage.cache_creation_input_tokens {
                        *total.cache_creation_input_tokens.get_or_insert(0) += v;
                    }
                    if let Some(v) = usage.cache_read_input_tokens {
                        *total.cache_read_input_tokens.get_or_insert(0) += v;
                    }
                }
            }
        }
        total
    }
}

impl Default for TurnStore {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn agent(n: u128) -> AgentId {
        AgentId::from_u128(n)
    }

    fn user_builder(uuid: &str) -> TurnBuilder {
        TurnBuilder {
            uuid: uuid.to_string(),
            parent_uuid: None,
            session_id: None,
            timestamp: "2026-03-29T10:00:00Z".to_string(),
            message_type: MessageType::User,
            cwd: None,
            git_branch: None,
            role: TurnRole::User,
            content: MessageContent::Text("hello".to_string()),
            meta: TurnMeta::User(UserMeta {
                is_meta: false,
                is_compact_summary: false,
                source_tool_assistant_uuid: None,
            }),
            status: TurnStatus::Complete,
            extra: json!({}),
        }
    }

    fn assistant_builder(uuid: &str, usage: Option<TokenUsage>) -> TurnBuilder {
        TurnBuilder {
            uuid: uuid.to_string(),
            parent_uuid: None,
            session_id: None,
            timestamp: "2026-03-29T10:00:01Z".to_string(),
            message_type: MessageType::Assistant,
            cwd: None,
            git_branch: None,
            role: TurnRole::Assistant,
            content: MessageContent::Blocks(vec![ContentBlock::Text {
                text: "response".to_string(),
            }]),
            meta: TurnMeta::Assistant(AssistantMeta {
                model: Some("claude-opus-4-6".to_string()),
                stop_reason: Some(StopReason::EndTurn),
                stop_sequence: None,
                usage,
                request_id: None,
            }),
            status: TurnStatus::Complete,
            extra: json!({}),
        }
    }

    // -- Content blocks --

    #[test]
    fn text_block_stores_text() {
        let block = ContentBlock::Text {
            text: "hello world".to_string(),
        };
        assert_eq!(
            block,
            ContentBlock::Text {
                text: "hello world".to_string()
            }
        );
    }

    #[test]
    fn thinking_block_with_signature() {
        let block = ContentBlock::Thinking {
            thinking: "let me think".to_string(),
            signature: Some("sig123".to_string()),
        };
        if let ContentBlock::Thinking {
            thinking,
            signature,
        } = &block
        {
            assert_eq!(thinking, "let me think");
            assert_eq!(signature.as_deref(), Some("sig123"));
        } else {
            panic!("expected Thinking block");
        }
    }

    #[test]
    fn thinking_block_without_signature() {
        let block = ContentBlock::Thinking {
            thinking: "hmm".to_string(),
            signature: None,
        };
        if let ContentBlock::Thinking { signature, .. } = &block {
            assert!(signature.is_none());
        } else {
            panic!("expected Thinking block");
        }
    }

    #[test]
    fn tool_use_preserves_arbitrary_json_input() {
        let input = json!({"command": "ls -la", "timeout": 5000});
        let block = ContentBlock::ToolUse {
            id: "toolu_001".to_string(),
            name: "bash".to_string(),
            input: input.clone(),
        };
        if let ContentBlock::ToolUse {
            id, name, input: i, ..
        } = &block
        {
            assert_eq!(id, "toolu_001");
            assert_eq!(name, "bash");
            assert_eq!(i, &input);
        } else {
            panic!("expected ToolUse block");
        }
    }

    #[test]
    fn tool_result_with_string_content() {
        let block = ContentBlock::ToolResult {
            tool_use_id: "toolu_001".to_string(),
            content: ToolResultContent::Text("file contents here".to_string()),
            is_error: false,
        };
        if let ContentBlock::ToolResult {
            content, is_error, ..
        } = &block
        {
            assert!(!is_error);
            assert_eq!(
                content,
                &ToolResultContent::Text("file contents here".to_string())
            );
        } else {
            panic!("expected ToolResult block");
        }
    }

    #[test]
    fn tool_result_with_block_content() {
        let inner = vec![ContentBlock::Text {
            text: "nested".to_string(),
        }];
        let block = ContentBlock::ToolResult {
            tool_use_id: "toolu_002".to_string(),
            content: ToolResultContent::Blocks(inner.clone()),
            is_error: false,
        };
        if let ContentBlock::ToolResult { content, .. } = &block {
            assert_eq!(content, &ToolResultContent::Blocks(inner));
        } else {
            panic!("expected ToolResult block");
        }
    }

    #[test]
    fn tool_result_error_flag() {
        let block = ContentBlock::ToolResult {
            tool_use_id: "toolu_003".to_string(),
            content: ToolResultContent::Text("command failed".to_string()),
            is_error: true,
        };
        if let ContentBlock::ToolResult { is_error, .. } = &block {
            assert!(is_error);
        } else {
            panic!("expected ToolResult block");
        }
    }

    #[test]
    fn image_block_stores_source() {
        let block = ContentBlock::Image {
            source: ImageSource {
                source_type: ImageSourceType::Base64,
                media_type: "image/png".to_string(),
                data: "iVBOR...".to_string(),
            },
        };
        if let ContentBlock::Image { source } = &block {
            assert_eq!(source.source_type, ImageSourceType::Base64);
            assert_eq!(source.media_type, "image/png");
            assert_eq!(source.data, "iVBOR...");
        } else {
            panic!("expected Image block");
        }
    }

    // -- MessageContent --

    #[test]
    fn message_content_text_variant() {
        let content = MessageContent::Text("plain string".to_string());
        assert_eq!(content, MessageContent::Text("plain string".to_string()));
    }

    #[test]
    fn message_content_blocks_variant() {
        let blocks = vec![
            ContentBlock::Text {
                text: "hello".to_string(),
            },
            ContentBlock::Thinking {
                thinking: "hmm".to_string(),
                signature: None,
            },
        ];
        let content = MessageContent::Blocks(blocks.clone());
        assert_eq!(content, MessageContent::Blocks(blocks));
    }

    // -- Store: insert and IDs --

    #[test]
    fn insert_assigns_incrementing_ids() {
        let mut store = TurnStore::new();
        let a = store.insert(agent(1), user_builder("uuid-1"));
        let b = store.insert(agent(1), user_builder("uuid-2"));
        assert_eq!(a, TurnId(1));
        assert_eq!(b, TurnId(2));
    }

    #[test]
    fn insert_assigns_per_agent_numbers() {
        let mut store = TurnStore::new();
        store.insert(agent(1), user_builder("u1"));
        store.insert(agent(2), user_builder("u2"));
        store.insert(agent(1), assistant_builder("a1", None));

        let a1_turns = store.turns_for(agent(1));
        assert_eq!(a1_turns[0].number, 1);
        assert_eq!(a1_turns[1].number, 2);

        let a2_turns = store.turns_for(agent(2));
        assert_eq!(a2_turns[0].number, 1);
    }

    // -- Store: get / get_mut --

    #[test]
    fn get_returns_inserted_turn() {
        let mut store = TurnStore::new();
        let id = store.insert(agent(1), user_builder("uuid-1"));
        let turn = store.get(id).unwrap();
        assert_eq!(turn.uuid, "uuid-1");
        assert_eq!(turn.agent_id, agent(1));
    }

    #[test]
    fn get_nonexistent_returns_none() {
        let store = TurnStore::new();
        assert!(store.get(TurnId(999)).is_none());
    }

    #[test]
    fn get_mut_allows_modification() {
        let mut store = TurnStore::new();
        let id = store.insert(agent(1), user_builder("uuid-1"));
        store.get_mut(id).unwrap().cwd = Some("/new/path".to_string());
        assert_eq!(store.get(id).unwrap().cwd.as_deref(), Some("/new/path"));
    }

    // -- Store: find_by_uuid --

    #[test]
    fn find_by_uuid_returns_matching_turn() {
        let mut store = TurnStore::new();
        store.insert(agent(1), user_builder("abc-123"));
        let found = store.find_by_uuid("abc-123").unwrap();
        assert_eq!(found.uuid, "abc-123");
    }

    #[test]
    fn find_by_uuid_returns_none_for_unknown() {
        let store = TurnStore::new();
        assert!(store.find_by_uuid("nope").is_none());
    }

    // -- Store: turns_for --

    #[test]
    fn turns_for_returns_only_matching_agent() {
        let mut store = TurnStore::new();
        store.insert(agent(1), user_builder("u1"));
        store.insert(agent(2), user_builder("u2"));
        store.insert(agent(1), assistant_builder("a1", None));

        assert_eq!(store.turns_for(agent(1)).len(), 2);
        assert_eq!(store.turns_for(agent(2)).len(), 1);
    }

    // -- Store: active_turn / complete_turn --

    #[test]
    fn active_turn_returns_active() {
        let mut store = TurnStore::new();
        let mut builder = user_builder("u1");
        builder.status = TurnStatus::Active;
        let id = store.insert(agent(1), builder);
        let turn = store.active_turn(agent(1)).unwrap();
        assert_eq!(turn.id, id);
    }

    #[test]
    fn active_turn_returns_none_when_all_complete() {
        let mut store = TurnStore::new();
        store.insert(agent(1), user_builder("u1"));
        assert!(store.active_turn(agent(1)).is_none());
    }

    #[test]
    fn complete_turn_transitions_status() {
        let mut store = TurnStore::new();
        let mut builder = user_builder("u1");
        builder.status = TurnStatus::Active;
        let id = store.insert(agent(1), builder);

        assert!(store.complete_turn(id));
        assert_eq!(store.get(id).unwrap().status, TurnStatus::Complete);
    }

    #[test]
    fn complete_already_complete_returns_false() {
        let mut store = TurnStore::new();
        let id = store.insert(agent(1), user_builder("u1"));
        assert!(!store.complete_turn(id));
    }

    #[test]
    fn complete_nonexistent_returns_false() {
        let mut store = TurnStore::new();
        assert!(!store.complete_turn(TurnId(999)));
    }

    // -- Store: counts --

    #[test]
    fn agent_turn_count_matches_turns_for_length() {
        let mut store = TurnStore::new();
        store.insert(agent(1), user_builder("u1"));
        store.insert(agent(1), assistant_builder("a1", None));
        assert_eq!(store.agent_turn_count(agent(1)), 2);
        assert_eq!(store.agent_turn_count(agent(99)), 0);
    }

    #[test]
    fn count_returns_total() {
        let mut store = TurnStore::new();
        store.insert(agent(1), user_builder("u1"));
        store.insert(agent(2), user_builder("u2"));
        assert_eq!(store.count(), 2);
    }

    // -- Store: remove --

    #[test]
    fn remove_agent_turns_cleans_up() {
        let mut store = TurnStore::new();
        store.insert(agent(1), user_builder("u1"));
        store.insert(agent(2), user_builder("u2"));
        store.insert(agent(1), assistant_builder("a1", None));

        store.remove_agent_turns(agent(1));
        assert_eq!(store.agent_turn_count(agent(1)), 0);
        assert_eq!(store.agent_turn_count(agent(2)), 1);
    }

    #[test]
    fn remove_agent_turns_for_unknown_is_noop() {
        let mut store = TurnStore::new();
        store.insert(agent(1), user_builder("u1"));
        store.remove_agent_turns(agent(99));
        assert_eq!(store.agent_turn_count(agent(1)), 1);
    }

    // -- Lossless field preservation --

    #[test]
    fn turn_preserves_uuid_and_parent() {
        let mut store = TurnStore::new();
        let mut builder = user_builder("uuid-child");
        builder.parent_uuid = Some("uuid-parent".to_string());
        let id = store.insert(agent(1), builder);

        let turn = store.get(id).unwrap();
        assert_eq!(turn.uuid, "uuid-child");
        assert_eq!(turn.parent_uuid.as_deref(), Some("uuid-parent"));
    }

    #[test]
    fn turn_preserves_cwd_and_branch() {
        let mut store = TurnStore::new();
        let mut builder = user_builder("u1");
        builder.cwd = Some("/home/user/project".to_string());
        builder.git_branch = Some("feature/turns".to_string());
        let id = store.insert(agent(1), builder);

        let turn = store.get(id).unwrap();
        assert_eq!(turn.cwd.as_deref(), Some("/home/user/project"));
        assert_eq!(turn.git_branch.as_deref(), Some("feature/turns"));
    }

    #[test]
    fn turn_preserves_timestamp() {
        let mut store = TurnStore::new();
        let mut builder = user_builder("u1");
        builder.timestamp = "2026-01-15T08:30:00Z".to_string();
        let id = store.insert(agent(1), builder);

        assert_eq!(store.get(id).unwrap().timestamp, "2026-01-15T08:30:00Z");
    }

    #[test]
    fn turn_preserves_session_id() {
        let mut store = TurnStore::new();
        let mut builder = user_builder("u1");
        builder.session_id = Some("sess-abc".to_string());
        let id = store.insert(agent(1), builder);

        assert_eq!(
            store.get(id).unwrap().session_id.as_deref(),
            Some("sess-abc")
        );
    }

    #[test]
    fn turn_preserves_extra_fields() {
        let mut store = TurnStore::new();
        let mut builder = user_builder("u1");
        builder.extra = json!({"isSidechain": true, "permissionMode": "auto"});
        let id = store.insert(agent(1), builder);

        let extra = &store.get(id).unwrap().extra;
        assert_eq!(extra["isSidechain"], true);
        assert_eq!(extra["permissionMode"], "auto");
    }

    // -- Metadata --

    #[test]
    fn assistant_meta_preserves_model_and_usage() {
        let usage = TokenUsage {
            input_tokens: 100,
            output_tokens: 50,
            cache_creation_input_tokens: Some(10),
            cache_read_input_tokens: None,
        };
        let mut store = TurnStore::new();
        let id = store.insert(agent(1), assistant_builder("a1", Some(usage)));

        let turn = store.get(id).unwrap();
        if let TurnMeta::Assistant(ref meta) = turn.meta {
            assert_eq!(meta.model.as_deref(), Some("claude-opus-4-6"));
            assert_eq!(meta.stop_reason, Some(StopReason::EndTurn));
            let u = meta.usage.as_ref().unwrap();
            assert_eq!(u.input_tokens, 100);
            assert_eq!(u.output_tokens, 50);
            assert_eq!(u.cache_creation_input_tokens, Some(10));
            assert!(u.cache_read_input_tokens.is_none());
        } else {
            panic!("expected AssistantMeta");
        }
    }

    #[test]
    fn system_meta_preserves_subtype() {
        let mut store = TurnStore::new();
        let builder = TurnBuilder {
            uuid: "sys-1".to_string(),
            parent_uuid: None,
            session_id: None,
            timestamp: "2026-03-29T10:00:00Z".to_string(),
            message_type: MessageType::System,
            cwd: None,
            git_branch: None,
            role: TurnRole::User,
            content: MessageContent::Text("error occurred".to_string()),
            meta: TurnMeta::System(SystemMeta {
                subtype: Some("error".to_string()),
                level: Some("warning".to_string()),
            }),
            status: TurnStatus::Complete,
            extra: json!({}),
        };
        let id = store.insert(agent(1), builder);

        let turn = store.get(id).unwrap();
        if let TurnMeta::System(ref meta) = turn.meta {
            assert_eq!(meta.subtype.as_deref(), Some("error"));
            assert_eq!(meta.level.as_deref(), Some("warning"));
        } else {
            panic!("expected SystemMeta");
        }
    }

    #[test]
    fn user_meta_preserves_flags() {
        let mut store = TurnStore::new();
        let mut builder = user_builder("u1");
        builder.meta = TurnMeta::User(UserMeta {
            is_meta: true,
            is_compact_summary: true,
            source_tool_assistant_uuid: Some("asst-uuid".to_string()),
        });
        let id = store.insert(agent(1), builder);

        let turn = store.get(id).unwrap();
        if let TurnMeta::User(ref meta) = turn.meta {
            assert!(meta.is_meta);
            assert!(meta.is_compact_summary);
            assert_eq!(
                meta.source_tool_assistant_uuid.as_deref(),
                Some("asst-uuid")
            );
        } else {
            panic!("expected UserMeta");
        }
    }

    // -- Token usage aggregation --

    #[test]
    fn agent_token_usage_sums_across_turns() {
        let mut store = TurnStore::new();
        let usage1 = Some(TokenUsage {
            input_tokens: 100,
            output_tokens: 50,
            cache_creation_input_tokens: Some(10),
            cache_read_input_tokens: None,
        });
        let usage2 = Some(TokenUsage {
            input_tokens: 200,
            output_tokens: 80,
            cache_creation_input_tokens: None,
            cache_read_input_tokens: Some(30),
        });
        store.insert(agent(1), assistant_builder("a1", usage1));
        store.insert(agent(1), assistant_builder("a2", usage2));

        let total = store.agent_token_usage(agent(1));
        assert_eq!(total.input_tokens, 300);
        assert_eq!(total.output_tokens, 130);
        assert_eq!(total.cache_creation_input_tokens, Some(10));
        assert_eq!(total.cache_read_input_tokens, Some(30));
    }

    #[test]
    fn agent_token_usage_ignores_user_turns() {
        let mut store = TurnStore::new();
        store.insert(agent(1), user_builder("u1"));
        store.insert(
            agent(1),
            assistant_builder(
                "a1",
                Some(TokenUsage {
                    input_tokens: 100,
                    output_tokens: 50,
                    cache_creation_input_tokens: None,
                    cache_read_input_tokens: None,
                }),
            ),
        );

        let total = store.agent_token_usage(agent(1));
        assert_eq!(total.input_tokens, 100);
        assert_eq!(total.output_tokens, 50);
    }

    #[test]
    fn agent_token_usage_handles_no_turns() {
        let store = TurnStore::new();
        let total = store.agent_token_usage(agent(99));
        assert_eq!(total.input_tokens, 0);
        assert_eq!(total.output_tokens, 0);
        assert!(total.cache_creation_input_tokens.is_none());
        assert!(total.cache_read_input_tokens.is_none());
    }
}
