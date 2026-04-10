use crate::AgentId;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Panel {
    AgentList,
    AgentPane,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Page {
    Home,
    Prompts,
    Context,
    Classifiers,
    Doctor,
    Database,
    Settings,
}

impl Page {
    /// Primary pages — left group in the nav bar.
    pub const LEFT: &[Page] = &[Page::Home, Page::Prompts, Page::Context, Page::Classifiers];

    /// Utility pages — right group in the nav bar, after the spacer.
    pub const RIGHT: &[Page] = &[Page::Doctor, Page::Database, Page::Settings];

    /// All pages in display order.
    pub const ALL: &[Page] = &[
        Page::Home,
        Page::Prompts,
        Page::Context,
        Page::Classifiers,
        Page::Doctor,
        Page::Database,
        Page::Settings,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Page::Home => "Home",
            Page::Prompts => "Prompts",
            Page::Context => "Repository Context",
            Page::Classifiers => "Classifiers",
            Page::Doctor => "Doctor",
            Page::Database => "Database",
            Page::Settings => "Settings",
        }
    }
}

/// Tracks which panel and page are focused, and which agent is active.
#[derive(Debug)]
pub struct FocusState {
    pub panel: Panel,
    pub page: Page,
    pub active_agent: Option<AgentId>,
}

impl FocusState {
    pub fn new() -> Self {
        Self {
            panel: Panel::AgentPane,
            page: Page::Home,
            active_agent: None,
        }
    }

    pub fn set_active_agent(&mut self, id: AgentId) {
        self.active_agent = Some(id);
    }

    pub fn clear_active_agent(&mut self) {
        self.active_agent = None;
    }
}

impl Default for FocusState {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_focus_is_agent_pane_with_no_agent() {
        let focus = FocusState::new();
        assert_eq!(focus.panel, Panel::AgentPane);
        assert!(focus.active_agent.is_none());
    }

    #[test]
    fn set_and_clear_active_agent() {
        let mut focus = FocusState::new();
        focus.set_active_agent(AgentId::from_u128(1));
        assert_eq!(focus.active_agent, Some(AgentId::from_u128(1)));
        focus.clear_active_agent();
        assert!(focus.active_agent.is_none());
    }
}
