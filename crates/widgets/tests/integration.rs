//! Integration tests for auriga-widgets public API

use auriga_core::{
    AgentStore, FileEntry, FileTree, FocusState, ScrollDirection, TraceStore, TurnStore,
};
use auriga_widgets::{
    activity_color, format_tokens, RenderContext, Widget, WidgetAction, WidgetId, WidgetRegistry,
};
use ratatui::backend::TestBackend;
use ratatui::layout::Rect;
use ratatui::style::Color;
use ratatui::Terminal;
use std::path::PathBuf;

fn make_render_context<'a>(
    agents: &'a AgentStore,
    turns: &'a TurnStore,
    traces: &'a TraceStore,
    focus: &'a FocusState,
    file_tree: &'a FileTree,
) -> RenderContext<'a> {
    RenderContext {
        agents,
        turns,
        traces,
        focus,
        file_tree,
        render_term: &|_, _, _| {},
        hidden_pages: &[],
    }
}

// -- Widget registry tests --

#[test]
fn widget_registry_provides_all_widgets() {
    let mut registry = WidgetRegistry::new();

    // Can get each widget type
    let _ = registry.get_mut(WidgetId::AgentPane);
    let _ = registry.get_mut(WidgetId::RecentActivity);
    let _ = registry.get_mut(WidgetId::FileTree);
    let _ = registry.get_mut(WidgetId::SettingsPage);
    let _ = registry.get_mut(WidgetId::DatabasePage);
    let _ = registry.get_mut(WidgetId::PromptsPage);
}

#[test]
fn widgets_render_without_panic() {
    let agents = AgentStore::new();
    let turns = TurnStore::new();
    let traces = TraceStore::new();
    let focus = FocusState::new();
    let file_tree = FileTree::new(PathBuf::from("/project"));

    let ctx = make_render_context(&agents, &turns, &traces, &focus, &file_tree);
    let mut registry = WidgetRegistry::new();

    let backend = TestBackend::new(80, 24);
    let mut terminal = Terminal::new(backend).unwrap();

    terminal
        .draw(|frame| {
            let area = Rect::new(0, 0, 80, 24);

            // Render each widget
            for widget_id in [
                WidgetId::AgentPane,
                WidgetId::RecentActivity,
                WidgetId::FileTree,
                WidgetId::SettingsPage,
                WidgetId::DatabasePage,
                WidgetId::PromptsPage,
            ] {
                registry.get_mut(widget_id).render(frame, area, &ctx);
            }
        })
        .unwrap();
}

#[test]
fn widgets_handle_scroll() {
    let mut registry = WidgetRegistry::new();

    // All widgets should handle scroll without panic
    for widget_id in [
        WidgetId::AgentPane,
        WidgetId::RecentActivity,
        WidgetId::FileTree,
        WidgetId::SettingsPage,
        WidgetId::DatabasePage,
        WidgetId::PromptsPage,
    ] {
        let widget = registry.get_mut(widget_id);
        widget.handle_scroll(ScrollDirection::Down);
        widget.handle_scroll(ScrollDirection::Up);
    }
}

#[test]
fn agent_pane_renders_with_agents() {
    let mut agents = AgentStore::new();
    let id = agents.create("claude");

    let turns = TurnStore::new();
    let traces = TraceStore::new();
    let mut focus = FocusState::new();
    focus.set_active_agent(id);
    let file_tree = FileTree::new(PathBuf::from("/project"));

    let ctx = make_render_context(&agents, &turns, &traces, &focus, &file_tree);
    let mut registry = WidgetRegistry::new();

    let backend = TestBackend::new(80, 24);
    let mut terminal = Terminal::new(backend).unwrap();

    terminal
        .draw(|frame| {
            let area = Rect::new(0, 0, 80, 24);
            registry.agent_pane.render(frame, area, &ctx);
        })
        .unwrap();
}

#[test]
fn file_tree_widget_renders_with_entries() {
    let agents = AgentStore::new();
    let turns = TurnStore::new();
    let traces = TraceStore::new();
    let focus = FocusState::new();

    let mut file_tree = FileTree::new(PathBuf::from("/project"));
    file_tree.set_entries(vec![
        FileEntry::dir(PathBuf::from("/project/src"), 0),
        FileEntry::file(PathBuf::from("/project/src/main.rs"), 1),
    ]);
    file_tree.refresh_caches();

    let ctx = make_render_context(&agents, &turns, &traces, &focus, &file_tree);
    let mut registry = WidgetRegistry::new();

    let backend = TestBackend::new(80, 24);
    let mut terminal = Terminal::new(backend).unwrap();

    terminal
        .draw(|frame| {
            let area = Rect::new(0, 0, 40, 20);
            registry.file_tree.render(frame, area, &ctx);
        })
        .unwrap();
}

// -- Utility function tests --

#[test]
fn format_tokens_scales_correctly() {
    assert_eq!(format_tokens(0), "0");
    assert_eq!(format_tokens(999), "999");
    assert_eq!(format_tokens(1_000), "1.0K");
    assert_eq!(format_tokens(999_999), "1000.0K");
    assert_eq!(format_tokens(1_000_000), "1.0M");
    assert_eq!(format_tokens(10_500_000), "10.5M");
}

#[test]
fn activity_color_reflects_recency() {
    // No activity
    assert_eq!(activity_color(None, 0), Color::DarkGray);

    // Very recent (< 5s)
    assert_eq!(activity_color(Some(2.0), 0), Color::Red);

    // Recent (5-30s)
    assert_eq!(activity_color(Some(15.0), 0), Color::Yellow);

    // Medium (30-120s)
    assert_eq!(activity_color(Some(60.0), 0), Color::Green);

    // Older (120-600s)
    assert_eq!(activity_color(Some(300.0), 0), Color::Cyan);

    // Old (> 600s)
    assert_eq!(activity_color(Some(1000.0), 0), Color::DarkGray);
}

#[test]
fn activity_color_boosted_by_modify_count() {
    // High modify count makes colors "hotter" for same age
    // 300s normally = Cyan, but with high modify count boosted to Green
    assert_eq!(activity_color(Some(300.0), 0), Color::Cyan);
    assert_eq!(activity_color(Some(300.0), 15), Color::Green); // 300/3 = 100 -> Green
}

// -- Widget action tests --

#[test]
fn widget_action_variants_exist() {
    // Just verify the action types exist and can be constructed
    let _ = WidgetAction::SelectAgent(auriga_core::AgentId::from_u128(1));
    let _ = WidgetAction::ToggleDir(0);
    let _ = WidgetAction::BackToGrid;
    let _ = WidgetAction::SaveConfig;
    let _ = WidgetAction::RefreshDatabase;
    let _ = WidgetAction::QueryTable {
        table: "test".to_string(),
        limit: 100,
        offset: 0,
    };
}
