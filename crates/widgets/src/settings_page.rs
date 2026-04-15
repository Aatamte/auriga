use crate::{RenderContext, Widget, WidgetAction};
use auriga_core::ScrollDirection;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;
use std::time::SystemTime;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum SettingsSection {
    General,
    ClaudeSettings,
}

impl SettingsSection {
    pub const ALL: [SettingsSection; 2] =
        [SettingsSection::General, SettingsSection::ClaudeSettings];

    pub fn label(self) -> &'static str {
        match self {
            SettingsSection::General => "General",
            SettingsSection::ClaudeSettings => "claude-settings",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum FocusArea {
    Sections,
    Fields,
}

impl FocusArea {
    fn next(self) -> Self {
        match self {
            FocusArea::Sections => FocusArea::Fields,
            FocusArea::Fields => FocusArea::Fields,
        }
    }

    fn prev(self) -> Self {
        match self {
            FocusArea::Sections => FocusArea::Sections,
            FocusArea::Fields => FocusArea::Sections,
        }
    }
}

pub enum FieldKind {
    Text,
    Toggle(Vec<String>),
}

pub struct SettingsField {
    pub section: SettingsSection,
    pub label: &'static str,
    pub key: &'static str,
    pub value: String,
    pub description: &'static str,
    pub kind: FieldKind,
    pub detail: Vec<(String, String)>,
}

pub struct SettingsPage {
    pub fields: Vec<SettingsField>,
    pub selected: Option<usize>,
    pub editing: bool,
    pub edit_buffer: String,
    pub dirty: bool,
    pub save_message: Option<String>,
    pub file_path: String,
    pub last_loaded_at: Option<SystemTime>,
    focus: FocusArea,
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
            file_path: ".auriga/settings.json".into(),
            last_loaded_at: None,
            focus: FocusArea::Fields,
        }
    }

    pub fn sync_from_disk(
        &mut self,
        fields: Vec<SettingsField>,
        file_path: String,
        modified_at: Option<SystemTime>,
    ) {
        self.file_path = file_path;
        if self.dirty {
            return;
        }
        if modified_at == self.last_loaded_at && !self.fields.is_empty() {
            return;
        }
        self.apply_fields(fields, modified_at);
    }

    pub fn force_reload(
        &mut self,
        fields: Vec<SettingsField>,
        file_path: String,
        modified_at: Option<SystemTime>,
    ) {
        self.file_path = file_path;
        self.editing = false;
        self.edit_buffer.clear();
        self.dirty = false;
        self.apply_fields(fields, modified_at);
    }

    fn apply_fields(&mut self, fields: Vec<SettingsField>, modified_at: Option<SystemTime>) {
        let selected_key = self.selected_field().map(|f| f.key);
        self.fields = fields;
        self.last_loaded_at = modified_at;
        self.selected = selected_key
            .and_then(|key| self.fields.iter().position(|f| f.key == key))
            .or_else(|| (!self.fields.is_empty()).then_some(0));
        if self.selected.is_none() {
            self.focus = FocusArea::Sections;
        }
    }

    pub fn field_values(&self) -> Vec<(&str, &str)> {
        self.fields
            .iter()
            .map(|f| (f.key, f.value.as_str()))
            .collect()
    }

    pub fn mark_saved(&mut self, modified_at: Option<SystemTime>) {
        self.dirty = false;
        self.last_loaded_at = modified_at;
        self.save_message = Some("saved".into());
    }

    fn selected_field(&self) -> Option<&SettingsField> {
        self.selected.and_then(|idx| self.fields.get(idx))
    }

    fn selected_field_mut(&mut self) -> Option<&mut SettingsField> {
        self.selected.and_then(|idx| self.fields.get_mut(idx))
    }

    fn selected_section(&self) -> SettingsSection {
        self.selected_field()
            .map(|f| f.section)
            .unwrap_or(SettingsSection::General)
    }

