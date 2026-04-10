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

#[cfg(test)]
mod tests {
    use super::*;
    use orchestrator_core::{AgentId, AgentStore, FileTree, FocusState, TraceStore, TurnStore};
    use std::path::PathBuf;

    macro_rules! with_ctx {
        ($agents:expr, |$ctx:ident| $body:expr) => {{
            let agents = $agents;
            let turns = TurnStore::new();
            let traces = TraceStore::new();
            let focus = FocusState::new();
            let file_tree = FileTree::new(PathBuf::from("/tmp"));
            let noop = |_: AgentId, _: &mut ratatui::buffer::Buffer, _: Rect| {};
            let $ctx = RenderContext {
                agents: &agents,
                turns: &turns,
                traces: &traces,
                focus: &focus,
                file_tree: &file_tree,
                render_term: &noop,
                hidden_pages: &[],
            };
            $body
        }};
    }

    #[test]
    fn new_widget_has_zero_scroll_offset() {
        let widget = AgentListWidget::new();
        assert_eq!(widget.scroll.offset, 0);
    }

    #[test]
    fn click_empty_list_returns_none() {
        let mut widget = AgentListWidget::new();
        with_ctx!(AgentStore::new(), |ctx| {
            assert!(widget.handle_click(0, 0, &ctx).is_none());
        });
    }

    #[test]
    fn click_valid_agent_returns_select_action() {
        let mut widget = AgentListWidget::new();
        let mut agents = AgentStore::new();
        let id = agents.create("claude");

        with_ctx!(agents, |ctx| {
            let action = widget.handle_click(0, 0, &ctx);
            match action {
                Some(WidgetAction::SelectAgent(selected_id)) => assert_eq!(selected_id, id),
                _ => panic!("expected SelectAgent"),
            }
        });
    }

    #[test]
    fn click_out_of_range_returns_none() {
        let mut widget = AgentListWidget::new();
        let mut agents = AgentStore::new();
        agents.create("claude");

        with_ctx!(agents, |ctx| {
            assert!(widget.handle_click(5, 0, &ctx).is_none());
        });
    }

    #[test]
    fn scroll_updates_offset() {
        let mut widget = AgentListWidget::new();
        // Need items to scroll through
        widget.scroll.set_item_count(20);
        widget.scroll.set_visible_height(5);

        widget.handle_scroll(ScrollDirection::Down);
        assert_eq!(widget.scroll.offset, 1);

        widget.handle_scroll(ScrollDirection::Up);
        assert_eq!(widget.scroll.offset, 0);
    }

    #[test]
    fn scroll_up_at_top_stays_at_zero() {
        let mut widget = AgentListWidget::new();
        widget.scroll.set_item_count(10);
        widget.scroll.set_visible_height(5);

        widget.handle_scroll(ScrollDirection::Up);
        assert_eq!(widget.scroll.offset, 0);
    }
}
