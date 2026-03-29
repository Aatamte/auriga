use crate::{RenderContext, Widget, WidgetAction};
use orchestrator_core::ScrollDirection;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;

pub struct StatusBarWidget {
    pub expanded: bool,
}

impl StatusBarWidget {
    pub fn new() -> Self {
        Self { expanded: false }
    }

    pub fn toggle_expanded(&mut self) {
        self.expanded = !self.expanded;
    }
}

impl Default for StatusBarWidget {
    fn default() -> Self {
        Self::new()
    }
}

impl Widget for StatusBarWidget {
    fn render(&mut self, frame: &mut Frame, area: Rect, _ctx: &RenderContext) {
        let title = if self.expanded {
            " Keys [ctrl+s] ▼ "
        } else {
            " Keys [ctrl+s] ▶ "
        };

        let block = Block::default()
            .title(title)
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::DarkGray));

        let inner = block.inner(area);
        frame.render_widget(block, area);

        if inner.height == 0 || inner.width == 0 {
            return;
        }

        if self.expanded {
            let lines = vec![
                Line::from(vec![
                    Span::styled("ctrl+n", Style::default().fg(Color::Cyan)),
                    Span::raw(" new"),
                ]),
                Line::from(vec![
                    Span::styled("ctrl+b", Style::default().fg(Color::Cyan)),
                    Span::raw(" view"),
                ]),
                Line::from(vec![
                    Span::styled("ctrl+w", Style::default().fg(Color::Cyan)),
                    Span::raw(" close"),
                ]),
                Line::from(vec![
                    Span::styled("ctrl+q", Style::default().fg(Color::Cyan)),
                    Span::raw(" quit"),
                ]),
                Line::from(vec![
                    Span::styled("shift+click", Style::default().fg(Color::Cyan)),
                    Span::raw(" copy"),
                ]),
            ];

            let paragraph = Paragraph::new(lines);
            frame.render_widget(paragraph, inner);
        } else {
            let hint = Line::from(Span::styled(
                "ctrl+s expand",
                Style::default()
                    .fg(Color::DarkGray)
                    .add_modifier(Modifier::ITALIC),
            ));
            let paragraph = Paragraph::new(hint);
            frame.render_widget(paragraph, inner);
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

    #[test]
    fn default_is_collapsed() {
        let widget = StatusBarWidget::new();
        assert!(!widget.expanded);
    }

    #[test]
    fn toggle_expands_and_collapses() {
        let mut widget = StatusBarWidget::new();
        widget.toggle_expanded();
        assert!(widget.expanded);
        widget.toggle_expanded();
        assert!(!widget.expanded);
    }
}
