use crate::{RenderContext, Widget, WidgetAction};
use crossterm::event::{KeyCode, KeyEvent};
use orchestrator_core::{AgentId, ScrollDirection};
use ratatui::layout::{Alignment, Rect};
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;

pub struct DoctorPage {
    pub agent_id: Option<AgentId>,
}

impl DoctorPage {
    pub fn new() -> Self {
        Self { agent_id: None }
    }

    pub fn set_agent(&mut self, id: AgentId) {
        self.agent_id = Some(id);
    }

    pub fn handle_key(&mut self, key: KeyEvent) -> Option<WidgetAction> {
        if self.agent_id.is_none() && key.code == KeyCode::Enter {
            return Some(WidgetAction::StartDoctor);
        }
        None
    }
}

impl Default for DoctorPage {
    fn default() -> Self {
        Self::new()
    }
}

impl Widget for DoctorPage {
    fn render(&mut self, frame: &mut Frame, area: Rect, ctx: &RenderContext) {
        let block = Block::default()
            .title(" Doctor ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan));

        match self.agent_id {
            Some(id) => {
                let inner = block.inner(area);
                frame.render_widget(block, area);
                (ctx.render_term)(id, frame.buffer_mut(), inner);
            }
            None => {
                let lines = vec![
                    Line::raw(""),
                    Line::from(vec![
                        Span::styled("Press ", Style::default().fg(Color::DarkGray)),
                        Span::styled("Enter", Style::default().fg(Color::Cyan)),
                        Span::styled(
                            " to start the doctor agent",
                            Style::default().fg(Color::DarkGray),
                        ),
                    ]),
                ];
                let paragraph = Paragraph::new(lines)
                    .block(block)
                    .alignment(Alignment::Center);
                frame.render_widget(paragraph, area);
            }
        }
    }

    fn handle_scroll(&mut self, _direction: ScrollDirection) {}

    fn handle_click(&mut self, _row: u16, _col: u16, _ctx: &RenderContext) -> Option<WidgetAction> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::{KeyEventKind, KeyEventState, KeyModifiers};

    fn make_key(code: KeyCode) -> KeyEvent {
        KeyEvent {
            code,
            modifiers: KeyModifiers::NONE,
            kind: KeyEventKind::Press,
            state: KeyEventState::NONE,
        }
    }

    #[test]
    fn enter_before_start_returns_start_action() {
        let mut page = DoctorPage::new();
        let action = page.handle_key(make_key(KeyCode::Enter));
        assert!(matches!(action, Some(WidgetAction::StartDoctor)));
    }

    #[test]
    fn enter_after_start_returns_none() {
        let mut page = DoctorPage::new();
        page.set_agent(AgentId::from_u128(1));
        let action = page.handle_key(make_key(KeyCode::Enter));
        assert!(action.is_none());
    }

    #[test]
    fn other_keys_before_start_return_none() {
        let mut page = DoctorPage::new();
        let action = page.handle_key(make_key(KeyCode::Char('a')));
        assert!(action.is_none());
    }
}
