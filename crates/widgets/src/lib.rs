mod agent_list;
pub mod agent_pane;
mod classifiers_page;
mod database_page;
mod file_tree_widget;
pub mod nav_bar;
mod recent_activity;
mod settings_page;
mod status_bar;
mod token_chart;

pub use agent_list::AgentListWidget;
pub use classifiers_page::{ClassificationResultView, ClassifierStatusView, ClassifiersPage};
pub use agent_pane::AgentPaneWidget;
pub use database_page::{
    DatabasePage, DbMetadata as DbMetadataView, QueryResult as QueryResultView,
    TableInfo as TableInfoView,
};
pub use file_tree_widget::FileTreeWidget;
pub use nav_bar::NavBarWidget;
pub use recent_activity::RecentActivityWidget;
pub use settings_page::{SettingsField, SettingsPage};
pub use status_bar::StatusBarWidget;
pub use token_chart::TokenChartWidget;

use orchestrator_core::{AgentId, AgentStore, FileTree, FocusState, Page, ScrollDirection, TraceStore, TurnStore};
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
    QueryTable { table: String, limit: u64, offset: u64 },
    ToggleClassifier(String),
}

pub struct WidgetRegistry {
    pub agent_list: AgentListWidget,
    pub agent_pane: AgentPaneWidget,
    pub token_chart: TokenChartWidget,
    pub recent_activity: RecentActivityWidget,
    pub file_tree: FileTreeWidget,
    pub status_bar: StatusBarWidget,
    pub nav_bar: NavBarWidget,
    pub settings_page: SettingsPage,
    pub database_page: DatabasePage,
    pub classifiers_page: ClassifiersPage,
}

impl WidgetRegistry {
    pub fn new() -> Self {
        Self {
            agent_list: AgentListWidget::new(),
            agent_pane: AgentPaneWidget::new(),
            token_chart: TokenChartWidget::new(),
            recent_activity: RecentActivityWidget::new(),
            file_tree: FileTreeWidget::new(),
            status_bar: StatusBarWidget::new(),
            nav_bar: NavBarWidget::new(),
            settings_page: SettingsPage::new(),
            database_page: DatabasePage::new(),
            classifiers_page: ClassifiersPage::new(),
        }
    }

    pub fn get_mut(&mut self, name: &str) -> Option<&mut dyn Widget> {
        match name {
            "agent-list" => Some(&mut self.agent_list),
            "agent-pane" => Some(&mut self.agent_pane),
            "token-chart" => Some(&mut self.token_chart),
            "recent-activity" => Some(&mut self.recent_activity),
            "file-tree" => Some(&mut self.file_tree),
            "status-bar" => Some(&mut self.status_bar),
            "settings-page" => Some(&mut self.settings_page),
            "database-page" => Some(&mut self.database_page),
            "classifiers-page" => Some(&mut self.classifiers_page),
            _ => None,
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
}
