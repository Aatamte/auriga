use crate::{RenderContext, Widget, WidgetAction};
use orchestrator_core::{FileEntry, ScrollDirection, Scrollable};
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;

use crate::activity_color;

const RECENT_LIMIT: usize = 10;

pub struct RecentActivityWidget {
    scroll: Scrollable,
}

impl RecentActivityWidget {
    pub fn new() -> Self {
        Self {
            scroll: Scrollable::new(),
        }
    }
}

impl Default for RecentActivityWidget {
    fn default() -> Self {
        Self::new()
    }
}

fn render_recent_entry(entry: &FileEntry, ctx: &RenderContext) -> Line<'static> {
    let color = activity_color(entry.age_secs(), entry.modify_count);
    let name = entry.display_name().to_string();

    let mut spans = vec![Span::styled(name, Style::default().fg(color))];

    if entry.lines_added > 0 || entry.lines_removed > 0 {
        spans.push(Span::raw(" "));
        if entry.lines_added > 0 {
            spans.push(Span::styled(
                format!("+{}", entry.lines_added),
                Style::default().fg(Color::Green),
            ));
        }
        if entry.lines_removed > 0 {
            spans.push(Span::styled(
                format!("-{}", entry.lines_removed),
                Style::default().fg(Color::Red),
            ));
        }
    }

    if let Some(agent_id) = entry.modified_by {
        if let Some(agent) = ctx.agents.get(agent_id) {
            spans.push(Span::styled(
                format!("  {}", agent.name),
                Style::default()
                    .fg(Color::DarkGray)
                    .add_modifier(Modifier::ITALIC),
            ));
        }
    }

    Line::from(spans)
}

impl Widget for RecentActivityWidget {
    fn render(&mut self, frame: &mut Frame, area: Rect, ctx: &RenderContext) {
        let block = Block::default()
            .title(" Recent Activity ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::DarkGray));

        let inner = block.inner(area);
        frame.render_widget(block, area);

        let recent_count = ctx.file_tree.recent_count(RECENT_LIMIT);
        let item_count = if recent_count == 0 { 1 } else { recent_count };

        self.scroll.set_item_count(item_count);
        self.scroll.set_visible_height(inner.height as usize);

        let range = self.scroll.visible_range();
        let mut lines: Vec<Line> = Vec::with_capacity(range.len());

        for i in range {
            let line = if recent_count == 0 {
                Line::from(Span::styled(
                    " (no activity yet)",
                    Style::default().fg(Color::DarkGray),
                ))
            } else {
                match ctx.file_tree.recent_entry_at(i) {
                    Some(entry) => render_recent_entry(entry, ctx),
                    None => Line::raw(""),
                }
            };
            lines.push(line);
        }

        let paragraph = Paragraph::new(lines);
        frame.render_widget(paragraph, inner);
    }

    fn handle_scroll(&mut self, direction: ScrollDirection) {
        self.scroll.scroll(direction);
    }

    fn handle_click(&mut self, _row: u16, _col: u16, _ctx: &RenderContext) -> Option<WidgetAction> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_creates_widget() {
        let widget = RecentActivityWidget::default();
        assert_eq!(widget.scroll.offset, 0);
    }
}