    fn selected_section_idx(&self) -> usize {
        SettingsSection::ALL
            .iter()
            .position(|section| *section == self.selected_section())
            .unwrap_or(0)
    }

    fn section_indices(&self, section: SettingsSection) -> Vec<usize> {
        self.fields
            .iter()
            .enumerate()
            .filter_map(|(idx, f)| (f.section == section).then_some(idx))
            .collect()
    }

    fn first_index_in_section(&self, section: SettingsSection) -> Option<usize> {
        self.fields.iter().position(|f| f.section == section)
    }

    fn move_section(&mut self, delta: isize) {
        let current = self.selected_section_idx();
        let next =
            (current as isize + delta).clamp(0, SettingsSection::ALL.len() as isize - 1) as usize;
        if let Some(idx) = self.first_index_in_section(SettingsSection::ALL[next]) {
            self.selected = Some(idx);
        }
        self.editing = false;
        self.edit_buffer.clear();
    }

    fn move_within_section(&mut self, delta: isize) {
        let section = self.selected_section();
        let indices = self.section_indices(section);
        if indices.is_empty() {
            return;
        }
        let current = self.selected.unwrap_or(indices[0]);
        let pos = indices.iter().position(|&idx| idx == current).unwrap_or(0);
        let next = (pos as isize + delta).clamp(0, indices.len() as isize - 1) as usize;
        self.selected = Some(indices[next]);
    }

    fn cycle_selected_toggle(&mut self, delta: isize) {
        let Some(idx) = self.selected else { return };
        let Some(field) = self.fields.get_mut(idx) else {
            return;
        };
        let FieldKind::Toggle(options) = &field.kind else {
            return;
        };
        if options.is_empty() {
            return;
        }
        let next = match options.iter().position(|o| *o == field.value) {
            Some(pos) => (pos as isize + delta).rem_euclid(options.len() as isize) as usize,
            None => 0,
        };
        field.value = options[next].clone();
        self.dirty = true;
        self.save_message = None;
    }

    fn begin_edit(&mut self) {
        let Some(idx) = self.selected else { return };
        if let Some(value) = self.fields.get(idx).map(|field| field.value.clone()) {
            self.editing = true;
            self.edit_buffer = value;
        }
    }

    fn cancel_edit(&mut self) {
        self.editing = false;
        self.edit_buffer.clear();
    }

    fn commit_edit(&mut self) {
        let buffer = self.edit_buffer.clone();
        if let Some(field) = self.selected_field_mut() {
            field.value = buffer;
            self.dirty = true;
            self.save_message = None;
        }
        self.editing = false;
        self.edit_buffer.clear();
    }

    fn save_action(&mut self) -> Option<WidgetAction> {
        if self.editing {
            self.commit_edit();
        }

        if let Some((label, error)) = self.first_validation_error() {
            self.save_message = Some(format!("{label}: {error}"));
            return None;
        }

        self.dirty.then_some(WidgetAction::SaveConfig)
    }

    fn first_validation_error(&self) -> Option<(&'static str, String)> {
        self.fields.iter().find_map(|field| {
            validate_field(field.key, &field.value)
                .err()
                .map(|error| (field.label, error))
        })
    }

    fn activate_current(&mut self) -> Option<WidgetAction> {
        match self.focus {
            FocusArea::Sections => {
                self.focus = FocusArea::Fields;
                None
            }
            FocusArea::Fields => {
                let field = self.selected_field()?;
                match field.kind {
                    FieldKind::Text => {
                        self.begin_edit();
                        None
                    }
                    FieldKind::Toggle(_) => {
                        self.cycle_selected_toggle(1);
                        None
                    }
                }
            }
        }
    }

    pub fn handle_key(&mut self, key: KeyEvent) -> Option<WidgetAction> {
        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('s') {
            return self.save_action();
        }

        if self.editing {
            self.handle_edit_key(key)
        } else {
            self.handle_nav_key(key)
        }
    }

