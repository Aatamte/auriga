use orchestrator_core::{ContentBlock, MessageContent, MessageType, Trace, Turn};
use std::collections::HashSet;

pub const FEATURE_NAMES: &[&str] = &[
    "turn_count",
    "input_tokens",
    "output_tokens",
    "total_tokens",
    "duration_secs",
    "assistant_turn_count",
    "user_turn_count",
    "tool_use_count",
    "tool_error_count",
    "thinking_block_count",
    "unique_tool_count",
    "avg_output_per_assistant",
    "error_rate",
    "text_length_total",
];

pub fn feature_count() -> usize {
    FEATURE_NAMES.len()
}

/// Extract a fixed-length feature vector from a trace and its turns.
/// Order matches `FEATURE_NAMES`.
pub fn extract_features(trace: &Trace, turns: &[Turn]) -> Vec<f64> {
    let turn_count = trace.turn_count as f64;
    let input_tokens = trace.token_usage.input_tokens as f64;
    let output_tokens = trace.token_usage.output_tokens as f64;
    let total_tokens = input_tokens + output_tokens;

    let duration_secs = parse_duration(trace);

    let mut assistant_turns = 0u64;
    let mut user_turns = 0u64;
    let mut tool_use_count = 0u64;
    let mut tool_error_count = 0u64;
    let mut thinking_block_count = 0u64;
    let mut text_length_total = 0u64;
    let mut tool_names: HashSet<String> = HashSet::new();

    for turn in turns {
        match turn.message_type {
            MessageType::Assistant => assistant_turns += 1,
            MessageType::User => user_turns += 1,
            MessageType::System => {}
        }

        let blocks = match &turn.content {
            MessageContent::Text(t) => {
                text_length_total += t.len() as u64;
                continue;
            }
            MessageContent::Blocks(blocks) => blocks,
        };

        for block in blocks {
            match block {
                ContentBlock::Text { text } => {
                    text_length_total += text.len() as u64;
                }
                ContentBlock::Thinking { thinking, .. } => {
                    thinking_block_count += 1;
                    text_length_total += thinking.len() as u64;
                }
                ContentBlock::ToolUse { name, .. } => {
                    tool_use_count += 1;
                    tool_names.insert(name.clone());
                }
                ContentBlock::ToolResult { is_error, .. } => {
                    if *is_error {
                        tool_error_count += 1;
                    }
                }
                ContentBlock::Image { .. } => {}
            }
        }
    }

    let unique_tool_count = tool_names.len() as f64;
    let avg_output_per_assistant = if assistant_turns > 0 {
        output_tokens / assistant_turns as f64
    } else {
        0.0
    };
    let error_rate = if tool_use_count > 0 {
        tool_error_count as f64 / tool_use_count as f64
    } else {
        0.0
    };

    vec![
        turn_count,
        input_tokens,
        output_tokens,
        total_tokens,
        duration_secs,
        assistant_turns as f64,
        user_turns as f64,
        tool_use_count as f64,
        tool_error_count as f64,
        thinking_block_count as f64,
        unique_tool_count,
        avg_output_per_assistant,
        error_rate,
        text_length_total as f64,
    ]
}

