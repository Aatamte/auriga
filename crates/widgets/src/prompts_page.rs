use crate::{RenderContext, Widget, WidgetAction};
use auriga_core::{ScrollDirection, Scrollable};
use auriga_skills::SkillStatus;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;

#[derive(Debug, Clone)]
pub struct SystemPromptEntry {
    pub name: String,
    pub description: String,
    pub content: String,
    pub provider: String,
    pub enabled: bool,
}

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
        self.selected = if total == 0 {
            0
        } else {
            self.selected.min(total - 1)
        };
    }

    fn skills_start(&self) -> usize {
        self.system_prompts.len()
    }

    fn selected_item(&self) -> Option<(Section, usize)> {
        if self.selected < self.system_prompts.len() {
            Some((Section::SystemPrompts, self.selected))
        } else {
            let idx = self.selected.saturating_sub(self.skills_start());
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
                        Section::SystemPrompts => Some(WidgetAction::ToggleSystemPrompt(
                            self.system_prompts[idx].name.clone(),
                        )),
                        Section::Skills => Some(self.skill_toggle_action(idx)),
                    };
                }
            }
            KeyCode::Char('d') => {
                if let Some((Section::Skills, idx)) = self.selected_item() {
                    if !self.skills[idx].downloaded {
                        return Some(WidgetAction::DownloadSkill(self.skills[idx].name.clone()));
                    }
                }
            }
            KeyCode::Char('x') => {
                if let Some((Section::Skills, idx)) = self.selected_item() {
                    if self.skills[idx].downloaded {
                        return Some(WidgetAction::DeleteSkill(self.skills[idx].name.clone()));
                    }
                }
            }
            _ => {}
        }
        None
    }

    fn skill_toggle_action(&self, idx: usize) -> WidgetAction {
        let skill = &self.skills[idx];
        if skill.downloaded {
            WidgetAction::DeleteSkill(skill.name.clone())
        } else {
            WidgetAction::DownloadSkill(skill.name.clone())
        }
    }

    fn build_lines(&self, width: u16) -> Vec<Line<'static>> {
        let mut lines = Vec::new();

        lines.push(section_header(" System Prompts", width));
        if self.system_prompts.is_empty() {
            lines.push(Line::styled(
                "  No system prompts found",
                Style::default().fg(Color::DarkGray),
            ));
        } else {
            for (i, p) in self.system_prompts.iter().enumerate() {
                lines.extend(render_system_prompt_owned(p, self.selected == i, width));
            }
        }

        lines.push(Line::from(""));
        lines.push(section_header(" Skills", width));
        if self.skills.is_empty() {
            lines.push(Line::styled(
                "  No skills registered",
                Style::default().fg(Color::DarkGray),
            ));
        } else {
            let start = self.skills_start();
            for (i, s) in self.skills.iter().enumerate() {
                lines.extend(render_skill_owned(s, self.selected == start + i, width));
            }
        }

        lines.push(Line::from(""));
        lines.push(Line::styled(
            "  ↑↓ select  │  Enter toggle  │  d download  │  x delete",
            Style::default().fg(Color::DarkGray),
        ));
        lines
    }

    fn selected_line_offset(&self, width: u16) -> usize {
        let mut offset = 1;

        if self.system_prompts.is_empty() {
            if self.selected == 0 {
                return offset;
            }
            offset += 1;
        } else {
            for (i, p) in self.system_prompts.iter().enumerate() {
                if self.selected == i {
                    return offset;
                }
                offset += render_system_prompt_owned(p, false, width).len();
            }
        }

        offset += 2;

        if self.skills.is_empty() {
            return offset;
        }

        for (i, s) in self.skills.iter().enumerate() {
            if self.selected == self.skills_start() + i {
                return offset;
            }
            offset += render_skill_owned(s, false, width).len();
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

        let lines = self.build_lines(inner.width);
        self.scroll.set_item_count(lines.len());
        self.scroll.set_visible_height(inner.height as usize);
        self.scroll.select(self.selected_line_offset(inner.width));
        let range = self.scroll.visible_range();
        frame.render_widget(Paragraph::new(lines[range].to_vec()), inner);
    }

    fn handle_scroll(&mut self, direction: ScrollDirection) {
        self.scroll.scroll(direction);
    }

    fn handle_click(&mut self, row: u16, _col: u16, _ctx: &RenderContext) -> Option<WidgetAction> {
        let width = 80u16;
        let clicked = self.scroll.offset + row as usize;
        let mut offset = 1;

        if self.system_prompts.is_empty() {
            offset += 1;
        } else {
            for (i, p) in self.system_prompts.iter().enumerate() {
                let h = render_system_prompt_owned(p, false, width).len();
                if clicked >= offset && clicked < offset + h {
                    self.selected = i;
                    return Some(WidgetAction::ToggleSystemPrompt(
                        self.system_prompts[i].name.clone(),
                    ));
                }
                offset += h;
            }
        }

        offset += 2;

        if self.skills.is_empty() {
            return None;
        }

        let start = self.skills_start();
        for (i, s) in self.skills.iter().enumerate() {
            let h = render_skill_owned(s, false, width).len();
            if clicked >= offset && clicked < offset + h {
                self.selected = start + i;
                return Some(self.skill_toggle_action(i));
            }
            offset += h;
        }

        None
    }
}

