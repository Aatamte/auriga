mod agent_list;
pub mod agent_pane;
mod database_page;
mod file_tree_widget;
pub mod nav_bar;
mod recent_activity;
mod settings_page;
mod status_bar;
mod token_chart;

pub use agent_list::AgentListWidget;
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

use orchestrator_core::{AgentId, AgentStore, FileTree, FocusState, Page, ScrollDirection, TurnStore};
use ratatui::layout::Rect;
use ratatui::style::Color;
use ratatui::Frame;

pub struct RenderContext<'a> {
    pub agents: &'a AgentStore,
    pub turns: &'a TurnStore,
    pub focus: &'a FocusState,
    pub file_tree: &'a FileTree,
    /// Render an agent's terminal directly into a buffer at the given rect.
    pub render_term: &'a dyn Fn(AgentId, &mut ratatui::buffer::Buffer, Rect),
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
