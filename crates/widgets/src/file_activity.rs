use crate::{RenderContext, Widget, WidgetAction};
use orchestrator_core::{FileEntry, FileTree, ScrollDirection, Scrollable};
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;

const RECENT_LIMIT: usize = 10;

struct SectionLayout {
    recent_header: usize,
    recent_body: usize,
    tree_header: usize,
    tree_body: usize,
}

impl SectionLayout {
    fn new(file_tree: &FileTree) -> Self {
        let recent_count = file_tree.recent_count(RECENT_LIMIT);
        Self {
            recent_header: 2, // "Recent Activity" + separator
            recent_body: if recent_count == 0 { 1 } else { recent_count },
            tree_header: 3, // blank + "File Tree" + separator
            tree_body: file_tree.visible_count(),
        }
    }

    fn total(&self) -> usize {
        self.recent_header + self.recent_body + self.tree_header + self.tree_body
    }

    fn tree_start(&self) -> usize {
        self.recent_header + self.recent_body + self.tree_header
    }
}

pub struct FileActivityWidget {
    scroll: Scrollable,
}

impl FileActivityWidget {
    pub fn new() -> Self {
        Self {
            scroll: Scrollable::new(),
        }
    }
}

impl Default for FileActivityWidget {
    fn default() -> Self {
        Self::new()
    }
}

fn activity_color(age_secs: Option<f64>, modify_count: usize) -> Color {
    let Some(age) = age_secs else {
        return Color::DarkGray;
    };

    let boost = if modify_count > 10 {
        3.0
    } else if modify_count > 5 {
        2.0
    } else if modify_count > 2 {
        1.5
    } else {
        1.0
    };

    let effective_age = age / boost;

    if effective_age < 5.0 {
        Color::Red
    } else if effective_age < 30.0 {
        Color::Yellow
    } else if effective_age < 120.0 {
        Color::Green
    } else if effective_age < 600.0 {
        Color::Cyan
    } else {
        Color::DarkGray
    }
}

fn render_recent_entry(entry: &FileEntry, ctx: &RenderContext) -> Line<'static> {
    let color = activity_color(entry.age_secs(), entry.modify_count);
    let name = entry.display_name().to_string();

    let mut spans = vec![Span::styled(name, Style::default().fg(color))];

    // Diff stats
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

