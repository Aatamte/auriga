use anyhow::Result;
use orchestrator_core::{
    AssistantMeta, ContentBlock, ImageSource, ImageSourceType, MessageContent, MessageType,
    StopReason, SystemMeta, TokenUsage, ToolResultContent, TurnBuilder, TurnMeta, TurnRole,
    TurnStatus, UserMeta,
};
use serde_json::Value;
use std::path::Path;

use crate::types::{ClaudeLogEntry, SessionInfo};

/// Parse one JSONL line into a ClaudeLogEntry.
pub fn parse_log_line(line: &str) -> Result<ClaudeLogEntry> {
    let v: Value = serde_json::from_str(line)?;

    let entry_type = v
        .get("type")
        .and_then(|t| t.as_str())
        .unwrap_or("")
        .to_string();

    let uuid = v
        .get("uuid")
        .and_then(|u| u.as_str())
        .unwrap_or("")
        .to_string();

    let parent_uuid = v
        .get("parentUuid")
        .and_then(|u| u.as_str())
        .map(|s| s.to_string());

    let session_id = v
        .get("sessionId")
        .and_then(|s| s.as_str())
        .unwrap_or("")
        .to_string();

    let timestamp = v
        .get("timestamp")
        .and_then(|t| t.as_str())
        .unwrap_or("")
        .to_string();

    let cwd = v.get("cwd").and_then(|c| c.as_str()).map(|s| s.to_string());

    let git_branch = v
        .get("gitBranch")
        .and_then(|g| g.as_str())
        .map(|s| s.to_string());

    let message = v.get("message").cloned();

    let is_meta = v.get("isMeta").and_then(|m| m.as_bool()).unwrap_or(false);

    Ok(ClaudeLogEntry {
        uuid,
        parent_uuid,
        entry_type,
        session_id,
        timestamp,
        cwd,
        git_branch,
        message,
        is_meta,
        extra: v,
    })
}

/// Parse a Claude session metadata file.
pub fn parse_session_file(path: &Path) -> Result<SessionInfo> {
    let contents = std::fs::read_to_string(path)?;
    let info: SessionInfo = serde_json::from_str(&contents)?;
    Ok(info)
}

/// Convert a ClaudeLogEntry into a TurnBuilder. Returns None for non-message entries.
pub fn to_turn_builder(entry: &ClaudeLogEntry) -> Option<TurnBuilder> {
    match entry.entry_type.as_str() {
        "user" | "assistant" | "system" => {}
        _ => return None, // skip file-history-snapshot, last-prompt, etc.
    }

    let message_type = match entry.entry_type.as_str() {
        "assistant" => MessageType::Assistant,
        "system" => MessageType::System,
        _ => MessageType::User,
    };

    let role = match entry.entry_type.as_str() {
        "assistant" => TurnRole::Assistant,
        _ => TurnRole::User,
    };

    let content = parse_content(entry.message.as_ref());

    let meta = match message_type {
        MessageType::Assistant => TurnMeta::Assistant(parse_assistant_meta(entry.message.as_ref())),
        MessageType::System => TurnMeta::System(SystemMeta {
            subtype: None,
            level: None,
        }),
        MessageType::User => TurnMeta::User(UserMeta {
            is_meta: entry.is_meta,
            is_compact_summary: false,
            source_tool_assistant_uuid: entry
                .extra
                .get("sourceToolAssistantUUID")
                .and_then(|s| s.as_str())
                .map(|s| s.to_string()),
        }),
    };

    Some(TurnBuilder {
        uuid: entry.uuid.clone(),
        parent_uuid: entry.parent_uuid.clone(),
        session_id: Some(entry.session_id.clone()),
        timestamp: entry.timestamp.clone(),
        message_type,
        cwd: entry.cwd.clone(),
        git_branch: entry.git_branch.clone(),
        role,
        content,
        meta,
        status: TurnStatus::Complete,
        extra: strip_known_fields(&entry.extra),
    })
}

fn parse_content(message: Option<&Value>) -> MessageContent {
    let Some(msg) = message else {
        return MessageContent::Text(String::new());
    };

    let content = msg.get("content");

    match content {
        Some(Value::String(s)) => MessageContent::Text(s.clone()),
        Some(Value::Array(arr)) => {
            let blocks: Vec<ContentBlock> = arr.iter().filter_map(parse_content_block).collect();
            if blocks.is_empty() {
                MessageContent::Text(String::new())
            } else {
                MessageContent::Blocks(blocks)
            }
        }
        _ => MessageContent::Text(String::new()),
    }
}

