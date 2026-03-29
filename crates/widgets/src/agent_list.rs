use crate::{RenderContext, Widget, WidgetAction};
use orchestrator_core::{AgentStatus, ScrollDirection, Scrollable};
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;

pub struct AgentListWidget {
    scroll: Scrollable,
}

impl AgentListWidget {
    pub fn new() -> Self {
        Self {
            scroll: Scrollable::new(),
        }
    }
}

impl Default for AgentListWidget {
    fn default() -> Self {
        Self::new()
    }
}

impl Widget for AgentListWidget {
    fn render(&mut self, frame: &mut Frame, area: Rect, ctx: &RenderContext) {
        let block = Block::default()
            .title(" Agents ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::DarkGray));

        let inner = block.inner(area);
        frame.render_widget(block, area);

        let agents = ctx.agents.list();
        self.scroll.set_item_count(agents.len());
        self.scroll.set_visible_height(inner.height as usize);

        let range = self.scroll.visible_range();
        let lines: Vec<Line> = agents[range.clone()]
            .iter()
            .map(|agent| {
                let is_active = ctx.focus.active_agent == Some(agent.id);
                let indicator = match agent.status {
                    AgentStatus::Working => "● ",
                    AgentStatus::Idle => "○ ",
                };
                let status_color = match agent.status {
                    AgentStatus::Working => Color::Green,
                    AgentStatus::Idle => Color::DarkGray,
                };

                let name_style = if is_active {
                    Style::default()
                        .fg(Color::White)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default()
                };

                Line::from(vec![
                    Span::styled(indicator, Style::default().fg(status_color)),
                    Span::styled(&agent.name, name_style),
                ])
            })
            .collect();

        let paragraph = Paragraph::new(lines);
        frame.render_widget(paragraph, inner);
    }

    fn handle_scroll(&mut self, direction: ScrollDirection) {
        self.scroll.scroll(direction);
    }

    fn handle_click(&mut self, row: u16, _col: u16, ctx: &RenderContext) -> Option<WidgetAction> {
        let idx = self.scroll.offset + row as usize;
        let agents = ctx.agents.list();
        if idx < agents.len() {
            self.scroll.select(idx);
            Some(WidgetAction::SelectAgent(agents[idx].id))
        } else {
            None
        }
    }
}