impl Widget for FileActivityWidget {
    fn render(&mut self, frame: &mut Frame, area: Rect, ctx: &RenderContext) {
        let block = Block::default()
            .title(" Files ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::DarkGray));

        let inner = block.inner(area);
        frame.render_widget(block, area);

        let layout = SectionLayout::new(ctx.file_tree);
        self.scroll.set_item_count(layout.total());
        self.scroll.set_visible_height(inner.height as usize);

        let range = self.scroll.visible_range();
        let separator_line = "─".repeat(inner.width as usize);
        let recent_empty = layout.recent_body == 1 && ctx.file_tree.recent_count(RECENT_LIMIT) == 0;

        let recent_start = layout.recent_header;
        let recent_end = recent_start + layout.recent_body;
        let tree_hdr_start = recent_end; // blank line
        let tree_start = layout.tree_start();

        let mut lines: Vec<Line> = Vec::with_capacity(range.len());
        for i in range {
            let line = if i == 0 {
                // "Recent Activity" header
                Line::from(Span::styled(
                    " Recent Activity",
                    Style::default()
                        .fg(Color::White)
                        .add_modifier(Modifier::BOLD),
                ))
            } else if i == 1 {
                // Separator
                Line::from(Span::styled(
                    separator_line.clone(),
                    Style::default().fg(Color::DarkGray),
                ))
            } else if i < recent_end {
                // Recent entries
                if recent_empty {
                    Line::from(Span::styled(
                        " (no activity yet)",
                        Style::default().fg(Color::DarkGray),
                    ))
                } else {
                    let entry_idx = i - recent_start;
                    match ctx.file_tree.recent_entry_at(entry_idx) {
                        Some(entry) => render_recent_entry(entry, ctx),
                        None => Line::raw(""),
                    }
                }
            } else if i == tree_hdr_start {
                Line::raw("")
            } else if i == tree_hdr_start + 1 {
                Line::from(Span::styled(
                    " File Tree",
                    Style::default()
                        .fg(Color::White)
                        .add_modifier(Modifier::BOLD),
                ))
            } else if i == tree_hdr_start + 2 {
                Line::from(Span::styled(
                    separator_line.clone(),
                    Style::default().fg(Color::DarkGray),
                ))
            } else {
                // Tree entries
                let tree_idx = i - tree_start;
                match ctx.file_tree.visible_entry_at(tree_idx) {
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
        let layout = SectionLayout::new(ctx.file_tree);
        let tree_start = layout.tree_start();

        if actual_row >= tree_start {
            let tree_idx = actual_row - tree_start;
            if let Some(entry) = ctx.file_tree.visible_entry_at(tree_idx) {
                if entry.is_dir {
                    return Some(WidgetAction::ToggleDir(tree_idx));
                }
            }
        }

        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use orchestrator_core::{AgentId, FileEntry, FileTree};
    use std::path::PathBuf;

    fn sample_tree() -> FileTree {
        let mut tree = FileTree::new(PathBuf::from("/project"));
        tree.set_entries(vec![
            FileEntry::dir(PathBuf::from("/project/src"), 0),
            FileEntry::file(PathBuf::from("/project/src/main.rs"), 1),
            FileEntry::file(PathBuf::from("/project/src/lib.rs"), 1),
            FileEntry::dir(PathBuf::from("/project/tests"), 0),
            FileEntry::file(PathBuf::from("/project/tests/test.rs"), 1),
        ]);
        tree
    }

    #[test]
    fn section_layout_no_recent_activity() {
        let tree = sample_tree();
        let layout = SectionLayout::new(&tree);
        assert_eq!(layout.recent_header, 2);
        assert_eq!(layout.recent_body, 1); // "(no activity yet)"
        assert_eq!(layout.tree_header, 3);
        assert_eq!(layout.tree_body, 5);
        assert_eq!(layout.total(), 11);
        assert_eq!(layout.tree_start(), 6); // 2 + 1 + 3
    }

    #[test]
    fn section_layout_with_recent_activity() {
        let mut tree = sample_tree();
        tree.record_activity(&PathBuf::from("/project/src/main.rs"), Some(AgentId(1)));
        tree.record_activity(&PathBuf::from("/project/src/lib.rs"), Some(AgentId(1)));
        tree.refresh_caches();
        let layout = SectionLayout::new(&tree);
        assert_eq!(layout.recent_body, 2);
        assert_eq!(layout.tree_start(), 7); // 2 + 2 + 3
        assert_eq!(layout.total(), 12); // 2 + 2 + 3 + 5
    }

    #[test]
    fn section_layout_collapsed_dir_reduces_tree_body() {
        let mut tree = sample_tree();
        tree.refresh_caches();
        tree.toggle_dir(0); // collapse /project/src
        tree.refresh_caches();
        let layout = SectionLayout::new(&tree);
        assert_eq!(layout.tree_body, 3); // src(collapsed), tests, tests/test.rs
    }

    #[test]
    fn no_activity_is_gray() {
        assert_eq!(activity_color(None, 0), Color::DarkGray);
    }

    #[test]
    fn activity_color_red_when_fresh() {
        assert_eq!(activity_color(Some(0.0), 1), Color::Red);
        assert_eq!(activity_color(Some(4.9), 1), Color::Red);
    }

    #[test]
    fn activity_color_yellow_when_recent() {
        assert_eq!(activity_color(Some(5.0), 1), Color::Yellow);
        assert_eq!(activity_color(Some(29.0), 1), Color::Yellow);
    }

    #[test]
    fn activity_color_green_when_moderate() {
        assert_eq!(activity_color(Some(30.0), 1), Color::Green);
        assert_eq!(activity_color(Some(119.0), 1), Color::Green);
    }

    #[test]
    fn activity_color_cyan_when_older() {
        assert_eq!(activity_color(Some(120.0), 1), Color::Cyan);
        assert_eq!(activity_color(Some(599.0), 1), Color::Cyan);
    }

    #[test]
    fn activity_color_gray_when_stale() {
        assert_eq!(activity_color(Some(600.0), 1), Color::DarkGray);
    }

    #[test]
    fn high_modify_count_boosts_color() {
        assert_eq!(activity_color(Some(60.0), 1), Color::Green);
        assert_eq!(activity_color(Some(60.0), 11), Color::Yellow);
        assert_eq!(activity_color(Some(14.0), 11), Color::Red);
    }
}
