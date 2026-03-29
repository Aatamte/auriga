use crate::{RenderContext, Widget, WidgetAction};
use orchestrator_core::ScrollDirection;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

pub struct StatusBarWidget;

impl Widget for StatusBarWidget {
    fn render(&mut self, frame: &mut Frame, area: Rect, _ctx: &RenderContext) {
        let hints = Line::from(vec![
            Span::styled("ctrl+n", Style::default().fg(Color::Cyan)),
            Span::raw(" new  "),
            Span::styled("ctrl+b", Style::default().fg(Color::Cyan)),
            Span::raw(" view  "),
            Span::styled("ctrl+w", Style::default().fg(Color::Cyan)),
            Span::raw(" close  "),
            Span::styled("ctrl+q", Style::default().fg(Color::Cyan)),
            Span::raw(" quit  "),
            Span::styled("shift+click", Style::default().fg(Color::Cyan)),
            Span::raw(" copy"),
        ]);

        let bar =
            Paragraph::new(hints).style(Style::default().bg(Color::DarkGray).fg(Color::White));

        frame.render_widget(bar, area);
    }

    fn handle_scroll(&mut self, _direction: ScrollDirection) {}

    fn handle_click(&mut self, _row: u16, _col: u16, _ctx: &RenderContext) -> Option<WidgetAction> {
        None
    }
}
