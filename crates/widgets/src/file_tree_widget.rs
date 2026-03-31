use crate::{RenderContext, Widget, WidgetAction};
use orchestrator_core::{FileEntry, ScrollDirection, Scrollable};
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;

use crate::activity_color;

pub struct FileTreeWidget {
    scroll: Scrollable,
}

impl FileTreeWidget {
    pub fn new() -> Self {
        Self {
            scroll: Scrollable::new(),
        }
    }
}

impl Default for FileTreeWidget {
    fn default() -> Self {
        Self::new()
    }
}

fn render_tree_entry(entry: &FileEntry, ctx: &RenderContext) -> Line<'static> {
    let indent = "  ".repeat(entry.depth);
    let color = activity_color(entry.age_secs(), entry.modify_count);

    let icon = if entry.is_dir {
        if entry.expanded {
            "▼ "
        } else {
            "▶ "
        }
    } else {
        "  "
    };

    let name = entry.display_name().to_string();

    let mut spans = vec![
        Span::raw(indent),
        Span::styled(icon, Style::default().fg(Color::DarkGray)),
        Span::styled(name, Style::default().fg(color)),
    ];

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

impl Widget for FileTreeWidget {
    fn render(&mut self, frame: &mut Frame, area: Rect, ctx: &RenderContext) {
        let block = Block::default()
            .title(" File Tree ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::DarkGray));

        let inner = block.inner(area);
        frame.render_widget(block, area);

        let visible_count = ctx.file_tree.visible_count();
        let item_count = if visible_count == 0 { 1 } else { visible_count };

        self.scroll.set_item_count(item_count);
        self.scroll.set_visible_height(inner.height as usize);

        let range = self.scroll.visible_range();
        let mut lines: Vec<Line> = Vec::with_capacity(range.len());

        for i in range {
            let line = if visible_count == 0 {
                Line::from(Span::styled(
                    " (empty)",
                    Style::default().fg(Color::DarkGray),
                ))
            } else {
                match ctx.file_tree.visible_entry_at(i) {
                    Some(entry) => render_tree_entry(entry, ctx),
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

    fn handle_click(&mut self, row: u16, _col: u16, ctx: &RenderContext) -> Option<WidgetAction> {
        let actual_row = self.scroll.offset + row as usize;
        if let Some(entry) = ctx.file_tree.visible_entry_at(actual_row) {
            if entry.is_dir {
                return Some(WidgetAction::ToggleDir(actual_row));
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_creates_widget() {
        let widget = FileTreeWidget::default();
        assert_eq!(widget.scroll.offset, 0);
    }
}