fn section_header(title: &str, width: u16) -> Line<'static> {
    let mut line = Vec::new();
    line.push(Span::styled(
        title.to_string(),
        Style::default()
            .fg(Color::White)
            .add_modifier(Modifier::BOLD),
    ));
    let fill = width.saturating_sub(title.len() as u16 + 1) as usize;
    if fill > 0 {
        line.push(Span::styled(
            format!(" {}", "─".repeat(fill.saturating_sub(1))),
            Style::default().fg(Color::DarkGray),
        ));
    }
    Line::from(line)
}

fn render_system_prompt_owned(
    prompt: &SystemPromptEntry,
    selected: bool,
    width: u16,
) -> Vec<Line<'static>> {
    let checkbox = if prompt.enabled { "[x]" } else { "[ ]" };
    let check_color = if prompt.enabled {
        Color::Green
    } else {
        Color::DarkGray
    };
    let name_style = if selected {
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::White)
    };

    vec![
        Line::from(vec![
            Span::styled(checkbox, Style::default().fg(check_color)),
            Span::raw(" "),
            Span::styled(prompt.name.clone(), name_style),
            Span::styled(
                format!(" [{}]", prompt.provider),
                Style::default().fg(Color::DarkGray),
            ),
        ]),
        Line::from(vec![
            Span::raw("    "),
            Span::styled(
                prompt.description.clone(),
                Style::default().fg(Color::DarkGray),
            ),
        ]),
        Line::styled(
            format!("  {}", "─".repeat(width.saturating_sub(4) as usize)),
            Style::default().fg(Color::DarkGray),
        ),
    ]
}

fn render_skill_owned(skill: &SkillStatus, selected: bool, width: u16) -> Vec<Line<'static>> {
    let badge = if skill.downloaded { "[on]" } else { "[off]" };
    let badge_color = if skill.downloaded {
        Color::Green
    } else {
        Color::DarkGray
    };
    let name_style = if selected {
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::White)
    };

    vec![
        Line::from(vec![
            Span::styled(badge, Style::default().fg(badge_color)),
            Span::raw(" "),
            Span::styled(skill.name.clone(), name_style),
        ]),
        Line::from(vec![
            Span::raw("    "),
            Span::styled(
                skill.description.clone(),
                Style::default().fg(Color::DarkGray),
            ),
        ]),
        Line::styled(
            format!("  {}", "─".repeat(width.saturating_sub(4) as usize)),
            Style::default().fg(Color::DarkGray),
        ),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use auriga_skills::SkillStatus;

    fn prompt(name: &str) -> SystemPromptEntry {
        SystemPromptEntry {
            name: name.into(),
            description: "desc".into(),
            content: "content".into(),
            provider: "claude".into(),
            enabled: true,
        }
    }

    fn skill(name: &str, downloaded: bool) -> SkillStatus {
        SkillStatus {
            name: name.into(),
            description: "desc".into(),
            downloaded,
        }
    }

    #[test]
    fn navigation_crosses_sections() {
        let mut page = PromptsPage::new();
        page.set_system_prompts(vec![prompt("a"), prompt("b")]);
        page.set_skills(vec![skill("s1", false)]);
        page.handle_key(KeyEvent::from(KeyCode::Down));
        assert_eq!(page.selected, 1);
        page.handle_key(KeyEvent::from(KeyCode::Down));
        assert_eq!(page.selected, 2);
    }

    #[test]
    fn toggle_system_prompt() {
        let mut page = PromptsPage::new();
        page.set_system_prompts(vec![prompt("a")]);
        let action = page.handle_key(KeyEvent::from(KeyCode::Enter));
        assert!(matches!(action, Some(WidgetAction::ToggleSystemPrompt(_))));
    }

    #[test]
    fn enter_on_skill_toggles_download() {
        let mut page = PromptsPage::new();
        page.set_skills(vec![skill("s1", false)]);
        let action = page.handle_key(KeyEvent::from(KeyCode::Enter));
        assert!(matches!(action, Some(WidgetAction::DownloadSkill(_))));
    }

    #[test]
    fn delete_downloaded_skill() {
        let mut page = PromptsPage::new();
        page.set_skills(vec![skill("s1", true)]);
        let action = page.handle_key(KeyEvent::from(KeyCode::Char('x')));
        assert!(matches!(action, Some(WidgetAction::DeleteSkill(_))));
    }
}
