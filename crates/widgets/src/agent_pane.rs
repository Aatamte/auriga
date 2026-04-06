use crate::{format_tokens, RenderContext, Widget, WidgetAction};
use orchestrator_core::{
    AgentId, AgentStatus, ContentBlock, DisplayMode, MessageContent, ScrollDirection, Scrollable,
    TurnRole,
};
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;

// ---------------------------------------------------------------------------
// Provider info bar
// ---------------------------------------------------------------------------

/// Data passed to a provider's info bar renderer.
pub struct InfoBarData<'a> {
    pub agent_name: &'a str,
    pub model: &'a str,
    pub turn_count: usize,
    pub total_tokens: u64,
    pub status_dot: &'a str,
    pub status_color: Color,
    pub show_back: bool,
    pub system_prompt_name: Option<&'a str>,
}

/// Renders the provider-specific content inside the bottom info box.
pub trait ProviderInfoBar {
    fn render_spans<'a>(&self, data: &InfoBarData<'a>) -> Vec<Span<'a>>;
}

/// Claude-specific info bar.
pub struct ClaudeInfoBar;

impl ProviderInfoBar for ClaudeInfoBar {
    fn render_spans<'a>(&self, data: &InfoBarData<'a>) -> Vec<Span<'a>> {
        let back_prefix = if data.show_back { "◀ " } else { "" };
        let mut spans = vec![
            Span::raw(" "),
            Span::raw(back_prefix),
            Span::styled(data.status_dot, Style::default().fg(data.status_color)),
            Span::styled(data.agent_name, Style::default().fg(Color::White)),
        ];

        if !data.model.is_empty() {
            spans.push(Span::raw("    "));
            spans.push(Span::styled(
                data.model.to_string(),
                Style::default().fg(Color::DarkGray),
            ));
        }

        spans.push(Span::raw("    T:"));
        spans.push(Span::styled(
            data.turn_count.to_string(),
            Style::default().fg(Color::White),
        ));

        if data.total_tokens > 0 {
            spans.push(Span::raw("    tok:"));
            spans.push(Span::styled(
                format_tokens(data.total_tokens),
                Style::default().fg(Color::Yellow),
            ));
        }

        if let Some(prompt_name) = data.system_prompt_name {
            spans.push(Span::raw("    "));
            spans.push(Span::styled(
                prompt_name.to_string(),
                Style::default().fg(Color::Magenta),
            ));
        }

        spans.push(Span::raw(" "));
        spans
    }
}

