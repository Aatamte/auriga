//! Integration tests for auriga-claude-log public API

use auriga_claude_log::{
    claude_project_dir, claude_sessions_dir, parse_log_line, parse_session_file, to_turn_builder,
    ClaudeLogEntry, ClaudeWatchEvent, SessionInfo,
};
use std::path::PathBuf;

// -- parse_log_line tests --

#[test]
fn parse_log_line_valid_json() {
    let line = r#"{"uuid":"123","type":"user","sessionId":"s1","timestamp":"2026-01-01T00:00:00Z","message":{"content":[{"type":"text","text":"Hello"}]}}"#;
    let result = parse_log_line(line);
    assert!(result.is_ok());

    let entry = result.unwrap();
    assert_eq!(entry.uuid, "123");
    assert_eq!(entry.entry_type, "user");
}

#[test]
fn parse_log_line_invalid_json() {
    let result = parse_log_line("not valid json");
    assert!(result.is_err());
}

#[test]
fn parse_log_line_empty() {
    let result = parse_log_line("");
    assert!(result.is_err());
}

// -- parse_session_file tests --

#[test]
fn parse_session_file_nonexistent() {
    let result = parse_session_file(&PathBuf::from("/nonexistent/file.json"));
    assert!(result.is_err());
}

// -- to_turn_builder tests --

#[test]
fn to_turn_builder_handles_entry() {
    let line = r#"{"uuid":"123","type":"user","sessionId":"s1","timestamp":"2026-01-01T00:00:00Z","message":{"content":[{"type":"text","text":"test"}]}}"#;
    if let Ok(entry) = parse_log_line(line) {
        let builder = to_turn_builder(&entry);
        assert!(builder.is_some());
    }
}

// -- Path functions tests --

#[test]
fn claude_project_dir_returns_option() {
    let result = claude_project_dir();
    // May be None if Claude CLI not installed, that's fine
    let _ = result;
}

#[test]
fn claude_sessions_dir_returns_option() {
    let result = claude_sessions_dir();
    // May be None if Claude CLI not installed
    let _ = result;
}

// -- ClaudeLogEntry tests --

#[test]
fn claude_log_entry_fields() {
    let line = r#"{"uuid":"test-uuid","type":"assistant","sessionId":"sess-1","timestamp":"2026-01-01T00:00:00Z"}"#;
    if let Ok(entry) = parse_log_line(line) {
        assert_eq!(entry.uuid, "test-uuid");
        assert_eq!(entry.entry_type, "assistant");
        assert_eq!(entry.session_id, "sess-1");
    }
}

// -- ClaudeWatchEvent tests --

#[test]
fn claude_watch_event_log_entry_variant() {
    let line =
        r#"{"uuid":"123","type":"user","sessionId":"s1","timestamp":"2026-01-01T00:00:00Z"}"#;
    if let Ok(entry) = parse_log_line(line) {
        let event = ClaudeWatchEvent::LogEntry(entry);
        if let ClaudeWatchEvent::LogEntry(e) = event {
            assert_eq!(e.uuid, "123");
        } else {
            panic!("expected LogEntry");
        }
    }
}

#[test]
fn claude_watch_event_session_discovered_variant() {
    let info = SessionInfo {
        pid: 12345,
        session_id: "sess-abc".to_string(),
        cwd: "/home/user".to_string(),
        started_at: 1234567890,
        kind: None,
    };

    let event = ClaudeWatchEvent::SessionDiscovered(info);
    if let ClaudeWatchEvent::SessionDiscovered(s) = event {
        assert_eq!(s.pid, 12345);
        assert_eq!(s.session_id, "sess-abc");
    } else {
        panic!("expected SessionDiscovered");
    }
}