fn parse_duration(trace: &Trace) -> f64 {
    // Simple ISO 8601 duration: parse start and end timestamps
    let Some(ref end) = trace.completed_at else {
        return 0.0;
    };
    let start = &trace.started_at;

    // Parse "YYYY-MM-DDTHH:MM:SSZ" to seconds since epoch (rough)
    let parse_ts = |s: &str| -> Option<f64> {
        // Minimal parser for ISO 8601 timestamps
        let s = s.trim_end_matches('Z');
        let parts: Vec<&str> = s.split('T').collect();
        if parts.len() != 2 {
            return None;
        }
        let date_parts: Vec<u64> = parts[0].split('-').filter_map(|p| p.parse().ok()).collect();
        let time_parts: Vec<f64> = parts[1].split(':').filter_map(|p| p.parse().ok()).collect();
        if date_parts.len() != 3 || time_parts.len() != 3 {
            return None;
        }
        // Approximate seconds since epoch (good enough for duration diff)
        let days = date_parts[0] * 365 + date_parts[1] * 30 + date_parts[2];
        let secs = time_parts[0] * 3600.0 + time_parts[1] * 60.0 + time_parts[2];
        Some(days as f64 * 86400.0 + secs)
    };

    match (parse_ts(start), parse_ts(end)) {
        (Some(s), Some(e)) => (e - s).max(0.0),
        _ => 0.0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use orchestrator_core::*;

    fn test_trace() -> Trace {
        Trace {
            id: TraceId::from_u128(1),
            agent_id: AgentId::from_u128(1),
            session_id: "s1".into(),
            status: TraceStatus::Complete,
            started_at: "2026-01-01T00:00:00Z".into(),
            completed_at: Some("2026-01-01T00:05:00Z".into()),
            turn_count: 4,
            token_usage: TokenUsage {
                input_tokens: 1000,
                output_tokens: 500,
                cache_creation_input_tokens: None,
                cache_read_input_tokens: None,
            },
            provider: "claude".into(),
            model: Some("claude-opus-4-6".into()),
        }
    }

    fn test_turns() -> Vec<Turn> {
        vec![
            Turn {
                id: TurnId(0),
                agent_id: AgentId::from_u128(1),
                number: 1,
                status: TurnStatus::Complete,
                uuid: "u1".into(),
                parent_uuid: None,
                session_id: Some("s1".into()),
                timestamp: "2026-01-01T00:00:00Z".into(),
                message_type: MessageType::User,
                cwd: None,
                git_branch: None,
                role: TurnRole::User,
                content: MessageContent::Text("Fix the bug".into()),
                meta: TurnMeta::User(UserMeta {
                    is_meta: false,
                    is_compact_summary: false,
                    source_tool_assistant_uuid: None,
                }),
                extra: serde_json::Value::Null,
            },
            Turn {
                id: TurnId(1),
                agent_id: AgentId::from_u128(1),
                number: 2,
                status: TurnStatus::Complete,
                uuid: "u2".into(),
                parent_uuid: Some("u1".into()),
                session_id: Some("s1".into()),
                timestamp: "2026-01-01T00:01:00Z".into(),
                message_type: MessageType::Assistant,
                cwd: None,
                git_branch: None,
                role: TurnRole::Assistant,
                content: MessageContent::Blocks(vec![
                    ContentBlock::Thinking {
                        thinking: "Let me analyze".into(),
                        signature: None,
                    },
                    ContentBlock::Text {
                        text: "I'll fix it".into(),
                    },
                    ContentBlock::ToolUse {
                        id: "t1".into(),
                        name: "read_file".into(),
                        input: serde_json::json!({"path": "main.rs"}),
                    },
                ]),
                meta: TurnMeta::Assistant(AssistantMeta {
                    model: Some("claude-opus-4-6".into()),
                    stop_reason: Some(StopReason::ToolUse),
                    stop_sequence: None,
                    usage: Some(TokenUsage {
                        input_tokens: 500,
                        output_tokens: 250,
                        cache_creation_input_tokens: None,
                        cache_read_input_tokens: None,
                    }),
                    request_id: None,
                }),
                extra: serde_json::Value::Null,
            },
            Turn {
                id: TurnId(2),
                agent_id: AgentId::from_u128(1),
                number: 3,
                status: TurnStatus::Complete,
                uuid: "u3".into(),
                parent_uuid: Some("u2".into()),
                session_id: Some("s1".into()),
                timestamp: "2026-01-01T00:02:00Z".into(),
                message_type: MessageType::User,
                cwd: None,
                git_branch: None,
                role: TurnRole::User,
                content: MessageContent::Blocks(vec![ContentBlock::ToolResult {
                    tool_use_id: "t1".into(),
                    content: ToolResultContent::Text("file contents".into()),
                    is_error: false,
                }]),
                meta: TurnMeta::User(UserMeta {
                    is_meta: false,
                    is_compact_summary: false,
                    source_tool_assistant_uuid: None,
                }),
                extra: serde_json::Value::Null,
            },
            Turn {
                id: TurnId(3),
                agent_id: AgentId::from_u128(1),
                number: 4,
                status: TurnStatus::Complete,
                uuid: "u4".into(),
                parent_uuid: Some("u3".into()),
                session_id: Some("s1".into()),
                timestamp: "2026-01-01T00:03:00Z".into(),
                message_type: MessageType::Assistant,
                cwd: None,
                git_branch: None,
                role: TurnRole::Assistant,
                content: MessageContent::Blocks(vec![ContentBlock::ToolUse {
                    id: "t2".into(),
                    name: "edit_file".into(),
                    input: serde_json::json!({"path": "main.rs"}),
                }]),
                meta: TurnMeta::Assistant(AssistantMeta {
                    model: Some("claude-opus-4-6".into()),
                    stop_reason: Some(StopReason::EndTurn),
                    stop_sequence: None,
                    usage: Some(TokenUsage {
                        input_tokens: 500,
                        output_tokens: 250,
                        cache_creation_input_tokens: None,
                        cache_read_input_tokens: None,
                    }),
                    request_id: None,
                }),
                extra: serde_json::Value::Null,
            },
        ]
    }

    #[test]
    fn feature_count_matches_names() {
        assert_eq!(feature_count(), FEATURE_NAMES.len());
    }

    #[test]
    fn extract_features_correct_length() {
        let features = extract_features(&test_trace(), &test_turns());
        assert_eq!(features.len(), feature_count());
    }

    #[test]
    fn extract_features_values() {
        let features = extract_features(&test_trace(), &test_turns());

        assert_eq!(features[0], 4.0); // turn_count
        assert_eq!(features[1], 1000.0); // input_tokens
        assert_eq!(features[2], 500.0); // output_tokens
        assert_eq!(features[3], 1500.0); // total_tokens
        assert!(features[4] > 0.0); // duration_secs (5 minutes)
        assert_eq!(features[5], 2.0); // assistant_turn_count
        assert_eq!(features[6], 2.0); // user_turn_count
        assert_eq!(features[7], 2.0); // tool_use_count (read_file + edit_file)
        assert_eq!(features[8], 0.0); // tool_error_count
        assert_eq!(features[9], 1.0); // thinking_block_count
        assert_eq!(features[10], 2.0); // unique_tool_count
        assert_eq!(features[11], 250.0); // avg_output_per_assistant (500/2)
        assert_eq!(features[12], 0.0); // error_rate
                                       // text_length_total: "Fix the bug" (11) + "Let me analyze" (14) + "I'll fix it" (11) = 36
                                       // (ToolResult content is ToolResultContent, not counted as text blocks)
        assert_eq!(features[13], 36.0);
    }

    #[test]
    fn extract_features_empty_turns() {
        let trace = test_trace();
        let features = extract_features(&trace, &[]);
        assert_eq!(features.len(), feature_count());
        assert_eq!(features[5], 0.0); // assistant_turn_count
        assert_eq!(features[11], 0.0); // avg_output_per_assistant (no division by zero)
    }

    #[test]
    fn duration_parses_correctly() {
        let mut trace = test_trace();
        trace.started_at = "2026-01-01T10:00:00Z".into();
        trace.completed_at = Some("2026-01-01T10:05:30Z".into());
        let features = extract_features(&trace, &[]);
        assert!((features[4] - 330.0).abs() < 1.0); // 5min 30sec = 330s
    }
}