/// Returns the info bar renderer for a given provider name.
fn info_bar_for_provider(_provider: &str) -> Box<dyn ProviderInfoBar> {
    // Currently only Claude. Add match arms for "codex", etc.
    Box::new(ClaudeInfoBar)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PaneMode {
    Grid,
    Focused,
}

pub struct AgentPaneWidget {
    pub mode: PaneMode,
    grid_rects: Vec<(AgentId, Rect)>,
    widget_origin: (u16, u16),
    timeline_scroll: Scrollable,
    pub input_buffer: String,
    pub generating: bool,
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
            timeline_scroll: Scrollable::new(),
            input_buffer: String::new(),
            generating: false,
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
        let agent_provider = agent.map(|a| a.provider.as_str()).unwrap_or("unknown");
        let agent_status = agent.map(|a| a.status).unwrap_or(AgentStatus::Idle);

        let (dot, dot_color) = match agent_status {
            AgentStatus::Working => ("● ", Color::Green),
            AgentStatus::Idle => ("○ ", Color::DarkGray),
        };

        // Model: prefer trace model, fall back to provider name
        let trace_model = ctx.traces.active_trace(id).and_then(|t| t.model.as_deref());
        let model_short = if trace_model.is_some() {
            shorten_model(trace_model)
        } else {
            agent_provider.to_string()
        };

        let turn_count = ctx.turns.agent_turn_count(id);
        let total_tokens = ctx.turns.agent_token_usage(id).total();

        let border_color = if is_active {
            Color::Cyan
        } else {
            Color::DarkGray
        };

        // Split area: content box on top, info box (3 rows) on bottom
        let info_height = 3u16.min(area.height);
        let content_area = Rect::new(
            area.x,
            area.y,
            area.width,
            area.height.saturating_sub(info_height),
        );
        let info_area = Rect::new(
            area.x,
            area.y + content_area.height,
            area.width,
            info_height,
        );

        // --- Top box: no bottom border (shared with info box top border) ---
        let content_block = Block::default()
            .borders(Borders::TOP | Borders::LEFT | Borders::RIGHT)
            .border_style(Style::default().fg(border_color));
        let content_inner = content_block.inner(content_area);
        frame.render_widget(content_block, content_area);

        let display_mode = ctx
            .agents
            .get(id)
            .map(|a| a.display_mode)
            .unwrap_or(DisplayMode::Provider);

        match display_mode {
            DisplayMode::Provider => {
                (ctx.render_term)(id, frame.buffer_mut(), content_inner);
            }
            DisplayMode::Native => {
                let input_height = 3u16.min(content_inner.height);
                let timeline_area = Rect::new(
                    content_inner.x,
                    content_inner.y,
                    content_inner.width,
                    content_inner.height.saturating_sub(input_height),
                );
                let input_area = Rect::new(
                    content_inner.x,
                    content_inner.y + timeline_area.height,
                    content_inner.width,
                    input_height,
                );
                self.render_timeline(frame, timeline_area, id, ctx);
                self.render_input_box(frame, input_area);
            }
        }

        // --- Bottom box: provider-specific info bar ---
        let prompt_name = agent.and_then(|a| a.system_prompt_name.as_deref());
        let info_bar = info_bar_for_provider(agent_provider);
        let data = InfoBarData {
            agent_name,
            model: &model_short,
            turn_count,
            total_tokens,
            status_dot: dot,
            status_color: dot_color,
            show_back,
            system_prompt_name: prompt_name,
        };
        let spans = info_bar.render_spans(&data);

        let info_block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(border_color));
        let info_inner = info_block.inner(info_area);
        frame.render_widget(info_block, info_area);

        let info_line = Paragraph::new(Line::from(spans));
        frame.render_widget(info_line, info_inner);
    }

    fn render_timeline(&mut self, frame: &mut Frame, area: Rect, id: AgentId, ctx: &RenderContext) {
        if area.height == 0 || area.width == 0 {
            return;
        }

        let turns = ctx.turns.turns_for(id);

        if turns.is_empty() {
            let line = Line::styled(
                " Waiting for activity...",
                Style::default().fg(Color::DarkGray),
            );
            frame.render_widget(Paragraph::new(vec![line]), area);
            return;
        }

        // Build lines from turns
        let max_width = area.width.saturating_sub(2) as usize;
        let mut lines: Vec<Line> = Vec::new();

        for turn in &turns {
            match turn.role {
                TurnRole::User => {
                    let text = content_to_text(&turn.content, max_width);
                    lines.push(Line::from(vec![
                        Span::styled(" > ", Style::default().fg(Color::Blue)),
                        Span::styled(text, Style::default().fg(Color::DarkGray)),
                    ]));
                }
                TurnRole::Assistant => {
                    let blocks = content_blocks(&turn.content);
                    for block in blocks {
                        match block {
                            ContentBlock::Text { ref text } => {
                                // Wrap long text into multiple lines
                                for chunk in wrap_text(text, max_width.saturating_sub(3)) {
                                    lines.push(Line::from(vec![
                                        Span::styled(" < ", Style::default().fg(Color::Green)),
                                        Span::styled(chunk, Style::default().fg(Color::White)),
                                    ]));
                                }
                            }
                            ContentBlock::ToolUse {
                                ref name,
                                ref input,
                                ..
                            } => {
                                let summary =
                                    tool_input_summary(input, max_width.saturating_sub(6));
                                lines.push(Line::from(vec![
                                    Span::styled("   @ ", Style::default().fg(Color::Yellow)),
                                    Span::styled(
                                        format!("{}({})", name, summary),
                                        Style::default().fg(Color::Yellow),
                                    ),
                                ]));
                            }
                            ContentBlock::ToolResult { is_error, .. } => {
                                let (label, color) = if *is_error {
                                    ("   ! error", Color::Red)
                                } else {
                                    ("   = done", Color::DarkGray)
                                };
                                lines.push(Line::styled(label, Style::default().fg(color)));
                            }
                            ContentBlock::Thinking { ref thinking, .. } => {
                                let truncated = truncate(thinking, max_width.saturating_sub(6));
                                lines.push(Line::from(vec![
                                    Span::styled(
                                        "   ~ ",
                                        Style::default()
                                            .fg(Color::Magenta)
                                            .add_modifier(Modifier::DIM),
                                    ),
                                    Span::styled(
                                        truncated,
                                        Style::default()
                                            .fg(Color::Magenta)
                                            .add_modifier(Modifier::DIM),
                                    ),
                                ]));
                            }
                            ContentBlock::Image { .. } => {
                                lines.push(Line::styled(
                                    "   [image]",
                                    Style::default().fg(Color::DarkGray),
                                ));
                            }
                        }
                    }
                }
            }
            // Blank separator between turns
            lines.push(Line::from(""));
        }

        // Scrolling: auto-scroll to bottom, but allow user to scroll up
        let visible_height = area.height as usize;
        self.timeline_scroll.set_item_count(lines.len());
        self.timeline_scroll.set_visible_height(visible_height);

        // Auto-scroll to bottom if user hasn't scrolled up
        if self.timeline_scroll.offset + visible_height >= lines.len().saturating_sub(1) {
            self.timeline_scroll.offset = lines.len().saturating_sub(visible_height);
        }

        let range = self.timeline_scroll.visible_range();
        let visible_lines: Vec<Line> = lines[range].to_vec();
        let paragraph = Paragraph::new(visible_lines);
        frame.render_widget(paragraph, area);
    }

    fn render_input_box(&self, frame: &mut Frame, area: Rect) {
        if area.height == 0 || area.width == 0 {
            return;
        }

        let block = Block::default()
            .borders(Borders::TOP)
            .border_style(Style::default().fg(Color::DarkGray));
        let inner = block.inner(area);
        frame.render_widget(block, area);

        if self.generating {
            let line = Line::from(vec![
                Span::styled(" ", Style::default()),
                Span::styled(
                    "Generating...",
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                ),
            ]);
            frame.render_widget(Paragraph::new(vec![line]), inner);
        } else {
            let max_w = inner.width.saturating_sub(3) as usize;
            let display = if self.input_buffer.len() > max_w {
                &self.input_buffer[self.input_buffer.len() - max_w..]
            } else {
                &self.input_buffer
            };
            let line = Line::from(vec![
                Span::styled(" > ", Style::default().fg(Color::Cyan)),
                Span::styled(display, Style::default().fg(Color::White)),
                Span::styled("▎", Style::default().fg(Color::Cyan)),
            ]);
            frame.render_widget(Paragraph::new(vec![line]), inner);
        }
    }
}

