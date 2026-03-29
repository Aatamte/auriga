mod agent_list;
pub mod agent_pane;
mod file_activity;
mod status_bar;

pub use agent_list::AgentListWidget;
pub use agent_pane::AgentPaneWidget;
pub use file_activity::FileActivityWidget;
pub use status_bar::StatusBarWidget;

use orchestrator_core::{AgentId, AgentStore, FileTree, FocusState, ScrollDirection};
use ratatui::layout::Rect;
use ratatui::Frame;

pub struct RenderContext<'a> {
    pub agents: &'a AgentStore,
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
}

pub struct WidgetRegistry {
    pub agent_list: AgentListWidget,
    pub agent_pane: AgentPaneWidget,
    pub file_activity: FileActivityWidget,
    pub status_bar: StatusBarWidget,
}

impl WidgetRegistry {
    pub fn new() -> Self {
        Self {
            agent_list: AgentListWidget::new(),
            agent_pane: AgentPaneWidget::new(),
            file_activity: FileActivityWidget::new(),
            status_bar: StatusBarWidget::new(),
        }
    }

    pub fn get_mut(&mut self, name: &str) -> Option<&mut dyn Widget> {
        match name {
            "agent-list" => Some(&mut self.agent_list),
            "agent-pane" => Some(&mut self.agent_pane),
            "file-activity" => Some(&mut self.file_activity),
            "status-bar" => Some(&mut self.status_bar),
            _ => None,
        }
    }
}

impl Default for WidgetRegistry {
    fn default() -> Self {
        Self::new()
    }
}
