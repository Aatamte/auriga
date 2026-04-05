# Traces and Turns — Technical Specification

## Trace

```rust
pub struct Trace {
    pub id: TraceId,                           // UUID
    pub agent_id: AgentId,                     // UUID
    pub session_id: String,                    // Claude log session identifier
    pub status: TraceStatus,                   // Active | Complete | Aborted
    pub started_at: String,                    // ISO 8601
    pub completed_at: Option<String>,
    pub turn_count: u32,
    pub token_usage: TokenUsage,
    pub provider: String,
    pub model: Option<String>,
}

pub struct TokenUsage {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_creation_input_tokens: Option<u64>,
    pub cache_read_input_tokens: Option<u64>,
}
```

## TraceStore Methods

```
create(agent_id, session_id, provider) → TraceId
get(id) → Option<&Trace>
get_mut(id) → Option<&mut Trace>
complete(id) → bool
abort(id) → bool
active_trace(agent_id) → Option<&Trace>
active_trace_mut(agent_id) → Option<&mut Trace>
traces_for(agent_id) → Vec<&Trace>
find_by_session(agent_id, session_id) → Option<&Trace>
take_finished() → Vec<Trace>       // drains Complete + Aborted
remove_agent_traces(agent_id)
count() → usize
```

## Turn

```rust
pub struct Turn {
    pub id: TurnId,                            // usize, sequential
    pub agent_id: AgentId,
    pub number: u32,                           // per-agent sequential
    pub status: TurnStatus,                    // Active | Complete
    pub uuid: String,                          // from Claude logs
    pub parent_uuid: Option<String>,
    pub session_id: Option<String>,
    pub timestamp: String,
    pub message_type: MessageType,             // User | Assistant | System
    pub role: TurnRole,                        // User | Assistant
    pub content: MessageContent,
    pub meta: TurnMeta,
    pub extra: serde_json::Value,
}
```

## Content Types

```rust
pub enum MessageContent {
    Text(String),
    Blocks(Vec<ContentBlock>),
}

pub enum ContentBlock {
    Text { text: String },
    Thinking { thinking: String, signature: Option<String> },
    ToolUse { id: String, name: String, input: serde_json::Value },
    ToolResult { tool_use_id: String, content: ToolResultContent, is_error: bool },
    Image { source_type: String, media_type: String, data: String },
}
```

## TurnMeta

```rust
pub enum TurnMeta {
    User(UserMeta),
    Assistant(AssistantMeta),
    System(SystemMeta),
}

pub struct AssistantMeta {
    pub model: Option<String>,
    pub stop_reason: Option<StopReason>,
    pub stop_sequence: Option<String>,
    pub usage: Option<TokenUsage>,
    pub request_id: Option<String>,
}
```

Token usage on traces is accumulated from `AssistantMeta.usage` on each assistant turn.

## TurnStore Methods

```
insert(agent_id, TurnBuilder) → TurnId
get(id) → Option<&Turn>
get_mut(id) → Option<&mut Turn>
complete(id) → bool
turns_for(agent_id) → Vec<&Turn>
find_by_uuid(agent_id, uuid) → Option<&Turn>
agent_token_usage(agent_id) → TokenUsage
agent_turn_count(agent_id) → usize
active_turn(agent_id) → Option<&Turn>
remove_agent_turns(agent_id)
count() → usize
```

## Session Mapping Logic

```
on new LogEntry:
  1. Look up entry.session_id in session_map → agent_id?
  2. If not found, look up entry.pid in pid_map → agent_id?
  3. If found via pid, add session_map[entry.session_id] = agent_id
  4. If neither found, skip entry
  5. Dedup: check find_by_uuid(agent_id, entry.uuid) — skip if exists
  6. Create TurnBuilder from LogEntry via to_turn_builder()
  7. Insert turn into TurnStore
  8. Ensure trace exists via find_by_session() or create new one
  9. Update trace: turn_count++, token_usage += assistant usage
```
