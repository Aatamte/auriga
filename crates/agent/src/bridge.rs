use orchestrator_core::{
    AssistantMeta, MessageContent, MessageType, TurnBuilder, TurnMeta, TurnRole, TurnStatus,
    UserMeta,
};

use crate::message::{GenerateResponse, Message, Role};
use crate::session::SessionId;

/// Convert a user Message into a TurnBuilder for the core observation system.
pub fn user_message_to_turn(
    message: &Message,
    session_id: &SessionId,
    timestamp: &str,
) -> TurnBuilder {
    TurnBuilder {
        uuid: uuid::Uuid::new_v4().to_string(),
        parent_uuid: None,
        session_id: Some(session_id.0.to_string()),
        timestamp: timestamp.to_string(),
        message_type: match message.role {
            Role::User => MessageType::User,
            Role::Assistant => MessageType::Assistant,
        },
        cwd: None,
        git_branch: None,
        role: role_to_turn_role(message.role),
        content: message.content.clone(),
        meta: TurnMeta::User(UserMeta {
            is_meta: false,
            is_compact_summary: false,
            source_tool_assistant_uuid: None,
        }),
        status: TurnStatus::Complete,
        extra: serde_json::Value::Null,
    }
}

/// Convert a GenerateResponse into a TurnBuilder for the core observation system.
pub fn response_to_turn(
    response: &GenerateResponse,
    session_id: &SessionId,
    timestamp: &str,
) -> TurnBuilder {
    TurnBuilder {
        uuid: uuid::Uuid::new_v4().to_string(),
        parent_uuid: None,
        session_id: Some(session_id.0.to_string()),
        timestamp: timestamp.to_string(),
        message_type: MessageType::Assistant,
        cwd: None,
        git_branch: None,
        role: TurnRole::Assistant,
        content: MessageContent::Blocks(response.content.clone()),
        meta: TurnMeta::Assistant(AssistantMeta {
            model: Some(response.model.clone()),
            stop_reason: Some(response.stop_reason),
            stop_sequence: None,
            usage: Some(response.usage.clone()),
            request_id: response.request_id.clone(),
        }),
        status: TurnStatus::Complete,
        extra: serde_json::Value::Null,
    }
}

/// Convert an agent Role to core TurnRole.
pub fn role_to_turn_role(role: Role) -> TurnRole {
    match role {
        Role::User => TurnRole::User,
        Role::Assistant => TurnRole::Assistant,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use orchestrator_core::{ContentBlock, StopReason, TokenUsage};

    #[test]
    fn user_message_to_turn_produces_correct_fields() {
        let msg = Message {
            role: Role::User,
            content: MessageContent::Text("Hello".into()),
        };
        let session_id = SessionId::from_u128(1);
        let turn = user_message_to_turn(&msg, &session_id, "2026-01-01T00:00:00Z");

        assert_eq!(turn.role, TurnRole::User);
        assert_eq!(turn.message_type, MessageType::User);
        assert_eq!(turn.timestamp, "2026-01-01T00:00:00Z");
        assert!(turn.session_id.is_some());
        assert_eq!(turn.status, TurnStatus::Complete);
        assert!(matches!(turn.content, MessageContent::Text(ref t) if t == "Hello"));
        assert!(matches!(turn.meta, TurnMeta::User(_)));
    }

    #[test]
    fn response_to_turn_maps_model_and_usage() {
        let response = GenerateResponse {
            content: vec![ContentBlock::Text {
                text: "Hi there".into(),
            }],
            model: "claude-opus-4-6".into(),
            stop_reason: StopReason::EndTurn,
            usage: TokenUsage {
                input_tokens: 500,
                output_tokens: 200,
                cache_creation_input_tokens: Some(10),
                cache_read_input_tokens: None,
            },
            request_id: Some("req-abc".into()),
            provider_session_id: None,
        };
        let session_id = SessionId::from_u128(2);
        let turn = response_to_turn(&response, &session_id, "2026-01-01T00:00:01Z");

        assert_eq!(turn.role, TurnRole::Assistant);
        assert_eq!(turn.message_type, MessageType::Assistant);

        if let TurnMeta::Assistant(ref meta) = turn.meta {
            assert_eq!(meta.model.as_deref(), Some("claude-opus-4-6"));
            assert_eq!(meta.stop_reason, Some(StopReason::EndTurn));
            assert_eq!(meta.request_id.as_deref(), Some("req-abc"));
            let usage = meta.usage.as_ref().unwrap();
            assert_eq!(usage.input_tokens, 500);
            assert_eq!(usage.output_tokens, 200);
            assert_eq!(usage.cache_creation_input_tokens, Some(10));
        } else {
            panic!("expected AssistantMeta");
        }
    }

    #[test]
    fn role_to_turn_role_maps_correctly() {
        assert_eq!(role_to_turn_role(Role::User), TurnRole::User);
        assert_eq!(role_to_turn_role(Role::Assistant), TurnRole::Assistant);
    }
}
