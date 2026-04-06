use crate::{RenderContext, Widget, WidgetAction};
use crossterm::event::{KeyCode, KeyEvent};
use orchestrator_core::{ScrollDirection, Scrollable};
use orchestrator_skills::{SkillStatus, SkillTrigger};
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;

// ---------------------------------------------------------------------------
// System prompt entry (loaded from .agent-orchestrator/prompts/*.json)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct SystemPromptEntry {
    pub name: String,
    pub description: String,
    pub content: String,
    pub provider: String,
    pub enabled: bool,
}

// ---------------------------------------------------------------------------
// Selectable item — unified index across both sections
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Section {
    SystemPrompts,
    Skills,
}

pub struct PromptsPage {
    pub system_prompts: Vec<SystemPromptEntry>,
    pub skills: Vec<SkillStatus>,
    selected: usize,
    scroll: Scrollable,
}

impl PromptsPage {
    pub fn new() -> Self {
        Self {
            system_prompts: Vec::new(),
            skills: Vec::new(),
            selected: 0,
            scroll: Scrollable::new(),
        }
    }

    pub fn set_system_prompts(&mut self, prompts: Vec<SystemPromptEntry>) {
        self.system_prompts = prompts;
        self.clamp_selection();
    }

    pub fn set_skills(&mut self, skills: Vec<SkillStatus>) {
        self.skills = skills;
        self.clamp_selection();
    }

    fn total_items(&self) -> usize {
        self.system_prompts.len() + self.skills.len()
    }

    fn clamp_selection(&mut self) {
        let total = self.total_items();
        if total == 0 {
            self.selected = 0;
        } else if self.selected >= total {
            self.selected = total - 1;
        }
    }

    /// Map a flat selection index to (section, index within section).
    fn selected_item(&self) -> Option<(Section, usize)> {
        if self.selected < self.system_prompts.len() {
            Some((Section::SystemPrompts, self.selected))
        } else {
            let idx = self.selected - self.system_prompts.len();
            if idx < self.skills.len() {
                Some((Section::Skills, idx))
            } else {
                None
            }
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
                let total = self.total_items();
                if total > 0 && self.selected + 1 < total {
                    self.selected += 1;
                }
            }
            KeyCode::Enter | KeyCode::Char(' ') => {
                if let Some((section, idx)) = self.selected_item() {
                    return match section {
                        Section::SystemPrompts => {
                            let name = self.system_prompts[idx].name.clone();
                            Some(WidgetAction::ToggleSystemPrompt(name))
                        }
                        Section::Skills => {
                            let name = self.skills[idx].name.clone();
                            Some(WidgetAction::ToggleSkill(name))
                        }
                    };
                }
            }
            _ => {}
        }
        None
    }

    fn build_lines(&self, width: u16) -> Vec<Line<'static>> {
        let mut lines: Vec<Line<'static>> = Vec::new();

        // --- System Prompts section ---
        lines.push(Line::styled(
            " System Prompts",
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        ));
        lines.push(Line::styled(
            format!(" {}", "─".repeat(width.saturating_sub(2) as usize)),
            Style::default().fg(Color::DarkGray),
        ));

        if self.system_prompts.is_empty() {
            lines.push(Line::styled(
                "  No system prompts found",
                Style::default().fg(Color::DarkGray),
            ));
        } else {
            for (i, p) in self.system_prompts.iter().enumerate() {
                let is_selected = self.selected == i;
                lines.extend(render_system_prompt_owned(p, is_selected, width));
            }
        }

        lines.push(Line::from(""));

        // --- Skills section ---
        lines.push(Line::styled(
            " Skills",
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        ));
        lines.push(Line::styled(
            format!(" {}", "─".repeat(width.saturating_sub(2) as usize)),
            Style::default().fg(Color::DarkGray),
        ));

        if self.skills.is_empty() {
            lines.push(Line::styled(
                "  No skills registered",
                Style::default().fg(Color::DarkGray),
            ));
        } else {
            let prompt_count = self.system_prompts.len();
            for (i, s) in self.skills.iter().enumerate() {
                let is_selected = self.selected == prompt_count + i;
                lines.extend(render_skill_owned(s, is_selected, width));
            }
        }

        // Help line
        lines.push(Line::from(""));
        lines.push(Line::styled(
            "  ↑↓ select  │  Enter/Space toggle",
            Style::default().fg(Color::DarkGray),
        ));

        lines
    }

    /// Compute the line offset of the currently selected item for scroll tracking.
    fn selected_line_offset(&self, width: u16) -> usize {
        let mut offset = 2; // section header + separator

        if self.system_prompts.is_empty() {
            offset += 1; // placeholder
        }

        if self.selected < self.system_prompts.len() {
            for i in 0..self.selected {
                offset += render_system_prompt_owned(&self.system_prompts[i], false, width).len();
            }
            return offset;
        }

        // Past system prompts section
        for p in &self.system_prompts {
            offset += render_system_prompt_owned(p, false, width).len();
        }
        offset += 3; // blank line + "Skills" header + separator

        if self.skills.is_empty() {
            offset += 1;
        }

        let skill_idx = self.selected - self.system_prompts.len();
        for i in 0..skill_idx {
            offset += render_skill_owned(&self.skills[i], false, width).len();
        }
        offset
    }
}

