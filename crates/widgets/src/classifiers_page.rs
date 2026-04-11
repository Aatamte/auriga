use crate::{RenderContext, Widget, WidgetAction};
use auriga_core::{ScrollDirection, Scrollable};
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;

/// Widget-local view of a classifier's status (kept for sidebar/panel reuse).
#[derive(Debug, Clone)]
pub struct ClassifierStatusView {
    pub name: String,
    pub trigger: String,
    pub enabled: bool,
}

/// Full detail view of a classifier for the page.
#[derive(Debug, Clone)]
pub struct ClassifierDetailView {
    pub name: String,
    pub description: String,
    pub version: String,
    pub classifier_type: String,
    pub trigger: String,
    pub enabled: bool,
    pub labels: Vec<LabelView>,
}

#[derive(Debug, Clone)]
pub struct LabelView {
    pub label: String,
    pub notification: String,
}

pub struct ClassifiersPage {
    pub classifiers: Vec<ClassifierDetailView>,
    selected: usize,
    scroll: Scrollable,
}

impl ClassifiersPage {
    pub fn new() -> Self {
        Self {
            classifiers: Vec::new(),
            selected: 0,
            scroll: Scrollable::new(),
        }
    }

    pub fn set_classifiers(&mut self, classifiers: Vec<ClassifierDetailView>) {
        self.classifiers = classifiers;
        if self.selected >= self.classifiers.len() && !self.classifiers.is_empty() {
            self.selected = self.classifiers.len() - 1;
        }
    }

    pub fn handle_key(&mut self, key: KeyEvent) -> Option<WidgetAction> {
        match key.code {
            KeyCode::Up => {
                if self.selected > 0 {
                    self.selected -= 1;
                }
            }
            KeyCode::Down => {
                if self.selected + 1 < self.classifiers.len() {
                    self.selected += 1;
                }
            }
            KeyCode::Enter | KeyCode::Char(' ') => {
                if let Some(c) = self.classifiers.get(self.selected) {
                    return Some(WidgetAction::ToggleClassifier(c.name.clone()));
                }
            }
            _ => {}
        }
        None
    }

    /// Build the lines for a single classifier detail block.
    fn render_classifier(c: &ClassifierDetailView, is_selected: bool, width: u16) -> Vec<Line<'_>> {
        let mut lines = Vec::new();

        // Header: checkbox + name
        let checkbox = if c.enabled { "[x]" } else { "[ ]" };
        let check_color = if c.enabled {
            Color::Green
        } else {
            Color::DarkGray
        };
        let name_style = if is_selected {
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::White)
        };

        lines.push(Line::from(vec![
            Span::styled(checkbox, Style::default().fg(check_color)),
            Span::raw(" "),
            Span::styled(c.name.as_str(), name_style),
            Span::styled(
                format!("  v{}", c.version),
                Style::default().fg(Color::DarkGray),
            ),
            Span::styled("  │  ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                c.classifier_type.as_str(),
                Style::default().fg(Color::DarkGray),
            ),
            Span::styled("  │  ", Style::default().fg(Color::DarkGray)),
            Span::styled(c.trigger.as_str(), Style::default().fg(Color::DarkGray)),
        ]));

        // Description
        if !c.description.is_empty() {
            lines.push(Line::from(vec![
                Span::raw("    "),
                Span::styled(c.description.as_str(), Style::default().fg(Color::DarkGray)),
            ]));
        }

        // Labels + notifications
        for label in &c.labels {
            lines.push(Line::from(vec![
                Span::raw("    "),
                Span::styled("→ ", Style::default().fg(Color::Yellow)),
                Span::styled(label.label.as_str(), Style::default().fg(Color::White)),
            ]));
            if !label.notification.is_empty() {
                let msg = if label.notification.len() > (width as usize).saturating_sub(12) {
                    let limit = (width as usize).saturating_sub(15);
                    // Safe truncation at char boundary
                    let end = label
                        .notification
                        .char_indices()
                        .take_while(|(i, _)| *i < limit)
                        .last()
                        .map(|(i, ch)| i + ch.len_utf8())
                        .unwrap_or(0);
                    format!("{}...", &label.notification[..end])
                } else {
                    label.notification.clone()
                };
                lines.push(Line::from(vec![
                    Span::raw("      "),
                    Span::styled(msg, Style::default().fg(Color::DarkGray)),
                ]));
            }
        }

        // Separator
        lines.push(Line::styled(
            format!("  {}", "─".repeat(width.saturating_sub(4) as usize)),
            Style::default().fg(Color::DarkGray),
        ));

        lines
    }
}

impl Default for ClassifiersPage {
    fn default() -> Self {
        Self::new()
    }
}

