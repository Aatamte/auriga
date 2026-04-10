use crate::{RenderContext, Widget, WidgetAction};
use orchestrator_core::{ScrollDirection, Scrollable};
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;

// ---------------------------------------------------------------------------
// View types (pushed in by App)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct ContextMapView {
    pub content: String,
    pub last_updated: Option<String>,
}

#[derive(Debug, Clone)]
pub struct AnnotationView {
    pub path: String,
    pub purpose: String,
}

#[derive(Debug, Clone)]
pub struct DeepContextView {
    pub name: String,
    pub description: Option<String>,
    pub last_updated: Option<String>,
}

// ---------------------------------------------------------------------------
// Widget
// ---------------------------------------------------------------------------

pub struct ContextPage {
    pub map: Option<ContextMapView>,
    pub annotations: Vec<AnnotationView>,
    pub deep_contexts: Vec<DeepContextView>,
    scroll: Scrollable,
}

impl ContextPage {
    pub fn new() -> Self {
        Self {
            map: None,
            annotations: Vec::new(),
            deep_contexts: Vec::new(),
            scroll: Scrollable::new(),
        }
    }

    pub fn set_map(&mut self, map: Option<ContextMapView>) {
        self.map = map;
    }

    pub fn set_annotations(&mut self, annotations: Vec<AnnotationView>) {
        self.annotations = annotations;
    }

    pub fn set_deep_contexts(&mut self, docs: Vec<DeepContextView>) {
        self.deep_contexts = docs;
    }

    fn build_lines(&self, width: u16) -> Vec<Line<'static>> {
        let mut lines: Vec<Line<'static>> = Vec::new();
        let separator = format!(" {}", "─".repeat(width.saturating_sub(2) as usize));
        let dim_sep = Style::default().fg(Color::DarkGray);

        // --- Layer 0: Map ---
        lines.push(Line::styled(
            " Project Map",
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        ));
        lines.push(Line::styled(separator.clone(), dim_sep));

        if let Some(ref map) = self.map {
            let verified = map.last_updated.as_deref().unwrap_or("never");
            lines.push(Line::from(vec![
                Span::raw("  "),
                Span::styled("map.md", Style::default().fg(Color::Cyan)),
                Span::styled(
                    format!("  (verified: {})", verified),
                    Style::default().fg(Color::DarkGray),
                ),
            ]));
            // Show first few lines of content as preview
            for line in map.content.lines().take(5) {
                lines.push(Line::from(vec![
                    Span::raw("    "),
                    Span::styled(
                        line.to_string(),
                        Style::default()
                            .fg(Color::DarkGray)
                            .add_modifier(Modifier::DIM),
                    ),
                ]));
            }
            if map.content.lines().count() > 5 {
                lines.push(Line::styled(
                    "    ...",
                    Style::default().fg(Color::DarkGray),
                ));
            }
        } else {
            lines.push(Line::styled(
                "  No map.md found",
                Style::default().fg(Color::DarkGray),
            ));
        }

        lines.push(Line::from(""));

        // --- Layer 1: Annotations ---
        lines.push(Line::styled(
            " File Annotations",
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        ));
        lines.push(Line::styled(separator.clone(), dim_sep));

        if self.annotations.is_empty() {
            lines.push(Line::styled(
                "  No annotations in annotations.yaml",
                Style::default().fg(Color::DarkGray),
            ));
        } else {
            lines.push(Line::styled(
                format!("  {} files annotated", self.annotations.len()),
                Style::default().fg(Color::DarkGray),
            ));
            for ann in &self.annotations {
                lines.push(Line::from(vec![
                    Span::raw("  "),
                    Span::styled(ann.path.clone(), Style::default().fg(Color::Cyan)),
                ]));
                if !ann.purpose.is_empty() {
                    lines.push(Line::from(vec![
                        Span::raw("    "),
                        Span::styled(ann.purpose.clone(), Style::default().fg(Color::DarkGray)),
                    ]));
                }
            }
        }

        lines.push(Line::from(""));

        // --- Layer 2: Deep Context ---
        lines.push(Line::styled(
            " Deep Context",
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        ));
        lines.push(Line::styled(separator, dim_sep));

        if self.deep_contexts.is_empty() {
            lines.push(Line::styled(
                "  No deep context documents",
                Style::default().fg(Color::DarkGray),
            ));
        } else {
            for doc in &self.deep_contexts {
                let verified = doc.last_updated.as_deref().unwrap_or("never");
                lines.push(Line::from(vec![
                    Span::raw("  "),
                    Span::styled(doc.name.clone(), Style::default().fg(Color::Cyan)),
                    Span::styled(
                        format!("  (verified: {})", verified),
                        Style::default().fg(Color::DarkGray),
                    ),
                ]));
            }
        }

        lines
    }
}

impl Default for ContextPage {
    fn default() -> Self {
        Self::new()
    }
}

impl Widget for ContextPage {
    fn render(&mut self, frame: &mut Frame, area: Rect, _ctx: &RenderContext) {
        let block = Block::default()
            .title(" Repository Context ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::DarkGray));

        let inner = block.inner(area);
        frame.render_widget(block, area);

        if inner.height == 0 || inner.width == 0 {
            return;
        }

        let all_lines = self.build_lines(inner.width);

        self.scroll.set_item_count(all_lines.len());
        self.scroll.set_visible_height(inner.height as usize);

        let range = self.scroll.visible_range();
        let visible: Vec<Line> = all_lines
            .into_iter()
            .skip(range.start)
            .take(range.len())
            .collect();

        let paragraph = Paragraph::new(visible);
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
    fn new_page_creates() {
        let page = ContextPage::new();
        assert!(page.map.is_none());
        assert!(page.annotations.is_empty());
        assert!(page.deep_contexts.is_empty());
    }

    #[test]
    fn set_map_updates() {
        let mut page = ContextPage::new();
        page.set_map(Some(ContextMapView {
            content: "# Test Project\nA test.".into(),
            last_updated: Some("2026-04-06".into()),
        }));
        assert!(page.map.is_some());
    }

    #[test]
    fn set_annotations_updates() {
        let mut page = ContextPage::new();
        page.set_annotations(vec![AnnotationView {
            path: "src/main.rs".into(),
            purpose: "Entry point".into(),
        }]);
        assert_eq!(page.annotations.len(), 1);
    }

    #[test]
    fn build_lines_empty_state() {
        let page = ContextPage::new();
        let lines = page.build_lines(80);
        // Should have section headers even when empty
        assert!(lines.len() >= 6);
    }

    #[test]
    fn build_lines_with_data() {
        let mut page = ContextPage::new();
        page.set_map(Some(ContextMapView {
            content: "Line 1\nLine 2\nLine 3".into(),
            last_updated: Some("2026-04-06".into()),
        }));
        page.set_annotations(vec![
            AnnotationView {
                path: "src/main.rs".into(),
                purpose: "Entry point".into(),
            },
            AnnotationView {
                path: "src/lib.rs".into(),
                purpose: "Library root".into(),
            },
        ]);
        page.set_deep_contexts(vec![DeepContextView {
            name: "principles".into(),
            description: Some("Code quality and conventions.".into()),
            last_updated: Some("2026-04-06".into()),
        }]);

        let lines = page.build_lines(80);
        // Should contain map preview, annotations, deep context
        let text: String = lines.iter().map(|l| format!("{:?}", l)).collect();
        assert!(text.contains("map.md"));
        assert!(text.contains("src/main.rs"));
        assert!(text.contains("principles"));
    }
}