    fn handle_edit_key(&mut self, key: KeyEvent) -> Option<WidgetAction> {
        match key.code {
            KeyCode::Esc => self.cancel_edit(),
            KeyCode::Enter => self.commit_edit(),
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
            KeyCode::Left => self.focus = self.focus.prev(),
            KeyCode::Right | KeyCode::Tab => self.focus = self.focus.next(),
            KeyCode::BackTab => self.focus = self.focus.prev(),
            KeyCode::Up => match self.focus {
                FocusArea::Sections => self.move_section(-1),
                FocusArea::Fields => self.move_within_section(-1),
            },
            KeyCode::Down => match self.focus {
                FocusArea::Sections => self.move_section(1),
                FocusArea::Fields => self.move_within_section(1),
            },
            KeyCode::Enter => return self.activate_current(),
            _ => {}
        }
        None
    }

    fn pane_style(&self, focus: FocusArea) -> Style {
        if self.focus == focus {
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::DarkGray)
        }
    }

    fn status_text(&self) -> (String, Color) {
        if let Some(msg) = &self.save_message {
            if msg == "saved" {
                return ("saved".into(), Color::Green);
            }
            return (msg.clone(), Color::Red);
        }
        if self.dirty {
            ("modified".into(), Color::Yellow)
        } else {
            ("clean".into(), Color::DarkGray)
        }
    }

    fn section_row_style(&self, section: SettingsSection) -> Style {
        if section == self.selected_section() {
            let base = if self.focus == FocusArea::Sections {
                Color::Black
            } else {
                Color::Cyan
            };
            Style::default()
                .fg(base)
                .bg(Color::White)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::White)
        }
    }

    fn field_row_style(&self, idx: usize) -> Style {
        if Some(idx) == self.selected {
            if self.focus == FocusArea::Fields {
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Cyan)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD)
            }
        } else {
            Style::default().fg(Color::White)
        }
    }

    fn render_header(&self, frame: &mut Frame, area: Rect) {
        let block = Block::default()
            .title(" Settings ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::DarkGray));
        let inner = block.inner(area);
        frame.render_widget(block, area);
        if inner.height == 0 || inner.width == 0 {
            return;
        }

        let (status, status_color) = self.status_text();
        let mut lines: Vec<Line> = Vec::new();
        lines.push(Line::from(vec![
            Span::styled(
                status,
                Style::default()
                    .fg(status_color)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw("   "),
            Span::styled(self.file_path.clone(), Style::default().fg(Color::DarkGray)),
        ]));
        lines.push(Line::styled(
            if self.editing {
                "Enter apply   Esc cancel   Ctrl+S save"
            } else {
                "Arrows move   Enter edit/apply   Ctrl+S save"
            },
            Style::default().fg(Color::DarkGray),
        ));
        frame.render_widget(Paragraph::new(lines), inner);
    }

    fn render_sections(&self, frame: &mut Frame, area: Rect) {
        let block = Block::default()
            .title(" Sections ")
            .borders(Borders::ALL)
            .border_style(self.pane_style(FocusArea::Sections));
        let inner = block.inner(area);
        frame.render_widget(block, area);
        if inner.height == 0 || inner.width == 0 {
            return;
        }

        let lines: Vec<Line> = SettingsSection::ALL
            .iter()
            .map(|section| {
                let prefix = if *section == self.selected_section() {
                    ">"
                } else {
                    " "
                };
                Line::styled(
                    format!("{prefix} {}", section.label()),
                    self.section_row_style(*section),
                )
            })
            .collect();
        frame.render_widget(Paragraph::new(lines), inner);
    }

    fn render_fields(&self, frame: &mut Frame, area: Rect) {
        let block = Block::default()
            .title(" Fields ")
            .borders(Borders::ALL)
            .border_style(self.pane_style(FocusArea::Fields));
        let inner = block.inner(area);
        frame.render_widget(block, area);
        if inner.height == 0 || inner.width == 0 {
            return;
        }

        let width = inner.width as usize;
        let section = self.selected_section();
        let lines: Vec<Line> = self
            .section_indices(section)
            .into_iter()
            .filter_map(|idx| {
                self.fields
                    .get(idx)
                    .map(|field| self.field_line(idx, field, width))
            })
            .collect();
        frame.render_widget(Paragraph::new(lines), inner);
    }

    fn field_line(&self, idx: usize, field: &SettingsField, width: usize) -> Line<'static> {
        let is_selected = Some(idx) == self.selected;
        let prefix = if is_selected { ">" } else { " " };
        let label_text = format!("{prefix} {}", field.label);

        let value_text = if is_selected && self.editing {
            format!(" {}▎ ", self.edit_buffer)
        } else {
            format!(" {} ", field.value)
        };

        let label_style = self.field_row_style(idx);
        let value_style = if is_selected && self.editing {
            Style::default()
                .fg(Color::Black)
                .bg(Color::Green)
                .add_modifier(Modifier::BOLD)
        } else if is_selected && self.focus == FocusArea::Fields {
            label_style
        } else {
            Style::default().fg(Color::Green)
        };

        let used = label_text.chars().count() + value_text.chars().count();
        let pad = width.saturating_sub(used);
        let spaces = " ".repeat(pad);

        Line::from(vec![
            Span::styled(label_text, label_style),
            Span::styled(spaces, label_style),
            Span::styled(value_text, value_style),
        ])
    }
}

