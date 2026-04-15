//! Integration tests for auriga-types public API

use auriga_types::{
    Agent, AgentId, AgentStatus, ContentBlock, DisplayMode, FileEntry, FocusState, ImageSource,
    ImageSourceType, MessageContent, Page, Panel, ScrollDirection, StopReason, TokenUsage,
    ToolResultContent, TurnId, TurnRole, TurnStatus,
};
use std::path::PathBuf;

// -- AgentId tests --

#[test]
fn agent_id_unique() {
    let id1 = AgentId::new();
    let id2 = AgentId::new();
    assert_ne!(id1, id2);
}

#[test]
fn agent_id_from_u128() {
    let id = AgentId::from_u128(12345);
    assert_eq!(id, AgentId::from_u128(12345));
}

#[test]
fn agent_id_equality() {
    let id = AgentId::new();
    let id_clone = id;
    assert_eq!(id, id_clone);
}

// -- Agent tests --

#[test]
fn agent_new() {
    let id = AgentId::new();
    let agent = Agent::new(id, "test-agent".to_string(), "claude".to_string());

    assert_eq!(agent.id, id);
    assert_eq!(agent.name, "test-agent");
    assert_eq!(agent.provider, "claude");
    assert_eq!(agent.status, AgentStatus::Idle);
    assert!(agent.session_id.is_none());
}

#[test]
fn agent_status_variants() {
    let _ = AgentStatus::Idle;
    let _ = AgentStatus::Working;
}

#[test]
fn display_mode_variants() {
    let _ = DisplayMode::Native;
    let _ = DisplayMode::Provider;
}

// -- Turn types tests --

#[test]
fn turn_id_equality() {
    let id1 = TurnId(1);
    let id2 = TurnId(1);
    let id3 = TurnId(2);
    assert_eq!(id1, id2);
    assert_ne!(id1, id3);
}

#[test]
fn turn_role_variants() {
    let _ = TurnRole::User;
    let _ = TurnRole::Assistant;
}

#[test]
fn turn_status_variants() {
    let _ = TurnStatus::Active;
    let _ = TurnStatus::Complete;
}

#[test]
fn content_block_text() {
    let block = ContentBlock::Text {
        text: "hello".to_string(),
    };
    if let ContentBlock::Text { text } = block {
        assert_eq!(text, "hello");
    } else {
        panic!("expected Text block");
    }
}

#[test]
fn content_block_thinking() {
    let block = ContentBlock::Thinking {
        thinking: "hmm".to_string(),
        signature: Some("sig".to_string()),
    };
    if let ContentBlock::Thinking {
        thinking,
        signature,
    } = block
    {
        assert_eq!(thinking, "hmm");
        assert_eq!(signature, Some("sig".to_string()));
    } else {
        panic!("expected Thinking block");
    }
}

#[test]
fn content_block_tool_use() {
    let block = ContentBlock::ToolUse {
        id: "1".to_string(),
        name: "read".to_string(),
        input: serde_json::json!({"path": "/test"}),
    };
    if let ContentBlock::ToolUse { id, name, input } = block {
        assert_eq!(id, "1");
        assert_eq!(name, "read");
        assert_eq!(input["path"], "/test");
    } else {
        panic!("expected ToolUse block");
    }
}

#[test]
fn content_block_tool_result() {
    let block = ContentBlock::ToolResult {
        tool_use_id: "1".to_string(),
        content: ToolResultContent::Text("result".to_string()),
        is_error: false,
    };
    if let ContentBlock::ToolResult {
        tool_use_id,
        content,
        is_error,
    } = block
    {
        assert_eq!(tool_use_id, "1");
        assert!(!is_error);
        if let ToolResultContent::Text(t) = content {
            assert_eq!(t, "result");
        }
    } else {
        panic!("expected ToolResult block");
    }
}

#[test]
fn content_block_image() {
    let block = ContentBlock::Image {
        source: ImageSource {
            source_type: ImageSourceType::Base64,
            media_type: "image/png".to_string(),
            data: "iVBOR...".to_string(),
        },
    };
    if let ContentBlock::Image { source } = block {
        assert_eq!(source.source_type, ImageSourceType::Base64);
        assert_eq!(source.media_type, "image/png");
    } else {
        panic!("expected Image block");
    }
}

