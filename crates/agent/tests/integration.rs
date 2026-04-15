//! Integration tests for auriga-agent public API

use auriga_agent::{
    response_to_turn, role_to_turn_role, user_message_to_turn, AgentConfig, AgentMode,
    ContentBlock, GenerateResponse, Message, MessageContent, Role, Session, SessionId,
    SessionStatus, StopReason, TokenUsage, ToolDefinition, ToolOutput, TurnRole,
};

fn test_config() -> AgentConfig {
    AgentConfig {
        name: "test-agent".to_string(),
        provider: "test".to_string(),
        model: "test-model".to_string(),
        max_tokens: 1024,
        system_prompt: Some("Be helpful.".to_string()),
        temperature: None,
        mode: AgentMode::Managed,
        provider_config: serde_json::json!({}),
    }
}

fn text_response(text: &str) -> GenerateResponse {
    GenerateResponse {
        content: vec![ContentBlock::Text {
            text: text.to_string(),
        }],
        model: "test-model".to_string(),
        stop_reason: StopReason::EndTurn,
        usage: TokenUsage {
            input_tokens: 100,
            output_tokens: 50,
            cache_creation_input_tokens: None,
            cache_read_input_tokens: None,
        },
        request_id: None,
        provider_session_id: None,
    }
}

// -- Bridge function tests --

#[test]
fn role_to_turn_role_user() {
    let turn_role = role_to_turn_role(Role::User);
    assert_eq!(turn_role, TurnRole::User);
}

#[test]
fn role_to_turn_role_assistant() {
    let turn_role = role_to_turn_role(Role::Assistant);
    assert_eq!(turn_role, TurnRole::Assistant);
}

#[test]
fn user_message_to_turn_converts() {
    let msg = Message {
        role: Role::User,
        content: MessageContent::Text("Hello".to_string()),
    };
    let session_id = SessionId::new();
    let timestamp = "2026-01-01T00:00:00Z";

    let builder = user_message_to_turn(&msg, &session_id, timestamp);
    assert_eq!(builder.role, TurnRole::User);
}

#[test]
fn response_to_turn_converts() {
    let response = text_response("Hello");
    let session_id = SessionId::new();
    let timestamp = "2026-01-01T00:00:00Z";

    let builder = response_to_turn(&response, &session_id, timestamp);
    assert_eq!(builder.role, TurnRole::Assistant);
}

// -- Session tests --

#[test]
fn session_new_is_ready() {
    let session = Session::new(test_config(), vec![]);
    assert_eq!(session.status, SessionStatus::Ready);
    assert!(session.messages.is_empty());
    assert_eq!(session.turn_count, 0);
}

#[test]
fn session_send_message() {
    let mut session = Session::new(test_config(), vec![]);
    let req = session.send_message(MessageContent::Text("Hello".to_string()));

    assert!(req.is_some());
    assert_eq!(session.status, SessionStatus::Generating);
    assert_eq!(session.messages.len(), 1);
}

#[test]
fn session_receive_response() {
    let mut session = Session::new(test_config(), vec![]);
    session.send_message(MessageContent::Text("Hello".to_string()));

    let tool_calls = session.receive_response(text_response("Hi!"));
    assert!(tool_calls.is_empty());
    assert_eq!(session.status, SessionStatus::Ready);
    assert_eq!(session.turn_count, 1);
}

#[test]
fn session_tracks_token_usage() {
    let mut session = Session::new(test_config(), vec![]);
    session.send_message(MessageContent::Text("Hello".to_string()));
    session.receive_response(text_response("Hi!"));

    assert_eq!(session.total_usage.input_tokens, 100);
    assert_eq!(session.total_usage.output_tokens, 50);
}

#[test]
fn session_complete_and_abort() {
    let mut session = Session::new(test_config(), vec![]);
    session.complete();
    assert_eq!(session.status, SessionStatus::Complete);

    let mut session = Session::new(test_config(), vec![]);
    session.abort();
    assert_eq!(session.status, SessionStatus::Aborted);
}

// -- Message tests --

#[test]
fn message_creation() {
    let msg = Message {
        role: Role::User,
        content: MessageContent::Text("Test".to_string()),
    };

    assert_eq!(msg.role, Role::User);
}

// -- Role tests --

#[test]
fn role_variants() {
    let _ = Role::User;
    let _ = Role::Assistant;
}

// -- TokenUsage tests --

#[test]
fn token_usage_total() {
    let usage = TokenUsage {
        input_tokens: 100,
        output_tokens: 50,
        cache_creation_input_tokens: Some(10),
        cache_read_input_tokens: Some(5),
    };

    assert_eq!(usage.total(), 150);
}

// -- SessionStatus tests --

#[test]
fn session_status_variants() {
    let _ = SessionStatus::Ready;
    let _ = SessionStatus::Generating;
    let _ = SessionStatus::ToolPending;
    let _ = SessionStatus::Complete;
    let _ = SessionStatus::Aborted;
}

// -- AgentConfig tests --

#[test]
fn agent_config_structure() {
    let config = test_config();
    assert_eq!(config.name, "test-agent");
    assert_eq!(config.provider, "test");
    assert_eq!(config.model, "test-model");
}

// -- AgentMode tests --

#[test]
fn agent_mode_variants() {
    let _ = AgentMode::Generate;
    let _ = AgentMode::Managed;
}