fn parse_content_block(v: &Value) -> Option<ContentBlock> {
    let block_type = v.get("type")?.as_str()?;
    match block_type {
        "text" => {
            let text = v.get("text")?.as_str()?.to_string();
            Some(ContentBlock::Text { text })
        }
        "thinking" => {
            let thinking = v.get("thinking")?.as_str()?.to_string();
            let signature = v
                .get("signature")
                .and_then(|s| s.as_str())
                .map(|s| s.to_string());
            Some(ContentBlock::Thinking {
                thinking,
                signature,
            })
        }
        "tool_use" => {
            let id = v.get("id")?.as_str()?.to_string();
            let name = v.get("name")?.as_str()?.to_string();
            let input = v.get("input").cloned().unwrap_or(Value::Null);
            Some(ContentBlock::ToolUse { id, name, input })
        }
        "tool_result" => {
            let tool_use_id = v.get("tool_use_id")?.as_str()?.to_string();
            let is_error = v.get("is_error").and_then(|e| e.as_bool()).unwrap_or(false);
            let content = match v.get("content") {
                Some(Value::String(s)) => ToolResultContent::Text(s.clone()),
                Some(Value::Array(arr)) => {
                    let blocks: Vec<ContentBlock> =
                        arr.iter().filter_map(parse_content_block).collect();
                    ToolResultContent::Blocks(blocks)
                }
                _ => ToolResultContent::Text(String::new()),
            };
            Some(ContentBlock::ToolResult {
                tool_use_id,
                content,
                is_error,
            })
        }
        "image" => {
            let source = v.get("source")?;
            let media_type = source.get("media_type")?.as_str()?.to_string();
            let data = source.get("data")?.as_str()?.to_string();
            Some(ContentBlock::Image {
                source: ImageSource {
                    source_type: ImageSourceType::Base64,
                    media_type,
                    data,
                },
            })
        }
        _ => None, // skip unknown block types (tool_reference, etc.)
    }
}

fn parse_assistant_meta(message: Option<&Value>) -> AssistantMeta {
    let Some(msg) = message else {
        return AssistantMeta {
            model: None,
            stop_reason: None,
            stop_sequence: None,
            usage: None,
            request_id: None,
        };
    };

    let model = msg
        .get("model")
        .and_then(|m| m.as_str())
        .map(|s| s.to_string());

    let stop_reason = msg
        .get("stop_reason")
        .and_then(|s| s.as_str())
        .and_then(|s| match s {
            "end_turn" => Some(StopReason::EndTurn),
            "tool_use" => Some(StopReason::ToolUse),
            "max_tokens" => Some(StopReason::MaxTokens),
            "stop_sequence" => Some(StopReason::StopSequence),
            _ => None,
        });

    let stop_sequence = msg
        .get("stop_sequence")
        .and_then(|s| s.as_str())
        .map(|s| s.to_string());

    let usage = msg.get("usage").map(|u| TokenUsage {
        input_tokens: u.get("input_tokens").and_then(|n| n.as_u64()).unwrap_or(0),
        output_tokens: u.get("output_tokens").and_then(|n| n.as_u64()).unwrap_or(0),
        cache_creation_input_tokens: u
            .get("cache_creation_input_tokens")
            .and_then(|n| n.as_u64()),
        cache_read_input_tokens: u.get("cache_read_input_tokens").and_then(|n| n.as_u64()),
    });

    AssistantMeta {
        model,
        stop_reason,
        stop_sequence,
        usage,
        request_id: None,
    }
}

/// Strip fields that are already represented in TurnBuilder to avoid duplication in `extra`.
fn strip_known_fields(v: &Value) -> Value {
    let Some(obj) = v.as_object() else {
        return Value::Null;
    };
    let mut filtered = serde_json::Map::new();
    for (k, v) in obj {
        match k.as_str() {
            "uuid" | "parentUuid" | "type" | "sessionId" | "timestamp" | "cwd" | "gitBranch"
            | "message" | "isMeta" => continue,
            _ => {
                filtered.insert(k.clone(), v.clone());
            }
        }
    }
    Value::Object(filtered)
}

#[cfg(test)]
mod tests {
    use super::*;

    const USER_LINE: &str = r#"{"parentUuid":null,"type":"user","message":{"role":"user","content":"hello world"},"uuid":"u1","timestamp":"2026-03-31T10:00:00Z","sessionId":"sess-1","cwd":"/home/user","gitBranch":"main"}"#;

    const ASSISTANT_LINE: &str = r#"{"parentUuid":"u1","type":"assistant","message":{"role":"assistant","model":"claude-opus-4-6","content":[{"type":"text","text":"Hi there!"}],"stop_reason":"end_turn","usage":{"input_tokens":100,"output_tokens":50,"cache_creation_input_tokens":10}},"uuid":"a1","timestamp":"2026-03-31T10:00:01Z","sessionId":"sess-1","requestId":"req-1"}"#;

    const SNAPSHOT_LINE: &str =
        r#"{"type":"file-history-snapshot","messageId":"u1","snapshot":{}}"#;

