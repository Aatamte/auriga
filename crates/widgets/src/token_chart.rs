use crate::{RenderContext, Widget, WidgetAction};
use orchestrator_core::ScrollDirection;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;

pub struct TokenChartWidget;

impl TokenChartWidget {
    pub fn new() -> Self {
        Self
    }
}

impl Default for TokenChartWidget {
    fn default() -> Self {
        Self::new()
    }
}

fn format_tokens(n: u64) -> String {
    if n >= 1_000_000 {
        format!("{:.1}M", n as f64 / 1_000_000.0)
    } else if n >= 1_000 {
        format!("{:.1}K", n as f64 / 1_000.0)
    } else {
        n.to_string()
    }
}

impl Widget for TokenChartWidget {
    fn render(&mut self, frame: &mut Frame, area: Rect, ctx: &RenderContext) {
        let block = Block::default()
            .title(" Tokens ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::DarkGray));

        let inner = block.inner(area);
        frame.render_widget(block, area);

        if inner.height == 0 || inner.width == 0 {
            return;
        }

        let agents = ctx.agents.list();
        if agents.is_empty() {
            let placeholder = Paragraph::new(Line::from(Span::styled(
                "No agents",
                Style::default().fg(Color::DarkGray),
            )));
            frame.render_widget(placeholder, inner);
            return;
        }

        // Collect token usage per agent
        let usages: Vec<_> = agents
            .iter()
            .map(|a| {
                let usage = ctx.turns.agent_token_usage(a.id);
                (a, usage)
            })
            .collect();

        let max_tokens = usages.iter().map(|(_, u)| u.total()).max().unwrap_or(1).max(1);

        let mut lines = Vec::new();
        for (agent, usage) in &usages {
            let total = usage.total();
            let is_active = ctx.focus.active_agent == Some(agent.id);

            // Short label: just the hex part after "#"
            let label = agent
                .name
                .find('#')
                .map(|i| &agent.name[i..])
                .unwrap_or(&agent.name);

            let label_style = if is_active {
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::White)
            };

            // Bar width proportional to max, fitting in available width
            let bar_area = inner.width.saturating_sub(10) as u64; // reserve space for label + count
            let bar_len = if max_tokens > 0 {
                (total * bar_area / max_tokens) as usize
            } else {
                0
            };

            let bar_char = "█";
            let bar: String = bar_char.repeat(bar_len);
            let count_str = format_tokens(total);

            let bar_color = if is_active {
                Color::Cyan
            } else {
                Color::DarkGray
            };

            lines.push(Line::from(vec![
                Span::styled(format!("{:<8}", label), label_style),
                Span::styled(bar, Style::default().fg(bar_color)),
                Span::raw(" "),
                Span::styled(count_str, Style::default().fg(Color::DarkGray)),
            ]));
        }

        let paragraph = Paragraph::new(lines);
        frame.render_widget(paragraph, inner);
    }

    fn handle_scroll(&mut self, _direction: ScrollDirection) {}

    fn handle_click(
        &mut self,
        _row: u16,
        _col: u16,
        _ctx: &RenderContext,
    ) -> Option<WidgetAction> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_tokens_small() {
        assert_eq!(format_tokens(0), "0");
        assert_eq!(format_tokens(500), "500");
        assert_eq!(format_tokens(999), "999");
    }

    #[test]
    fn format_tokens_thousands() {
        assert_eq!(format_tokens(1_000), "1.0K");
        assert_eq!(format_tokens(15_500), "15.5K");
    }

    #[test]
    fn format_tokens_millions() {
        assert_eq!(format_tokens(1_000_000), "1.0M");
        assert_eq!(format_tokens(2_500_000), "2.5M");
    }
}