fn validate_field(key: &str, value: &str) -> Result<(), String> {
    match key {
        "mcp_port" => value
            .parse::<u16>()
            .map(|_| ())
            .map_err(|_| "must be a valid port".into()),
        "font_size" => match value.parse::<u16>() {
            Ok(size) if (8..=32).contains(&size) => Ok(()),
            _ => Err("must be between 8 and 32".into()),
        },
        "claude.max_budget_usd" => {
            let trimmed = value.trim();
            if trimmed.is_empty() || trimmed == "—" {
                return Ok(());
            }
            match trimmed.parse::<f64>() {
                Ok(n) if n > 0.0 => Ok(()),
                _ => Err("must be a positive number or empty".into()),
            }
        }
        _ => Ok(()),
    }
}

impl Default for SettingsPage {
    fn default() -> Self {
        Self::new()
    }
}

impl Widget for SettingsPage {
    fn render(&mut self, frame: &mut Frame, area: Rect, _ctx: &RenderContext) {
        if area.width < 40 || area.height < 10 {
            let block = Block::default()
                .title(" Settings ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::DarkGray));
            frame.render_widget(block, area);
            return;
        }

        let rows = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(4), Constraint::Min(6)])
            .split(area);

        self.render_header(frame, rows[0]);

        let cols = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Length(16), Constraint::Min(20)])
            .split(rows[1]);

        self.render_sections(frame, cols[0]);
        self.render_fields(frame, cols[1]);
    }

    fn handle_scroll(&mut self, _direction: ScrollDirection) {}

    fn handle_click(&mut self, _row: u16, col: u16, _ctx: &RenderContext) -> Option<WidgetAction> {
        self.focus = if col < 16 {
            FocusArea::Sections
        } else {
            FocusArea::Fields
        };
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::{KeyEventKind, KeyEventState};
    use std::time::UNIX_EPOCH;

    fn key(code: KeyCode, modifiers: KeyModifiers) -> KeyEvent {
        KeyEvent {
            code,
            modifiers,
            kind: KeyEventKind::Press,
            state: KeyEventState::NONE,
        }
    }

    fn sample_page() -> SettingsPage {
        let mut page = SettingsPage::new();
        page.force_reload(
            vec![
                SettingsField {
                    section: SettingsSection::General,
                    label: "MCP Port",
                    key: "mcp_port",
                    value: "7850".into(),
                    description: "Port",
                    kind: FieldKind::Text,
                    detail: vec![],
                },
                SettingsField {
                    section: SettingsSection::General,
                    label: "Font Size",
                    key: "font_size",
                    value: "10".into(),
                    description: "Font",
                    kind: FieldKind::Text,
                    detail: vec![],
                },
                SettingsField {
                    section: SettingsSection::ClaudeSettings,
                    label: "Model",
                    key: "claude.model",
                    value: "default".into(),
                    description: "Model",
                    kind: FieldKind::Toggle(vec!["default".into(), "sonnet".into(), "opus".into()]),
                    detail: vec![],
                },
            ],
            ".auriga/settings.json".into(),
            None,
        );
        page
    }

    #[test]
    fn right_moves_focus_from_sections_to_fields() {
        let mut page = sample_page();
        page.focus = FocusArea::Sections;
        page.handle_key(key(KeyCode::Right, KeyModifiers::NONE));
        assert_eq!(page.focus, FocusArea::Fields);
        page.handle_key(key(KeyCode::Right, KeyModifiers::NONE));
        assert_eq!(page.focus, FocusArea::Fields);
    }

    #[test]
    fn left_moves_focus_from_fields_to_sections() {
        let mut page = sample_page();
        page.focus = FocusArea::Fields;
        page.handle_key(key(KeyCode::Left, KeyModifiers::NONE));
        assert_eq!(page.focus, FocusArea::Sections);
    }

    #[test]
    fn section_focus_moves_selected_section() {
        let mut page = sample_page();
        page.focus = FocusArea::Sections;
        page.selected = Some(0);
        page.handle_key(key(KeyCode::Down, KeyModifiers::NONE));
        assert_eq!(page.selected, Some(2));
    }

    #[test]
    fn fields_focus_moves_within_section() {
        let mut page = sample_page();
        page.focus = FocusArea::Fields;
        page.selected = Some(0);
        page.handle_key(key(KeyCode::Down, KeyModifiers::NONE));
        assert_eq!(page.selected, Some(1));
    }

    #[test]
    fn enter_on_toggle_field_cycles_value() {
        let mut page = sample_page();
        page.focus = FocusArea::Fields;
        page.selected = Some(2);
        page.handle_key(key(KeyCode::Enter, KeyModifiers::NONE));
        assert_eq!(page.fields[2].value, "sonnet");
        assert!(page.dirty);
    }

    #[test]
    fn enter_on_text_field_begins_edit() {
        let mut page = sample_page();
        page.focus = FocusArea::Fields;
        page.selected = Some(0);
        page.handle_key(key(KeyCode::Enter, KeyModifiers::NONE));
        assert!(page.editing);
        assert_eq!(page.edit_buffer, "7850");
    }

    #[test]
    fn ctrl_s_commits_edit_and_saves() {
        let mut page = sample_page();
        page.selected = Some(0);
        page.begin_edit();
        page.edit_buffer = "9999".into();
        let action = page.handle_key(key(KeyCode::Char('s'), KeyModifiers::CONTROL));
        assert!(matches!(action, Some(WidgetAction::SaveConfig)));
        assert_eq!(page.fields[0].value, "9999");
    }

    #[test]
    fn invalid_save_is_blocked() {
        let mut page = sample_page();
        page.selected = Some(0);
        page.begin_edit();
        page.edit_buffer = "abc".into();
        let action = page.handle_key(key(KeyCode::Char('s'), KeyModifiers::CONTROL));
        assert!(action.is_none());
        assert_eq!(
            page.save_message.as_deref(),
            Some("MCP Port: must be a valid port")
        );
    }

    #[test]
    fn sync_from_disk_preserves_unsaved_edits() {
        let mut page = sample_page();
        page.dirty = true;
        page.last_loaded_at = Some(UNIX_EPOCH);
        let before = page.fields[0].value.clone();
        page.sync_from_disk(
            vec![],
            ".auriga/settings.json".into(),
            Some(UNIX_EPOCH + std::time::Duration::from_secs(1)),
        );
        assert_eq!(page.fields[0].value, before);
        assert!(page.dirty);
    }
}