    #[test]
    fn parse_user_entry() {
        let entry = parse_log_line(USER_LINE).unwrap();
        assert_eq!(entry.uuid, "u1");
        assert_eq!(entry.entry_type, "user");
        assert_eq!(entry.session_id, "sess-1");
        assert_eq!(entry.timestamp, "2026-03-31T10:00:00Z");
        assert_eq!(entry.cwd.as_deref(), Some("/home/user"));
        assert_eq!(entry.git_branch.as_deref(), Some("main"));
        assert!(entry.parent_uuid.is_none());
    }

    #[test]
    fn parse_assistant_entry() {
        let entry = parse_log_line(ASSISTANT_LINE).unwrap();
        assert_eq!(entry.uuid, "a1");
        assert_eq!(entry.entry_type, "assistant");
        assert_eq!(entry.parent_uuid.as_deref(), Some("u1"));
    }

    #[test]
    fn snapshot_skipped_by_to_turn_builder() {
        let entry = parse_log_line(SNAPSHOT_LINE).unwrap();
        assert!(to_turn_builder(&entry).is_none());
    }

    #[test]
    fn user_entry_to_turn_builder() {
        let entry = parse_log_line(USER_LINE).unwrap();
        let builder = to_turn_builder(&entry).unwrap();
        assert_eq!(builder.uuid, "u1");
        assert_eq!(builder.message_type, MessageType::User);
        assert_eq!(builder.role, TurnRole::User);
        assert_eq!(builder.cwd.as_deref(), Some("/home/user"));
        assert_eq!(builder.git_branch.as_deref(), Some("main"));
        assert_eq!(builder.session_id.as_deref(), Some("sess-1"));
        if let MessageContent::Text(ref text) = builder.content {
            assert_eq!(text, "hello world");
        } else {
            panic!("expected Text content");
        }
    }

    #[test]
    fn assistant_entry_to_turn_builder() {
        let entry = parse_log_line(ASSISTANT_LINE).unwrap();
        let builder = to_turn_builder(&entry).unwrap();
        assert_eq!(builder.message_type, MessageType::Assistant);
        assert_eq!(builder.role, TurnRole::Assistant);

        if let TurnMeta::Assistant(ref meta) = builder.meta {
            assert_eq!(meta.model.as_deref(), Some("claude-opus-4-6"));
            assert_eq!(meta.stop_reason, Some(StopReason::EndTurn));
            let usage = meta.usage.as_ref().unwrap();
            assert_eq!(usage.input_tokens, 100);
            assert_eq!(usage.output_tokens, 50);
            assert_eq!(usage.cache_creation_input_tokens, Some(10));
        } else {
            panic!("expected AssistantMeta");
        }

        if let MessageContent::Blocks(ref blocks) = builder.content {
            assert_eq!(blocks.len(), 1);
            if let ContentBlock::Text { ref text } = blocks[0] {
                assert_eq!(text, "Hi there!");
            } else {
                panic!("expected Text block");
            }
        } else {
            panic!("expected Blocks content");
        }
    }

    #[test]
    fn tool_use_content_parsed() {
        let line = r#"{"type":"assistant","message":{"role":"assistant","content":[{"type":"tool_use","id":"toolu_01","name":"Bash","input":{"command":"ls"}}],"stop_reason":"tool_use","usage":{"input_tokens":10,"output_tokens":5}},"uuid":"a2","timestamp":"2026-03-31T10:00:02Z","sessionId":"sess-1"}"#;
        let entry = parse_log_line(line).unwrap();
        let builder = to_turn_builder(&entry).unwrap();

        if let MessageContent::Blocks(ref blocks) = builder.content {
            if let ContentBlock::ToolUse {
                ref id, ref name, ..
            } = blocks[0]
            {
                assert_eq!(id, "toolu_01");
                assert_eq!(name, "Bash");
            } else {
                panic!("expected ToolUse block");
            }
        } else {
            panic!("expected Blocks content");
        }
    }

    #[test]
    fn extra_preserves_unknown_fields() {
        let entry = parse_log_line(USER_LINE).unwrap();
        let builder = to_turn_builder(&entry).unwrap();
        // Known fields should be stripped from extra
        assert!(builder.extra.get("uuid").is_none());
        assert!(builder.extra.get("message").is_none());
    }

    #[test]
    fn parse_session_file_works() {
        let dir = std::env::temp_dir();
        let path = dir.join("test_session_12345.json");
        std::fs::write(
            &path,
            r#"{"pid":12345,"sessionId":"sess-abc","cwd":"/home/user","startedAt":1700000000000}"#,
        )
        .unwrap();
        let info = parse_session_file(&path).unwrap();
        assert_eq!(info.pid, 12345);
        assert_eq!(info.session_id, "sess-abc");
        assert_eq!(info.cwd, "/home/user");
        let _ = std::fs::remove_file(&path);
    }
}
