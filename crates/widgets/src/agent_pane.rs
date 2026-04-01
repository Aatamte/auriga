use crate::{format_tokens, RenderContext, Widget, WidgetAction};
use orchestrator_core::{AgentId, AgentStatus, ScrollDirection};
use ratatui::layout::{Alignment, Rect};
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::block::{Position, Title};
use ratatui::widgets::{Block, Borders};
use ratatui::Frame;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PaneMode {
    Grid,
    Focused,
}

pub struct AgentPaneWidget {
    pub mode: PaneMode,
    grid_rects: Vec<(AgentId, Rect)>,
    widget_origin: (u16, u16),
}

impl Default for AgentPaneWidget {
    fn default() -> Self {
        Self::new()
    }
}

impl AgentPaneWidget {
    pub fn new() -> Self {
        Self {
            mode: PaneMode::Grid,
            grid_rects: Vec::new(),
            widget_origin: (0, 0),
        }
    }

    pub fn set_mode(&mut self, mode: PaneMode) {
        self.mode = mode;
    }

    fn render_single_agent(
        &mut self,
        frame: &mut Frame,
        area: Rect,
        id: AgentId,
        ctx: &RenderContext,
        is_active: bool,
        show_back: bool,
    ) {
        let agent = ctx.agents.get(id);
        let agent_name = agent.map(|a| a.name.as_str()).unwrap_or("Unknown");
        let agent_status = agent.map(|a| a.status).unwrap_or(AgentStatus::Idle);

        // Status indicator (matches agent_list.rs pattern)
        let (dot, dot_color) = match agent_status {
            AgentStatus::Working => ("● ", Color::Green),
            AgentStatus::Idle => ("○ ", Color::DarkGray),
        };

        let back_prefix = if show_back { " ◀ " } else { " " };
        let title_left = Line::from(vec![
            Span::raw(back_prefix),
            Span::styled(dot, Style::default().fg(dot_color)),
            Span::raw(agent_name),
            Span::raw(" "),
        ]);

        // Model name (top-right)
        let model_str = ctx.traces.active_trace(id).and_then(|t| t.model.as_deref());
        let model_short = shorten_model(model_str);

        // Turn count (bottom-left)
        let turn_count = ctx.turns.agent_turn_count(id);

        // Token usage (bottom-right)
        let total_tokens = ctx.turns.agent_token_usage(id).total();

        let border_color = if is_active {
            Color::Cyan
        } else {
            Color::DarkGray
        };

        let mut block = Block::default()
            .title(title_left)
            .borders(Borders::ALL)
            .border_style(Style::default().fg(border_color));

        if !model_short.is_empty() {
            block = block.title(
                Title::from(Line::from(vec![
                    Span::raw(" "),
                    Span::styled(model_short, Style::default().fg(Color::DarkGray)),
                    Span::raw(" "),
                ]))
                .alignment(Alignment::Right),
            );
        }

        block = block.title_bottom(Line::from(vec![
            Span::raw(" T:"),
            Span::styled(turn_count.to_string(), Style::default().fg(Color::White)),
            Span::raw(" "),
        ]));

        if total_tokens > 0 {
            block = block.title(
                Title::from(Line::from(vec![
                    Span::raw(" tok:"),
                    Span::styled(
                        format_tokens(total_tokens),
                        Style::default().fg(Color::Yellow),
                    ),
                    Span::raw(" "),
                ]))
                .position(Position::Bottom)
                .alignment(Alignment::Right),
            );
        }

        let inner = block.inner(area);
        frame.render_widget(block, area);

        // Render terminal directly into the buffer
        (ctx.render_term)(id, frame.buffer_mut(), inner);
    }
}

/// Shorten a model name for compact display.
/// Strips "claude-" prefix: "claude-opus-4-6" → "opus-4-6"
fn shorten_model(model: Option<&str>) -> String {
    match model {
        Some(m) => m.strip_prefix("claude-").unwrap_or(m).to_string(),
        None => String::new(),
    }
}

/// Compute sub-rects for a 2-column grid layout that adapts to agent count
fn compute_grid_rects(area: Rect, count: usize) -> Vec<Rect> {
    if count == 0 {
        return vec![];
    }
    if count == 1 {
        return vec![area];
    }

    let cols = 2;
    let rows = count.div_ceil(2);
    let col_width = area.width / cols as u16;
    let row_height = area.height / rows as u16;

    let mut rects = Vec::with_capacity(count);
    for i in 0..count {
        let r = i / 2;
        let c = i % 2;

        let x = area.x + c as u16 * col_width;
        let y = area.y + r as u16 * row_height;

        let w = if c == 1 || count == i + 1 && count % 2 == 1 {
            (area.x + area.width).saturating_sub(x)
        } else {
            col_width
        };

        let h = if r == rows - 1 {
            (area.y + area.height).saturating_sub(y)
        } else {
            row_height
        };

        rects.push(Rect::new(x, y, w, h));
    }

    if count % 2 == 1 {
        if let Some(last) = rects.last_mut() {
            last.width = area.width;
        }
    }

    rects
}

