use crate::{RenderContext, Widget, WidgetAction};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use orchestrator_core::ScrollDirection;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;

const LABEL_WIDTH: usize = 20;

pub struct SettingsField {
    pub label: &'static str,
    pub key: &'static str,
    pub value: String,
    pub description: &'static str,
}

pub struct SettingsPage {
    pub fields: Vec<SettingsField>,
    pub selected: Option<usize>,
    pub editing: bool,
    pub edit_buffer: String,
    pub dirty: bool,
    pub save_message: Option<&'static str>,
}

impl SettingsPage {
    pub fn new() -> Self {
        Self {
            fields: Vec::new(),
            selected: None,
            editing: false,
            edit_buffer: String::new(),
            dirty: false,
            save_message: None,
        }
    }

    /// Set the fields from external config data.
    pub fn set_fields(&mut self, fields: Vec<SettingsField>) {
        self.fields = fields;
    }

    /// Get current field values as (key, value) pairs.
    pub fn field_values(&self) -> Vec<(&str, &str)> {
        self.fields.iter().map(|f| (f.key, f.value.as_str())).collect()
    }

    /// Handle a key event. Returns a WidgetAction if one should be dispatched.
    pub fn handle_key(&mut self, key: KeyEvent) -> Option<WidgetAction> {
        // Ctrl+S saves
        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('s') {
            if self.dirty {
                return Some(WidgetAction::SaveConfig);
            }
            return None;
        }

        if self.editing {
            self.handle_edit_key(key)
        } else {
            self.handle_nav_key(key)
        }
    }

    fn handle_edit_key(&mut self, key: KeyEvent) -> Option<WidgetAction> {
        match key.code {
            KeyCode::Esc => {
                self.editing = false;
                self.edit_buffer.clear();
            }
            KeyCode::Enter => {
                if let Some(idx) = self.selected {
                    if idx < self.fields.len() {
                        self.fields[idx].value = self.edit_buffer.clone();
                        self.dirty = true;
                        self.save_message = None;
                    }
                }
                self.editing = false;
                self.edit_buffer.clear();
            }
            KeyCode::Backspace => {
                self.edit_buffer.pop();
            }
            KeyCode::Char(c) => {
                self.edit_buffer.push(c);
            }
            _ => {}
        }
        None
    }

    fn handle_nav_key(&mut self, key: KeyEvent) -> Option<WidgetAction> {
        match key.code {
            KeyCode::Up => {
                if let Some(idx) = self.selected {
                    if idx > 0 {
                        self.selected = Some(idx - 1);
                    }
                } else if !self.fields.is_empty() {
                    self.selected = Some(0);
                }
            }
            KeyCode::Down => {
                if let Some(idx) = self.selected {
                    if idx + 1 < self.fields.len() {
                        self.selected = Some(idx + 1);
                    }
                } else if !self.fields.is_empty() {
                    self.selected = Some(0);
                }
            }
            KeyCode::Enter => {
                if let Some(idx) = self.selected {
                    if idx < self.fields.len() {
                        self.editing = true;
                        self.edit_buffer = self.fields[idx].value.clone();
                    }
                }
            }
            _ => {}
        }
        None
    }

    pub fn mark_saved(&mut self) {
        self.dirty = false;
        self.save_message = Some("Saved!");
    }
}

impl Default for SettingsPage {
    fn default() -> Self {
        Self::new()
    }
}

impl Widget for SettingsPage {
    fn render(&mut self, frame: &mut Frame, area: Rect, _ctx: &RenderContext) {
        let block = Block::default()
            .title(" Settings ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::DarkGray));

        let inner = block.inner(area);
        frame.render_widget(block, area);

        if inner.height == 0 || inner.width == 0 {
            return;
        }

        let mut lines: Vec<Line> = Vec::new();
        lines.push(Line::from(""));

        for (i, field) in self.fields.iter().enumerate() {
            let is_selected = self.selected == Some(i);
            let is_editing = is_selected && self.editing;

            // Label
            let label_style = if is_selected {
                Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::White)
            };

            let padded_label = format!("  {:<width$}", field.label, width = LABEL_WIDTH);

            // Value
            let value_display = if is_editing {
                format!("{}▎", self.edit_buffer)
            } else {
                field.value.clone()
            };

            let value_style = if is_editing {
                Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
            } else if is_selected {
                Style::default().fg(Color::Cyan)
            } else {
                Style::default().fg(Color::Green)
            };

            lines.push(Line::from(vec![
                Span::styled(padded_label, label_style),
                Span::styled(value_display, value_style),
            ]));

            // Description
            let desc_pad = format!("  {:<width$}", "", width = LABEL_WIDTH);
            lines.push(Line::from(vec![
                Span::raw(desc_pad),
                Span::styled(field.description, Style::default().fg(Color::DarkGray)),
            ]));

            lines.push(Line::from(""));
        }

        // Status line
        if self.dirty {
            lines.push(Line::from(vec![
                Span::raw("  "),
                Span::styled(
                    "[Save] Ctrl+S",
                    Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
                ),
                Span::styled("  unsaved changes", Style::default().fg(Color::DarkGray)),
            ]));
        } else if let Some(msg) = self.save_message {
            lines.push(Line::from(vec![
                Span::raw("  "),
                Span::styled(msg, Style::default().fg(Color::Green)),
            ]));
        }

        // Help
        lines.push(Line::from(""));
        let help = if self.editing {
            "  Enter confirm  │  Esc cancel"
        } else {
            "  ↑↓ select  │  Enter edit  │  Ctrl+S save"
        };
        lines.push(Line::styled(help, Style::default().fg(Color::DarkGray)));

        let paragraph = Paragraph::new(lines);
        frame.render_widget(paragraph, inner);
    }