impl Widget for ClassifiersPage {
    fn render(&mut self, frame: &mut Frame, area: Rect, _ctx: &RenderContext) {
        let block = Block::default()
            .title(" Classifiers ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::DarkGray));

        let inner = block.inner(area);
        frame.render_widget(block, area);

        if inner.height == 0 || inner.width == 0 {
            return;
        }

        if self.classifiers.is_empty() {
            let placeholder = Paragraph::new(Line::from(Span::styled(
                "  No classifiers registered",
                Style::default().fg(Color::DarkGray),
            )));
            frame.render_widget(placeholder, inner);
            return;
        }

        // Build all lines
        let mut all_lines: Vec<Line> = Vec::new();
        for (i, c) in self.classifiers.iter().enumerate() {
            let is_selected = self.selected == i;
            all_lines.extend(Self::render_classifier(c, is_selected, inner.width));
        }

        // Help line
        all_lines.push(Line::from(""));
        all_lines.push(Line::styled(
            "  ↑↓ select  │  Enter/Space toggle",
            Style::default().fg(Color::DarkGray),
        ));

        self.scroll.set_item_count(all_lines.len());
        self.scroll.set_visible_height(inner.height as usize);

        // Auto-scroll to keep selected classifier visible
        let mut line_idx = 0;
        for (i, c) in self.classifiers.iter().enumerate() {
            if i == self.selected {
                break;
            }
            line_idx += Self::render_classifier(c, false, inner.width).len();
        }
        self.scroll.select(line_idx);

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

    fn handle_click(&mut self, row: u16, _col: u16, _ctx: &RenderContext) -> Option<WidgetAction> {
        // Map click row to classifier index by counting rendered line heights
        let visible_row = self.scroll.offset + row as usize;
        let mut line_idx = 0;
        for (i, c) in self.classifiers.iter().enumerate() {
            let height = Self::render_classifier(c, false, 80).len();
            if visible_row < line_idx + height {
                self.selected = i;
                return Some(WidgetAction::ToggleClassifier(c.name.clone()));
            }
            line_idx += height;
        }
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

    fn sample_classifiers() -> Vec<ClassifierDetailView> {
        vec![
            ClassifierDetailView {
                name: "loop-detector".into(),
                description: "Detects looping agents".into(),
                version: "1.0".into(),
                classifier_type: "ml".into(),
                trigger: "Incremental".into(),
                enabled: true,
                labels: vec![
                    LabelView {
                        label: "looping".into(),
                        notification: "Agent is repeating the same actions".into(),
                    },
                    LabelView {
                        label: "healthy".into(),
                        notification: "".into(),
                    },
                ],
            },
            ClassifierDetailView {
                name: "cost-alert".into(),
                description: "Flags excessive token usage".into(),
                version: "2.1".into(),
                classifier_type: "cli".into(),
                trigger: "OnComplete".into(),
                enabled: false,
                labels: vec![LabelView {
                    label: "over-budget".into(),
                    notification: "Token usage exceeded threshold".into(),
                }],
            },
        ]
    }

    #[test]
    fn new_page_empty() {
        let page = ClassifiersPage::new();
        assert!(page.classifiers.is_empty());
    }

    #[test]
    fn set_classifiers_updates_list() {
        let mut page = ClassifiersPage::new();
        page.set_classifiers(sample_classifiers());
        assert_eq!(page.classifiers.len(), 2);
    }

    #[test]
    fn up_down_navigates() {
        let mut page = ClassifiersPage::new();
        page.set_classifiers(sample_classifiers());

        assert_eq!(page.selected, 0);
        page.handle_key(make_key(KeyCode::Down));
        assert_eq!(page.selected, 1);
        page.handle_key(make_key(KeyCode::Down));
        assert_eq!(page.selected, 1); // clamped
        page.handle_key(make_key(KeyCode::Up));
        assert_eq!(page.selected, 0);
    }

    #[test]
    fn enter_emits_toggle_action() {
        let mut page = ClassifiersPage::new();
        page.set_classifiers(sample_classifiers());

        let action = page.handle_key(make_key(KeyCode::Enter));
        assert!(
            matches!(action, Some(WidgetAction::ToggleClassifier(name)) if name == "loop-detector")
        );
    }

    #[test]
    fn space_emits_toggle_on_selected() {
        let mut page = ClassifiersPage::new();
        page.set_classifiers(sample_classifiers());
        page.handle_key(make_key(KeyCode::Down));

        let action = page.handle_key(make_key(KeyCode::Char(' ')));
        assert!(
            matches!(action, Some(WidgetAction::ToggleClassifier(name)) if name == "cost-alert")
        );
    }

    #[test]
    fn selected_clamped_on_shrink() {
        let mut page = ClassifiersPage::new();
        page.set_classifiers(sample_classifiers());
        page.handle_key(make_key(KeyCode::Down)); // selected = 1
        page.set_classifiers(vec![sample_classifiers().remove(0)]); // shrink to 1
        assert_eq!(page.selected, 0);
    }

    #[test]
    fn render_classifier_shows_labels() {
        let c = &sample_classifiers()[0];
        let lines = ClassifiersPage::render_classifier(c, false, 80);
        // Should have: header, description, 2 labels (each with arrow line), 1 notification, separator
        assert!(lines.len() >= 5);
    }
}
