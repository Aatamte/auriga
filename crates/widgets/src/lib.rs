pub mod agent_pane;
mod database_page;
mod file_tree_widget;
pub mod nav_bar;
pub mod prompts_page;
mod recent_activity;
mod settings_page;

pub use agent_pane::AgentPaneWidget;
pub use auriga_grid::WidgetId;
pub use database_page::{
    DatabasePage, DbMetadata as DbMetadataView, QueryResult as QueryResultView,
    TableInfo as TableInfoView,
};
pub use file_tree_widget::FileTreeWidget;
pub use nav_bar::NavBarWidget;
pub use prompts_page::{PromptsPage, SystemPromptEntry};
pub use recent_activity::RecentActivityWidget;
pub use settings_page::{FieldKind, SettingsField, SettingsPage, SettingsSection};

use auriga_core::{
    AgentId, AgentStore, FileTree, FocusState, Page, ScrollDirection, TraceStore, TurnStore,
};
use ratatui::layout::Rect;
use ratatui::style::Color;
use ratatui::Frame;

pub struct RenderContext<'a> {
    pub agents: &'a AgentStore,
    pub turns: &'a TurnStore,
    pub traces: &'a TraceStore,
    pub focus: &'a FocusState,
    pub file_tree: &'a FileTree,
    /// Render an agent's terminal directly into a buffer at the given rect.
    pub render_term: &'a dyn Fn(AgentId, &mut ratatui::buffer::Buffer, Rect),
    /// Pages to hide from the nav bar.
    pub hidden_pages: &'a [Page],
}

/// Format a token count compactly: 500, 1.0K, 45.5K, 1.2M
pub fn format_tokens(n: u64) -> String {
    if n >= 1_000_000 {
        format!("{:.1}M", n as f64 / 1_000_000.0)
    } else if n >= 1_000 {
        format!("{:.1}K", n as f64 / 1_000.0)
    } else {
        n.to_string()
    }
}

pub trait Widget {
    fn render(&mut self, frame: &mut Frame, area: Rect, ctx: &RenderContext);
    fn handle_scroll(&mut self, direction: ScrollDirection);
    fn handle_click(&mut self, row: u16, col: u16, ctx: &RenderContext) -> Option<WidgetAction>;
}

pub enum WidgetAction {
    SelectAgent(AgentId),
    ToggleDir(usize),
    FocusAgent(AgentId),
    BackToGrid,
    NavigateTo(Page),
    SaveConfig,
    RefreshDatabase,
    QueryTable {
        table: String,
        limit: u64,
        offset: u64,
    },
    ToggleSystemPrompt(String),
    DownloadSkill(String),
    DeleteSkill(String),
}

pub struct WidgetRegistry {
    pub agent_pane: AgentPaneWidget,
    pub recent_activity: RecentActivityWidget,
    pub file_tree: FileTreeWidget,
    pub nav_bar: NavBarWidget,
    pub settings_page: SettingsPage,
    pub database_page: DatabasePage,
    pub prompts_page: PromptsPage,
}

impl WidgetRegistry {
    pub fn new() -> Self {
        Self {
            agent_pane: AgentPaneWidget::new(),
            recent_activity: RecentActivityWidget::new(),
            file_tree: FileTreeWidget::new(),
            nav_bar: NavBarWidget::new(),
            settings_page: SettingsPage::new(),
            database_page: DatabasePage::new(),
            prompts_page: PromptsPage::new(),
        }
    }

    pub fn get_mut(&mut self, id: WidgetId) -> &mut dyn Widget {
        match id {
            WidgetId::AgentPane => &mut self.agent_pane,
            WidgetId::RecentActivity => &mut self.recent_activity,
            WidgetId::FileTree => &mut self.file_tree,
            WidgetId::SettingsPage => &mut self.settings_page,
            WidgetId::DatabasePage => &mut self.database_page,
            WidgetId::PromptsPage => &mut self.prompts_page,
        }
    }
}

impl Default for WidgetRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Shared color function for file activity age visualization.
pub fn activity_color(age_secs: Option<f64>, modify_count: usize) -> Color {
    let Some(age) = age_secs else {
        return Color::DarkGray;
    };

    let boost = if modify_count > 10 {
        3.0
    } else if modify_count > 5 {
        2.0
    } else if modify_count > 2 {
        1.5
    } else {
        1.0
    };

    let effective_age = age / boost;

    if effective_age < 5.0 {
        Color::Red
    } else if effective_age < 30.0 {
        Color::Yellow
    } else if effective_age < 120.0 {
        Color::Green
    } else if effective_age < 600.0 {
        Color::Cyan
    } else {
        Color::DarkGray
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_tokens_small() {
        assert_eq!(format_tokens(0), "0");
        assert_eq!(format_tokens(500), "500");
        assert_eq!(format_tokens(999), "999");
    }

    #[test]
    fn format_tokens_thousands() {
        assert_eq!(format_tokens(1_000), "1.0K");
        assert_eq!(format_tokens(15_500), "15.5K");
    }

    #[test]
    fn format_tokens_millions() {
        assert_eq!(format_tokens(1_000_000), "1.0M");
        assert_eq!(format_tokens(2_500_000), "2.5M");
    }

    #[test]
    fn activity_color_none_age_returns_dark_gray() {
        assert_eq!(activity_color(None, 0), Color::DarkGray);
        assert_eq!(activity_color(None, 100), Color::DarkGray);
    }

    #[test]
    fn activity_color_recent_is_red() {
        assert_eq!(activity_color(Some(1.0), 0), Color::Red);
        assert_eq!(activity_color(Some(4.9), 0), Color::Red);
    }

    #[test]
    fn activity_color_medium_is_yellow() {
        assert_eq!(activity_color(Some(10.0), 0), Color::Yellow);
    }

    #[test]
    fn activity_color_old_is_dark_gray() {
        assert_eq!(activity_color(Some(1000.0), 0), Color::DarkGray);
    }

    #[test]
    fn activity_color_high_modify_count_boosts() {
        // 100s / 3.0 boost = 33.3 effective -> Green (30..120)
        assert_eq!(activity_color(Some(100.0), 15), Color::Green);
        // Without boost, 100s -> Green (30..120) too, but let's try 200s
        // 200s / 1.0 = 200 -> Cyan
        assert_eq!(activity_color(Some(200.0), 0), Color::Cyan);
        // 200s / 3.0 = 66.7 -> Green (still in 30..120)
        assert_eq!(activity_color(Some(200.0), 15), Color::Green);
    }

    #[test]
    fn widget_registry_creates_all_widgets() {
        let registry = WidgetRegistry::new();
        // Just verify construction succeeds and get_mut works
        let mut registry = registry;
        let _ = registry.get_mut(WidgetId::AgentPane);
    }
}
