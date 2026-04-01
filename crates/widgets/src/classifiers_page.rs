use crate::{RenderContext, Widget, WidgetAction};
use crossterm::event::{KeyCode, KeyEvent};
use orchestrator_core::ScrollDirection;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;

/// Widget-local view of a classifier's status.
#[derive(Debug, Clone)]
pub struct ClassifierStatusView {
    pub name: String,
    pub trigger: String,
    pub enabled: bool,
}

/// Widget-local view of a classification result.
#[derive(Debug, Clone)]
pub struct ClassificationResultView {
    pub classifier_name: String,
    pub trace_id: String,
    pub timestamp: String,
    pub payload: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Section {
    Classifiers,
    Results,
}

pub struct ClassifiersPage {
    pub classifiers: Vec<ClassifierStatusView>,
    pub results: Vec<ClassificationResultView>,
    selected_classifier: usize,
    selected_result: usize,
    section: Section,
}

impl ClassifiersPage {
    pub fn new() -> Self {
        Self {
            classifiers: Vec::new(),
            results: Vec::new(),
            selected_classifier: 0,
            selected_result: 0,
            section: Section::Classifiers,
        }
    }

    pub fn set_classifiers(&mut self, classifiers: Vec<ClassifierStatusView>) {
        self.classifiers = classifiers;
    }

    pub fn set_results(&mut self, results: Vec<ClassificationResultView>) {
        self.results = results;
    }

    pub fn handle_key(&mut self, key: KeyEvent) -> Option<WidgetAction> {
        match key.code {
            KeyCode::Tab => {
                self.section = match self.section {
                    Section::Classifiers => Section::Results,
                    Section::Results => Section::Classifiers,
                };
            }
            KeyCode::Up => match self.section {
                Section::Classifiers => {
                    if self.selected_classifier > 0 {
                        self.selected_classifier -= 1;
                    }
                }
                Section::Results => {
                    if self.selected_result > 0 {
                        self.selected_result -= 1;
                    }
                }
            },
            KeyCode::Down => match self.section {
                Section::Classifiers => {
                    if self.selected_classifier + 1 < self.classifiers.len() {
                        self.selected_classifier += 1;
                    }
                }
                Section::Results => {
                    if self.selected_result + 1 < self.results.len() {
                        self.selected_result += 1;
                    }
                }
            },
            KeyCode::Enter | KeyCode::Char(' ') => {
                if self.section == Section::Classifiers {
                    if let Some(c) = self.classifiers.get(self.selected_classifier) {
                        return Some(WidgetAction::ToggleClassifier(c.name.clone()));
                    }
                }
            }
            _ => {}
        }
        None
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

        let mut lines: Vec<Line> = Vec::new();

        // Section: Classifiers
        let cls_header_style = if self.section == Section::Classifiers {
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::White)
        };
        lines.push(Line::styled("  Registered Classifiers", cls_header_style));
        lines.push(Line::from(""));

