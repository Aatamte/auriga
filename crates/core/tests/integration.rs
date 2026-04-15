//! Integration tests for auriga-core public API

use auriga_core::{
    AgentId, AgentStore, FileEntry, FileTree, FocusState, ScrollDirection, Scrollable, TraceStore,
    TurnStore,
};
use std::path::PathBuf;

// -- AgentStore integration tests --

#[test]
fn agent_lifecycle() {
    let mut store = AgentStore::new();

    // Create multiple agents
    let claude1 = store.create("claude");
    let claude2 = store.create("claude");
    let codex = store.create("codex");

    assert_eq!(store.count(), 3);
    assert_eq!(store.ids().len(), 3);

    // Verify agents are retrievable
    assert!(store.get(claude1).is_some());
    assert!(store.get(claude2).is_some());
    assert!(store.get(codex).is_some());

    // Remove one agent
    assert!(store.remove(claude1));
    assert_eq!(store.count(), 2);
    assert!(store.get(claude1).is_none());

    // Others still exist
    assert!(store.get(claude2).is_some());
    assert!(store.get(codex).is_some());
}

#[test]
fn agent_mutation() {
    let mut store = AgentStore::new();
    let id = store.create("claude");

    // Mutate via get_mut
    if let Some(agent) = store.get_mut(id) {
        agent.session_id = Some("test-session".to_string());
    }

    // Verify mutation persisted
    let agent = store.get(id).unwrap();
    assert_eq!(agent.session_id, Some("test-session".to_string()));
}

// -- FileTree integration tests --

#[test]
fn file_tree_workflow() {
    let mut tree = FileTree::new(PathBuf::from("/project"));

    // Set initial structure
    tree.set_entries(vec![
        FileEntry::dir(PathBuf::from("/project/src"), 0),
        FileEntry::file(PathBuf::from("/project/src/main.rs"), 1),
        FileEntry::file(PathBuf::from("/project/src/lib.rs"), 1),
        FileEntry::dir(PathBuf::from("/project/tests"), 0),
        FileEntry::file(PathBuf::from("/project/Cargo.toml"), 0),
    ]);

    tree.refresh_caches();
    assert_eq!(tree.visible_count(), 5);

    // Collapse src directory
    tree.toggle_dir(0);
    tree.refresh_caches();
    assert_eq!(tree.visible_count(), 3); // src, tests, Cargo.toml

    // Record activity on a file
    let agent_id = AgentId::from_u128(1);
    tree.record_activity(&PathBuf::from("/project/src/main.rs"), Some(agent_id));
    tree.refresh_caches();

    // Check recent activity
    let recent = tree.recent_activity(10);
    assert!(!recent.is_empty());
    assert_eq!(recent[0].display_name(), "main.rs");
}

#[test]
fn file_tree_dynamic_insertion() {
    let mut tree = FileTree::new(PathBuf::from("/project"));
    tree.set_entries(vec![FileEntry::dir(PathBuf::from("/project/src"), 0)]);

    // Insert new files via record_activity
    for i in 0..10 {
        tree.record_activity(&PathBuf::from(format!("/project/src/file_{}.rs", i)), None);
    }

    tree.refresh_caches();
    assert_eq!(tree.count(), 11); // 1 dir + 10 files
}

// -- Scrollable integration tests --

#[test]
fn scrollable_navigation() {
    let mut scroll = Scrollable::new();
    scroll.set_item_count(100);
    scroll.set_visible_height(20);

    assert_eq!(scroll.offset, 0);

    // Scroll down
    for _ in 0..25 {
        scroll.scroll(ScrollDirection::Down);
    }

    assert_eq!(scroll.offset, 25);

    // Scroll back up
    for _ in 0..25 {
        scroll.scroll(ScrollDirection::Up);
    }

    assert_eq!(scroll.offset, 0);
}