    fn handle_scroll(&mut self, _direction: ScrollDirection) {}

    fn handle_click(&mut self, row: u16, _col: u16, _ctx: &RenderContext) -> Option<WidgetAction> {
        // Each field takes 3 lines (label+value, description, blank), starting at line 1
        // row 0 = blank line
        // row 1 = field 0 label+value
        // row 2 = field 0 description
        // row 3 = blank
        // row 4 = field 1 label+value
        if row == 0 {
            return None;
        }
        let field_idx = (row as usize - 1) / 3;
        if field_idx < self.fields.len() {
            if self.selected == Some(field_idx) && !self.editing {
                // Second click starts editing
                self.editing = true;
                self.edit_buffer = self.fields[field_idx].value.clone();
            } else {
                self.selected = Some(field_idx);
                self.editing = false;
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::KeyEventState;

    fn make_key(code: KeyCode, modifiers: KeyModifiers) -> KeyEvent {
        KeyEvent {
            code,
            modifiers,
            kind: crossterm::event::KeyEventKind::Press,
            state: KeyEventState::NONE,
        }
    }

    fn page_with_field() -> SettingsPage {
        let mut page = SettingsPage::new();
        page.set_fields(vec![SettingsField {
            label: "MCP Port",
            key: "mcp_port",
            value: "7850".to_string(),
            description: "Port for MCP server",
        }]);
        page
    }

    #[test]
    fn new_page_has_no_selection() {
        let page = SettingsPage::new();
        assert!(page.selected.is_none());
        assert!(!page.editing);
        assert!(!page.dirty);
    }

    #[test]
    fn arrow_down_selects_first_field() {
        let mut page = page_with_field();
        page.handle_key(make_key(KeyCode::Down, KeyModifiers::NONE));
        assert_eq!(page.selected, Some(0));
    }

    #[test]
    fn enter_starts_editing_selected_field() {
        let mut page = page_with_field();
        page.selected = Some(0);
        page.handle_key(make_key(KeyCode::Enter, KeyModifiers::NONE));
        assert!(page.editing);
        assert_eq!(page.edit_buffer, "7850");
    }

    #[test]
    fn typing_modifies_edit_buffer() {
        let mut page = page_with_field();
        page.selected = Some(0);
        page.editing = true;
        page.edit_buffer = "785".to_string();
        page.handle_key(make_key(KeyCode::Char('1'), KeyModifiers::NONE));
        assert_eq!(page.edit_buffer, "7851");
    }

    #[test]
    fn backspace_removes_last_char() {
        let mut page = page_with_field();
        page.selected = Some(0);
        page.editing = true;
        page.edit_buffer = "785".to_string();
        page.handle_key(make_key(KeyCode::Backspace, KeyModifiers::NONE));
        assert_eq!(page.edit_buffer, "78");
    }

    #[test]
    fn enter_confirms_edit() {
        let mut page = page_with_field();
        page.selected = Some(0);
        page.editing = true;
        page.edit_buffer = "9000".to_string();
        page.handle_key(make_key(KeyCode::Enter, KeyModifiers::NONE));
        assert!(!page.editing);
        assert_eq!(page.fields[0].value, "9000");
        assert!(page.dirty);
    }

    #[test]
    fn esc_cancels_edit() {
        let mut page = page_with_field();
        page.selected = Some(0);
        page.editing = true;
        page.edit_buffer = "9000".to_string();
        page.handle_key(make_key(KeyCode::Esc, KeyModifiers::NONE));
        assert!(!page.editing);
        assert_eq!(page.fields[0].value, "7850"); // unchanged
        assert!(!page.dirty);
    }

    #[test]
    fn ctrl_s_returns_save_action_when_dirty() {
        let mut page = page_with_field();
        page.dirty = true;
        let action = page.handle_key(make_key(KeyCode::Char('s'), KeyModifiers::CONTROL));
        assert!(matches!(action, Some(WidgetAction::SaveConfig)));
    }

    #[test]
    fn ctrl_s_returns_none_when_clean() {
        let mut page = page_with_field();
        let action = page.handle_key(make_key(KeyCode::Char('s'), KeyModifiers::CONTROL));
        assert!(action.is_none());
    }

    #[test]
    fn mark_saved_clears_dirty() {
        let mut page = page_with_field();
        page.dirty = true;
        page.mark_saved();
        assert!(!page.dirty);
        assert_eq!(page.save_message, Some("Saved!"));
    }

    #[test]
    fn field_values_returns_pairs() {
        let page = page_with_field();
        let vals = page.field_values();
        assert_eq!(vals, vec![("mcp_port", "7850")]);
    }
}