        if self.classifiers.is_empty() {
            lines.push(Line::styled(
                "  (none registered)",
                Style::default().fg(Color::DarkGray),
            ));
        } else {
            for (i, c) in self.classifiers.iter().enumerate() {
                let is_selected =
                    self.section == Section::Classifiers && self.selected_classifier == i;
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
                    Span::raw("  "),
                    Span::styled(checkbox, Style::default().fg(check_color)),
                    Span::raw(" "),
                    Span::styled(&c.name, name_style),
                    Span::styled("  │  ", Style::default().fg(Color::DarkGray)),
                    Span::styled(&c.trigger, Style::default().fg(Color::DarkGray)),
                ]));
            }
        }

        // Separator
        lines.push(Line::from(""));
        lines.push(Line::styled(
            format!("  {}", "─".repeat(inner.width.saturating_sub(4) as usize)),
            Style::default().fg(Color::DarkGray),
        ));
        lines.push(Line::from(""));

        // Section: Recent Results
        let res_header_style = if self.section == Section::Results {
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::White)
        };
        lines.push(Line::styled("  Recent Results", res_header_style));
        lines.push(Line::from(""));

        if self.results.is_empty() {
            lines.push(Line::styled(
                "  (no results yet)",
                Style::default().fg(Color::DarkGray),
            ));
        } else {
            // Column header
            lines.push(Line::from(vec![Span::styled(
                "  Classifier            Trace       Timestamp                 Payload",
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            )]));

            for (i, r) in self.results.iter().enumerate() {
                let is_selected = self.section == Section::Results && self.selected_result == i;
                let style = if is_selected {
                    Style::default().fg(Color::Cyan)
                } else {
                    Style::default().fg(Color::White)
                };

                let trace_short = if r.trace_id.len() > 8 {
                    &r.trace_id[..8]
                } else {
                    &r.trace_id
                };

                let payload_max = inner.width.saturating_sub(60) as usize;
                let payload_display = if r.payload.len() > payload_max {
                    format!("{}...", &r.payload[..payload_max.saturating_sub(3)])
                } else {
                    r.payload.clone()
                };

                lines.push(Line::from(vec![
                    Span::raw("  "),
                    Span::styled(format!("{:<20}", r.classifier_name), style),
                    Span::styled(format!("  {:<10}", trace_short), style),
                    Span::styled(
                        format!("  {:<24}", r.timestamp),
                        Style::default().fg(Color::DarkGray),
                    ),
                    Span::styled(payload_display, style),
                ]));
            }
        }

        // Help line
        lines.push(Line::from(""));
        lines.push(Line::styled(
            "  ↑↓ select  │  Tab switch section  │  Enter/Space toggle",
            Style::default().fg(Color::DarkGray),
        ));

        let paragraph = Paragraph::new(lines);
        frame.render_widget(paragraph, inner);
    }

    fn handle_scroll(&mut self, _direction: ScrollDirection) {}

    fn handle_click(&mut self, _row: u16, _col: u16, _ctx: &RenderContext) -> Option<WidgetAction> {
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

    fn sample_classifiers() -> Vec<ClassifierStatusView> {
        vec![
            ClassifierStatusView {
                name: "loop-detector".into(),
                trigger: "Incremental".into(),
                enabled: true,
            },
            ClassifierStatusView {
                name: "cost-alert".into(),
                trigger: "OnComplete".into(),
                enabled: false,
            },
        ]
    }

    #[test]
    fn new_page_empty() {
        let page = ClassifiersPage::new();
        assert!(page.classifiers.is_empty());
        assert!(page.results.is_empty());
    }

    #[test]
    fn set_classifiers_updates_list() {
        let mut page = ClassifiersPage::new();
        page.set_classifiers(sample_classifiers());
        assert_eq!(page.classifiers.len(), 2);
    }

    #[test]
    fn up_down_navigates_classifiers() {
        let mut page = ClassifiersPage::new();
        page.set_classifiers(sample_classifiers());

        assert_eq!(page.selected_classifier, 0);
        page.handle_key(make_key(KeyCode::Down));
        assert_eq!(page.selected_classifier, 1);
        page.handle_key(make_key(KeyCode::Down));
        assert_eq!(page.selected_classifier, 1); // clamped
        page.handle_key(make_key(KeyCode::Up));
        assert_eq!(page.selected_classifier, 0);
    }

    #[test]
    fn tab_switches_section() {
        let mut page = ClassifiersPage::new();
        assert_eq!(page.section, Section::Classifiers);
        page.handle_key(make_key(KeyCode::Tab));
        assert_eq!(page.section, Section::Results);
        page.handle_key(make_key(KeyCode::Tab));
        assert_eq!(page.section, Section::Classifiers);
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
    fn space_emits_toggle_action() {
        let mut page = ClassifiersPage::new();
        page.set_classifiers(sample_classifiers());
        page.handle_key(make_key(KeyCode::Down)); // select second

        let action = page.handle_key(make_key(KeyCode::Char(' ')));
        assert!(
            matches!(action, Some(WidgetAction::ToggleClassifier(name)) if name == "cost-alert")
        );
    }

    #[test]
    fn enter_in_results_section_does_nothing() {
        let mut page = ClassifiersPage::new();
        page.set_classifiers(sample_classifiers());
        page.handle_key(make_key(KeyCode::Tab)); // switch to results

        let action = page.handle_key(make_key(KeyCode::Enter));
        assert!(action.is_none());
    }
}