#[test]
fn scrollable_selection() {
    let mut scroll = Scrollable::new();
    scroll.set_item_count(100);
    scroll.set_visible_height(20);

    assert!(scroll.selected.is_none());

    scroll.select(5);
    assert_eq!(scroll.selected, Some(5));

    scroll.select_next();
    assert_eq!(scroll.selected, Some(6));

    scroll.select_prev();
    assert_eq!(scroll.selected, Some(5));
}

// -- FocusState integration tests --

#[test]
fn focus_state_agent_tracking() {
    let mut store = AgentStore::new();
    let agent1 = store.create("claude");
    let agent2 = store.create("claude");

    let mut focus = FocusState::new();
    assert!(focus.active_agent.is_none());

    focus.set_active_agent(agent1);
    assert_eq!(focus.active_agent, Some(agent1));

    focus.set_active_agent(agent2);
    assert_eq!(focus.active_agent, Some(agent2));
}

// -- TurnStore integration tests --

#[test]
fn turn_store_empty_initially() {
    let store = TurnStore::new();
    assert_eq!(store.count(), 0);
}

#[test]
fn turn_store_agent_turn_count() {
    let store = TurnStore::new();
    let agent = AgentId::from_u128(1);
    assert_eq!(store.agent_turn_count(agent), 0);
}

// -- TraceStore integration tests --

#[test]
fn trace_store_create_and_get() {
    let mut traces = TraceStore::new();
    let agent = AgentId::from_u128(1);

    let trace_id = traces.create(
        agent,
        "session-1".to_string(),
        "claude".to_string(),
        "2026-01-01T00:00:00Z".to_string(),
    );

    assert!(traces.get(trace_id).is_some());
    let trace = traces.get(trace_id).unwrap();
    assert_eq!(trace.agent_id, agent);
}

#[test]
fn trace_store_active_trace() {
    let mut traces = TraceStore::new();
    let agent = AgentId::from_u128(1);

    traces.create(
        agent,
        "session-1".to_string(),
        "claude".to_string(),
        "2026-01-01T00:00:00Z".to_string(),
    );

    let active = traces.active_trace(agent);
    assert!(active.is_some());
}

#[test]
fn trace_store_complete() {
    let mut traces = TraceStore::new();
    let agent = AgentId::from_u128(1);

    let trace_id = traces.create(
        agent,
        "session-1".to_string(),
        "claude".to_string(),
        "2026-01-01T00:00:00Z".to_string(),
    );

    assert!(traces.complete(trace_id, "2026-01-01T00:05:00Z".to_string()));

    // No longer active
    assert!(traces.active_trace(agent).is_none());
}

// -- Cross-component integration --

#[test]
fn full_agent_session_workflow() {
    // Simulates a realistic agent workflow
    let mut agents = AgentStore::new();
    let turns = TurnStore::new();
    let mut traces = TraceStore::new();
    let mut focus = FocusState::new();
    let mut tree = FileTree::new(PathBuf::from("/project"));

    // Create agent and set focus
    let agent = agents.create("claude");
    focus.set_active_agent(agent);

    // Create a trace for the agent
    let _trace_id = traces.create(
        agent,
        "session-1".to_string(),
        "claude".to_string(),
        "2026-01-01T00:00:00Z".to_string(),
    );

    tree.set_entries(vec![
        FileEntry::dir(PathBuf::from("/project/src"), 0),
        FileEntry::file(PathBuf::from("/project/src/main.rs"), 1),
    ]);
    tree.record_activity(&PathBuf::from("/project/src/main.rs"), Some(agent));
    tree.refresh_caches();

    // Verify state
    assert_eq!(focus.active_agent, Some(agent));
    assert!(traces.active_trace(agent).is_some());
    assert_eq!(turns.agent_turn_count(agent), 0);

    let recent = tree.recent_activity(10);
    assert_eq!(recent.len(), 1);
    assert_eq!(recent[0].modified_by, Some(agent));
}