#[test]
fn message_content_text() {
    let content = MessageContent::Text("hello".to_string());
    if let MessageContent::Text(t) = content {
        assert_eq!(t, "hello");
    } else {
        panic!("expected Text");
    }
}

#[test]
fn message_content_blocks() {
    let blocks = vec![ContentBlock::Text {
        text: "hello".to_string(),
    }];
    let content = MessageContent::Blocks(blocks);
    if let MessageContent::Blocks(b) = content {
        assert_eq!(b.len(), 1);
    } else {
        panic!("expected Blocks");
    }
}

#[test]
fn token_usage_creation() {
    let usage = TokenUsage {
        input_tokens: 100,
        output_tokens: 50,
        cache_creation_input_tokens: Some(10),
        cache_read_input_tokens: None,
    };
    assert_eq!(usage.input_tokens, 100);
    assert_eq!(usage.output_tokens, 50);
    assert_eq!(usage.total(), 150);
}

#[test]
fn stop_reason_variants() {
    let _ = StopReason::EndTurn;
    let _ = StopReason::ToolUse;
    let _ = StopReason::MaxTokens;
    let _ = StopReason::StopSequence;
}

// -- FocusState tests --

#[test]
fn focus_state_default() {
    let focus = FocusState::new();
    assert!(focus.active_agent.is_none());
    assert_eq!(focus.panel, Panel::AgentPane);
    assert_eq!(focus.page, Page::Home);
}

#[test]
fn focus_state_set_agent() {
    let mut focus = FocusState::new();
    let id = AgentId::new();
    focus.set_active_agent(id);
    assert_eq!(focus.active_agent, Some(id));
}

#[test]
fn focus_state_clear_agent() {
    let mut focus = FocusState::new();
    let id = AgentId::new();
    focus.set_active_agent(id);
    focus.clear_active_agent();
    assert!(focus.active_agent.is_none());
}

#[test]
fn page_variants() {
    let _ = Page::Home;
    let _ = Page::Prompts;
    let _ = Page::Database;
    let _ = Page::Settings;
}

#[test]
fn page_all_constant() {
    assert!(!Page::ALL.is_empty());
    assert!(Page::ALL.contains(&Page::Home));
    assert!(Page::ALL.contains(&Page::Settings));
}

#[test]
fn page_label() {
    assert_eq!(Page::Home.label(), "Home");
    assert_eq!(Page::Settings.label(), "Settings");
}

#[test]
fn panel_variants() {
    let _ = Panel::AgentPane;
}

// -- ScrollDirection tests --

#[test]
fn scroll_direction_variants() {
    let _ = ScrollDirection::Up;
    let _ = ScrollDirection::Down;
}

// -- FileEntry tests --

#[test]
fn file_entry_file() {
    let entry = FileEntry::file(PathBuf::from("/test/file.rs"), 1);
    assert!(!entry.is_dir);
    assert_eq!(entry.depth, 1);
    assert_eq!(entry.display_name(), "file.rs");
}

#[test]
fn file_entry_dir() {
    let entry = FileEntry::dir(PathBuf::from("/test/src"), 0);
    assert!(entry.is_dir);
    assert!(entry.expanded);
    assert_eq!(entry.display_name(), "src");
}

#[test]
fn file_entry_touch() {
    let mut entry = FileEntry::file(PathBuf::from("/test/file.rs"), 0);
    assert!(entry.last_modified.is_none());
    assert_eq!(entry.modify_count, 0);

    let agent = AgentId::from_u128(1);
    entry.touch(Some(agent));

    assert!(entry.last_modified.is_some());
    assert_eq!(entry.modify_count, 1);
    assert_eq!(entry.modified_by, Some(agent));
}

#[test]
fn file_entry_set_diff() {
    let mut entry = FileEntry::file(PathBuf::from("/test/file.rs"), 0);
    entry.set_diff(10, 5);
    assert_eq!(entry.lines_added, 10);
    assert_eq!(entry.lines_removed, 5);
}

#[test]
fn file_entry_age_secs() {
    let entry = FileEntry::file(PathBuf::from("/test/file.rs"), 0);
    assert!(entry.age_secs().is_none());

    let mut entry2 = FileEntry::file(PathBuf::from("/test/file2.rs"), 0);
    entry2.touch(None);
    assert!(entry2.age_secs().is_some());
}
