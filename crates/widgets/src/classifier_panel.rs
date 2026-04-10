use crate::{ClassifierStatusView, RenderContext, Widget, WidgetAction};
use orchestrator_core::ScrollDirection;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;

pub struct ClassifierPanelWidget {
    classifiers: Vec<ClassifierStatusView>,
}

impl ClassifierPanelWidget {
    pub fn new() -> Self {
        Self {
            classifiers: Vec::new(),
        }
    }

    pub fn set_classifiers(&mut self, classifiers: Vec<ClassifierStatusView>) {
        self.classifiers = classifiers;
    }
}

impl Default for ClassifierPanelWidget {
    fn default() -> Self {
        Self::new()
    }
}

impl Widget for ClassifierPanelWidget {
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
                "No classifiers",
                Style::default().fg(Color::DarkGray),
            )));
            frame.render_widget(placeholder, inner);
            return;
        }

        let lines: Vec<Line> = self
            .classifiers
            .iter()
            .take(inner.height as usize)
            .map(|c| {
                let (icon, color) = if c.enabled {
                    ("● ", Color::Green)
                } else {
                    ("○ ", Color::DarkGray)
                };
                let name_color = if c.enabled {
                    Color::White
                } else {
                    Color::DarkGray
                };
                Line::from(vec![
                    Span::styled(icon, Style::default().fg(color)),
                    Span::styled(&c.name, Style::default().fg(name_color)),
                ])
            })
            .collect();

        let paragraph = Paragraph::new(lines);
        frame.render_widget(paragraph, inner);
    }

    fn handle_scroll(&mut self, _direction: ScrollDirection) {}

    fn handle_click(&mut self, row: u16, _col: u16, _ctx: &RenderContext) -> Option<WidgetAction> {
        let idx = row as usize;
        if idx < self.classifiers.len() {
            Some(WidgetAction::ToggleClassifier(
                self.classifiers[idx].name.clone(),
            ))
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use orchestrator_core::{AgentStore, FileTree, FocusState, TraceStore, TurnStore};
    use std::path::PathBuf;

    macro_rules! with_ctx {
        (|$ctx:ident| $body:expr) => {{
            let agents = AgentStore::new();
            let turns = TurnStore::new();
            let traces = TraceStore::new();
            let focus = FocusState::new();
            let file_tree = FileTree::new(PathBuf::from("/tmp"));
            let noop = |_: orchestrator_core::AgentId, _: &mut ratatui::buffer::Buffer, _: Rect| {};
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
    fn new_panel_is_empty() {
        let panel = ClassifierPanelWidget::new();
        assert!(panel.classifiers.is_empty());
    }

    #[test]
    fn set_classifiers_updates_list() {
        let mut panel = ClassifierPanelWidget::new();
        panel.set_classifiers(vec![
            ClassifierStatusView {
                name: "clf-a".into(),
                trigger: "on_complete".into(),
                enabled: true,
            },
            ClassifierStatusView {
                name: "clf-b".into(),
                trigger: "on_complete".into(),
                enabled: false,
            },
        ]);
        assert_eq!(panel.classifiers.len(), 2);
    }

    #[test]
    fn click_valid_row_returns_toggle_action() {
        let mut panel = ClassifierPanelWidget::new();
        panel.set_classifiers(vec![
            ClassifierStatusView {
                name: "clf-a".into(),
                trigger: "on_complete".into(),
                enabled: true,
            },
            ClassifierStatusView {
                name: "clf-b".into(),
                trigger: "on_complete".into(),
                enabled: false,
            },
        ]);

        with_ctx!(|ctx| {
            let action = panel.handle_click(0, 0, &ctx);
            match action {
                Some(WidgetAction::ToggleClassifier(name)) => assert_eq!(name, "clf-a"),
                _ => panic!("expected ToggleClassifier"),
            }

            let action = panel.handle_click(1, 0, &ctx);
            match action {
                Some(WidgetAction::ToggleClassifier(name)) => assert_eq!(name, "clf-b"),
                _ => panic!("expected ToggleClassifier"),
            }
        });
    }

    #[test]
    fn click_out_of_range_returns_none() {
        let mut panel = ClassifierPanelWidget::new();
        panel.set_classifiers(vec![ClassifierStatusView {
            name: "clf-a".into(),
            trigger: "on_complete".into(),
            enabled: true,
        }]);

        with_ctx!(|ctx| {
            assert!(panel.handle_click(5, 0, &ctx).is_none());
        });
    }

    #[test]
    fn click_empty_panel_returns_none() {
        let mut panel = ClassifierPanelWidget::new();
        with_ctx!(|ctx| {
            assert!(panel.handle_click(0, 0, &ctx).is_none());
        });
    }

    #[test]
    fn scroll_is_noop() {
        let mut panel = ClassifierPanelWidget::new();
        // Should not panic
        panel.handle_scroll(ScrollDirection::Up);
        panel.handle_scroll(ScrollDirection::Down);
    }
}
