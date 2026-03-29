use crate::agent::AgentId;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Panel {
    AgentList,
    AgentPane,
}

#[derive(Debug)]
pub struct FocusState {
    pub panel: Panel,
    pub active_agent: Option<AgentId>,
}

impl FocusState {
    pub fn new() -> Self {
        Self {
            panel: Panel::AgentPane,
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
        focus.set_active_agent(AgentId(1));
        assert_eq!(focus.active_agent, Some(AgentId(1)));
        focus.clear_active_agent();
        assert!(focus.active_agent.is_none());
    }
}