impl Widget for AgentPaneWidget {
    fn render(&mut self, frame: &mut Frame, area: Rect, ctx: &RenderContext) {
        self.widget_origin = (area.x, area.y);

        match self.mode {
            PaneMode::Focused => {
                if let Some(id) = ctx.focus.active_agent {
                    self.render_single_agent(frame, area, id, ctx, true, true);
                } else {
                    let block = Block::default()
                        .title(" No Agent ")
                        .borders(Borders::ALL)
                        .border_style(Style::default().fg(Color::DarkGray));
                    frame.render_widget(block, area);
                }
                self.grid_rects.clear();
            }
            PaneMode::Grid => {
                let agents = ctx.agents.list();
                let display_count = agents.len().min(6);

                if display_count == 0 {
                    let block = Block::default()
                        .title(" No Agents ")
                        .borders(Borders::ALL)
                        .border_style(Style::default().fg(Color::DarkGray));
                    frame.render_widget(block, area);
                    self.grid_rects.clear();
                    return;
                }

                let sub_rects = compute_grid_rects(area, display_count);
                self.grid_rects.clear();

                for (i, rect) in sub_rects.iter().enumerate() {
                    let agent = &agents[i];
                    let is_active = ctx.focus.active_agent == Some(agent.id);
                    self.render_single_agent(frame, *rect, agent.id, ctx, is_active, false);
                    self.grid_rects.push((agent.id, *rect));
                }
            }
        }
    }

    fn handle_scroll(&mut self, _direction: ScrollDirection) {
        // Scrolling is handled by alacritty_terminal's scroll_display now
        // The app layer should call term.scroll_display() directly
    }

    fn handle_click(&mut self, row: u16, col: u16, _ctx: &RenderContext) -> Option<WidgetAction> {
        match self.mode {
            PaneMode::Focused => {
                if row == 0 && col < 4 {
                    return Some(WidgetAction::BackToGrid);
                }
                None
            }
            PaneMode::Grid => {
                let abs_row = row + self.widget_origin.1 + 1;
                let abs_col = col + self.widget_origin.0 + 1;

                for (agent_id, rect) in &self.grid_rects {
                    if abs_col >= rect.x
                        && abs_col < rect.x + rect.width
                        && abs_row >= rect.y
                        && abs_row < rect.y + rect.height
                    {
                        return Some(WidgetAction::FocusAgent(*agent_id));
                    }
                }
                None
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grid_rects_single_agent() {
        let area = Rect::new(0, 0, 100, 40);
        let rects = compute_grid_rects(area, 1);
        assert_eq!(rects.len(), 1);
        assert_eq!(rects[0], area);
    }

    #[test]
    fn grid_rects_two_agents() {
        let area = Rect::new(0, 0, 100, 40);
        let rects = compute_grid_rects(area, 2);
        assert_eq!(rects.len(), 2);
        assert_eq!(rects[0], Rect::new(0, 0, 50, 40));
        assert_eq!(rects[1], Rect::new(50, 0, 50, 40));
    }

    #[test]
    fn grid_rects_four_agents() {
        let area = Rect::new(0, 0, 100, 40);
        let rects = compute_grid_rects(area, 4);
        assert_eq!(rects.len(), 4);
        assert_eq!(rects[0], Rect::new(0, 0, 50, 20));
        assert_eq!(rects[1], Rect::new(50, 0, 50, 20));
        assert_eq!(rects[2], Rect::new(0, 20, 50, 20));
        assert_eq!(rects[3], Rect::new(50, 20, 50, 20));
    }

    #[test]
    fn grid_rects_six_agents() {
        let area = Rect::new(0, 0, 120, 60);
        let rects = compute_grid_rects(area, 6);
        assert_eq!(rects.len(), 6);
        assert_eq!(rects[0], Rect::new(0, 0, 60, 20));
        assert_eq!(rects[1], Rect::new(60, 0, 60, 20));
        assert_eq!(rects[4], Rect::new(0, 40, 60, 20));
        assert_eq!(rects[5], Rect::new(60, 40, 60, 20));
    }

    #[test]
    fn grid_rects_three_agents_odd() {
        let area = Rect::new(0, 0, 100, 40);
        let rects = compute_grid_rects(area, 3);
        assert_eq!(rects.len(), 3);
        assert_eq!(rects[0], Rect::new(0, 0, 50, 20));
        assert_eq!(rects[1], Rect::new(50, 0, 50, 20));
        assert_eq!(rects[2], Rect::new(0, 20, 100, 20));
    }

    #[test]
    fn grid_rects_zero_agents() {
        let rects = compute_grid_rects(Rect::new(0, 0, 100, 40), 0);
        assert!(rects.is_empty());
    }

    #[test]
    fn shorten_model_strips_claude_prefix() {
        assert_eq!(shorten_model(Some("claude-opus-4-6")), "opus-4-6");
        assert_eq!(shorten_model(Some("claude-sonnet-4-6")), "sonnet-4-6");
    }

    #[test]
    fn shorten_model_keeps_non_claude() {
        assert_eq!(shorten_model(Some("gpt-4o")), "gpt-4o");
    }

    #[test]
    fn shorten_model_none_returns_empty() {
        assert_eq!(shorten_model(None), "");
    }
}