impl Default for PromptsPage {
    fn default() -> Self {
        Self::new()
    }
}

impl Widget for PromptsPage {
    fn render(&mut self, frame: &mut Frame, area: Rect, _ctx: &RenderContext) {
        let block = Block::default()
            .title(" Prompts ")
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
        self.scroll.select(self.selected_line_offset(inner.width));

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
        // Simple: map click to the flat item list
        let visible_row = self.scroll.offset + row as usize;
        let width = 80u16; // approximate for hit-testing

        let mut line_idx = 2; // section header + separator

        if self.system_prompts.is_empty() {
            line_idx += 1;
        } else {
            for (i, p) in self.system_prompts.iter().enumerate() {
                let h = render_system_prompt_owned(p, false, width).len();
                if visible_row < line_idx + h {
                    self.selected = i;
                    return Some(WidgetAction::ToggleSystemPrompt(p.name.clone()));
                }
                line_idx += h;
            }
        }

        line_idx += 3; // blank + skills header + separator

        if !self.skills.is_empty() {
            for (i, s) in self.skills.iter().enumerate() {
                let h = render_skill_owned(s, false, width).len();
                if visible_row < line_idx + h {
                    self.selected = self.system_prompts.len() + i;
                    return Some(WidgetAction::ToggleSkill(s.name.clone()));
                }
                line_idx += h;
            }
        }

        None
    }
}

// ---------------------------------------------------------------------------
// Rendering helpers
// ---------------------------------------------------------------------------

fn render_system_prompt_owned(
    p: &SystemPromptEntry,
    is_selected: bool,
    width: u16,
) -> Vec<Line<'static>> {
    let mut lines = Vec::new();

    let checkbox = if p.enabled { "[x]" } else { "[ ]" };
    let check_color = if p.enabled {
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
        Span::styled(p.name.clone(), name_style),
        Span::styled("  │  ", Style::default().fg(Color::DarkGray)),
        Span::styled(p.provider.clone(), Style::default().fg(Color::DarkGray)),
    ]));

    if !p.description.is_empty() {
        lines.push(Line::from(vec![
            Span::raw("    "),
            Span::styled(p.description.clone(), Style::default().fg(Color::DarkGray)),
        ]));
    }

    if !p.content.is_empty() {
        let preview = p.content.lines().next().unwrap_or("");
        let max = (width as usize).saturating_sub(8);
        let truncated = if preview.len() > max && max > 3 {
            format!("{}...", &preview[..max - 3])
        } else {
            preview.to_string()
        };
        lines.push(Line::from(vec![
            Span::raw("    "),
            Span::styled(
                truncated,
                Style::default()
                    .fg(Color::DarkGray)
                    .add_modifier(Modifier::DIM),
            ),
        ]));
    }

    lines.push(Line::styled(
        format!("  {}", "─".repeat(width.saturating_sub(4) as usize)),
        Style::default().fg(Color::DarkGray),
    ));

    lines
}

fn render_skill_owned(
    s: &SkillStatus,
    is_selected: bool,
    width: u16,
) -> Vec<Line<'static>> {
    let mut lines = Vec::new();

    let checkbox = if s.enabled { "[x]" } else { "[ ]" };
    let check_color = if s.enabled {
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

    let (trigger_label, trigger_color) = trigger_display(&s.trigger);

    lines.push(Line::from(vec![
        Span::styled(checkbox, Style::default().fg(check_color)),
        Span::raw(" "),
        Span::styled(s.name.clone(), name_style),
        Span::styled("  │  ", Style::default().fg(Color::DarkGray)),
        Span::styled(trigger_label, Style::default().fg(trigger_color)),
    ]));

    if !s.description.is_empty() {
        lines.push(Line::from(vec![
            Span::raw("    "),
            Span::styled(s.description.clone(), Style::default().fg(Color::DarkGray)),
        ]));
    }

    lines.push(Line::styled(
        format!("  {}", "─".repeat(width.saturating_sub(4) as usize)),
        Style::default().fg(Color::DarkGray),
    ));

    lines
}

