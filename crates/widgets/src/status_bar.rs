use crate::{RenderContext, Widget, WidgetAction};
use orchestrator_core::ScrollDirection;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;

pub struct StatusBarWidget;

impl StatusBarWidget {
    pub fn new() -> Self {
        Self
    }
}

impl Default for StatusBarWidget {
    fn default() -> Self {
        Self::new()
    }
}

impl Widget for StatusBarWidget {
    fn render(&mut self, frame: &mut Frame, area: Rect, _ctx: &RenderContext) {
        let block = Block::default()
            .title(" Keys ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::DarkGray));

        let inner = block.inner(area);
        frame.render_widget(block, area);

        if inner.height == 0 || inner.width == 0 {
            return;
        }

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
    }

    fn handle_scroll(&mut self, _direction: ScrollDirection) {}

    fn handle_click(&mut self, _row: u16, _col: u16, _ctx: &RenderContext) -> Option<WidgetAction> {
        None
    }
}