/// Extract text from MessageContent, truncated to max_width.
fn content_to_text(content: &MessageContent, max_width: usize) -> String {
    let raw = match content {
        MessageContent::Text(t) => t.clone(),
        MessageContent::Blocks(blocks) => blocks
            .iter()
            .filter_map(|b| match b {
                ContentBlock::Text { text } => Some(text.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join(" "),
    };
    // Collapse whitespace and truncate
    let collapsed: String = raw.split_whitespace().collect::<Vec<_>>().join(" ");
    truncate(&collapsed, max_width)
}

/// Get content blocks from MessageContent.
fn content_blocks(content: &MessageContent) -> Vec<&ContentBlock> {
    match content {
        MessageContent::Text(_) => vec![],
        MessageContent::Blocks(blocks) => blocks.iter().collect(),
    }
}

/// Truncate a string to max_len, adding "..." if truncated.
fn truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else if max_len > 3 {
        format!("{}...", &s[..max_len - 3])
    } else {
        s[..max_len].to_string()
    }
}

/// Wrap text into lines of at most max_width characters.
fn wrap_text(text: &str, max_width: usize) -> Vec<String> {
    if max_width == 0 {
        return vec![];
    }
    let collapsed: String = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if collapsed.is_empty() {
        return vec![];
    }
    let mut result = Vec::new();
    let mut remaining = collapsed.as_str();
    while !remaining.is_empty() {
        if remaining.len() <= max_width {
            result.push(remaining.to_string());
            break;
        }
        // Find last space within max_width
        let split_at = remaining[..max_width].rfind(' ').unwrap_or(max_width);
        result.push(remaining[..split_at].to_string());
        remaining = remaining[split_at..].trim_start();
    }
    result
}

/// Summarize tool input JSON for compact display.
fn tool_input_summary(input: &serde_json::Value, max_len: usize) -> String {
    if let Some(cmd) = input.get("command").and_then(serde_json::Value::as_str) {
        return truncate(cmd, max_len);
    }
    if let Some(path) = input.get("file_path").and_then(serde_json::Value::as_str) {
        return truncate(path, max_len);
    }
    if let Some(path) = input.get("path").and_then(serde_json::Value::as_str) {
        return truncate(path, max_len);
    }
    let s = input.to_string();
    truncate(&s, max_len)
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