fn trigger_display(trigger: &SkillTrigger) -> (&'static str, Color) {
    match trigger {
        SkillTrigger::OnDemand => ("on-demand", Color::Cyan),
        SkillTrigger::OnSessionStart => ("session-start", Color::Green),
        SkillTrigger::OnSessionEnd => ("session-end", Color::Yellow),
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

    fn sample_prompts() -> Vec<SystemPromptEntry> {
        vec![
            SystemPromptEntry {
                name: "default".into(),
                description: "Default coding assistant".into(),
                content: "You are a helpful coding assistant.".into(),
                provider: "claude".into(),
                enabled: true,
            },
            SystemPromptEntry {
                name: "reviewer".into(),
                description: "Code review focused".into(),
                content: "You review code for bugs and style issues.".into(),
                provider: "claude".into(),
                enabled: false,
            },
        ]
    }

    fn sample_skills() -> Vec<SkillStatus> {
        vec![
            SkillStatus {
                name: "file-search".into(),
                description: "Search files by content".into(),
                trigger: SkillTrigger::OnDemand,
                enabled: true,
            },
            SkillStatus {
                name: "session-init".into(),
                description: "Load context on session start".into(),
                trigger: SkillTrigger::OnSessionStart,
                enabled: false,
            },
        ]
    }

    #[test]
    fn new_page_empty() {
        let page = PromptsPage::new();
        assert!(page.system_prompts.is_empty());
        assert!(page.skills.is_empty());
    }

    #[test]
    fn set_data_updates_lists() {
        let mut page = PromptsPage::new();
        page.set_system_prompts(sample_prompts());
        page.set_skills(sample_skills());
        assert_eq!(page.system_prompts.len(), 2);
        assert_eq!(page.skills.len(), 2);
    }

    #[test]
    fn navigation_crosses_sections() {
        let mut page = PromptsPage::new();
        page.set_system_prompts(sample_prompts());
        page.set_skills(sample_skills());

        // Start at first system prompt
        assert_eq!(page.selected, 0);
        assert_eq!(page.selected_item(), Some((Section::SystemPrompts, 0)));

        // Down through system prompts
        page.handle_key(make_key(KeyCode::Down));
        assert_eq!(page.selected_item(), Some((Section::SystemPrompts, 1)));

        // Down into skills
        page.handle_key(make_key(KeyCode::Down));
        assert_eq!(page.selected_item(), Some((Section::Skills, 0)));

        page.handle_key(make_key(KeyCode::Down));
        assert_eq!(page.selected_item(), Some((Section::Skills, 1)));

        // Clamped at end
        page.handle_key(make_key(KeyCode::Down));
        assert_eq!(page.selected_item(), Some((Section::Skills, 1)));

        // Back up into system prompts
        page.handle_key(make_key(KeyCode::Up));
        page.handle_key(make_key(KeyCode::Up));
        assert_eq!(page.selected_item(), Some((Section::SystemPrompts, 1)));
    }

    #[test]
    fn toggle_system_prompt() {
        let mut page = PromptsPage::new();
        page.set_system_prompts(sample_prompts());
        page.set_skills(sample_skills());

        let action = page.handle_key(make_key(KeyCode::Enter));
        assert!(
            matches!(action, Some(WidgetAction::ToggleSystemPrompt(name)) if name == "default")
        );
    }

    #[test]
    fn toggle_skill_from_skills_section() {
        let mut page = PromptsPage::new();
        page.set_system_prompts(sample_prompts());
        page.set_skills(sample_skills());

        // Navigate past system prompts into skills
        page.handle_key(make_key(KeyCode::Down));
        page.handle_key(make_key(KeyCode::Down));
        let action = page.handle_key(make_key(KeyCode::Enter));
        assert!(
            matches!(action, Some(WidgetAction::ToggleSkill(name)) if name == "file-search")
        );
    }

    #[test]
    fn selected_clamped_on_shrink() {
        let mut page = PromptsPage::new();
        page.set_system_prompts(sample_prompts());
        page.set_skills(sample_skills());
        page.selected = 3; // last skill
        page.set_skills(Vec::new()); // remove all skills
        assert_eq!(page.selected, 1); // clamped to last system prompt
    }

    #[test]
    fn render_system_prompt_lines() {
        let p = &sample_prompts()[0];
        let lines = render_system_prompt_owned(p, false, 80);
        // header + description + content preview + separator = 4
        assert_eq!(lines.len(), 4);
    }

    #[test]
    fn render_skill_lines() {
        let s = &sample_skills()[0];
        let lines = render_skill_owned(s, false, 80);
        // header + description + separator = 3
        assert_eq!(lines.len(), 3);
    }

    #[test]
    fn trigger_display_values() {
        assert_eq!(trigger_display(&SkillTrigger::OnDemand).0, "on-demand");
        assert_eq!(trigger_display(&SkillTrigger::OnSessionStart).0, "session-start");
        assert_eq!(trigger_display(&SkillTrigger::OnSessionEnd).0, "session-end");
    }

    #[test]
    fn skills_only_no_prompts() {
        let mut page = PromptsPage::new();
        page.set_skills(sample_skills());
        assert_eq!(page.selected_item(), Some((Section::Skills, 0)));
    }

    #[test]
    fn prompts_only_no_skills() {
        let mut page = PromptsPage::new();
        page.set_system_prompts(sample_prompts());
        assert_eq!(page.selected_item(), Some((Section::SystemPrompts, 0)));
    }
}
