use crate::{RenderContext, WidgetAction};
use auriga_core::{AgentId, AgentStatus, Page};
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

const SCROLLBAR_WIDTH: u16 = 7; // "◀━━▓━▶ "
const AGENT_GAP: &str = " ";

pub struct NavBarWidget {
    horizontal_scroll: u16,
}

impl NavBarWidget {
    pub fn new() -> Self {
        Self {
            horizontal_scroll: 0,
        }
    }

    pub fn render(
        &mut self,
        frame: &mut Frame,
        area: Rect,
        current_page: Page,
        ctx: &RenderContext,
    ) {
        if area.height == 0 || area.width == 0 {
            return;
        }

        let layout = HeaderLayout::compute(area.width, current_page, ctx, self.horizontal_scroll);
        self.horizontal_scroll = layout.agent_scroll;

        let mut spans = Vec::new();
        let left_pad = layout.tab_start.saturating_sub(layout.agent_render_width);

        if layout.agent_render_width > 0 {
            for part in &layout.agent_parts {
                match part {
                    HeaderPart::AgentText { text, style } => {
                        spans.push(Span::styled(text.clone(), *style))
                    }
                    HeaderPart::Raw(text) => spans.push(Span::raw(text.clone())),
                }
            }
        }

        if left_pad > 0 {
            spans.push(Span::raw(" ".repeat(left_pad as usize)));
        }

        spans.extend(layout.tab_spans);

        let paragraph = Paragraph::new(Line::from(spans));
        frame.render_widget(paragraph, area);
    }

    pub fn handle_click(
        &mut self,
        col: u16,
        area: Rect,
        current_page: Page,
        ctx: &RenderContext,
    ) -> Option<WidgetAction> {
        let layout = HeaderLayout::compute(area.width, current_page, ctx, self.horizontal_scroll);
        let x = col.checked_sub(area.x)?;

        if let Some(action) = layout.hit_test_agent_action(x) {
            self.horizontal_scroll =
                action.next_scroll(self.horizontal_scroll, layout.agent_scroll_max);
            return action.into_widget_action();
        }

        layout.hit_test_page(x).map(WidgetAction::NavigateTo)
    }
}

impl Default for NavBarWidget {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone)]
enum HeaderPart {
    AgentText { text: String, style: Style },
    Raw(String),
}

struct ScrollbarResult {
    parts: Vec<HeaderPart>,
    left_arrow_hit: Option<(u16, u16)>,
    track_hit: Option<(u16, u16, u16)>,
    right_arrow_hit: Option<(u16, u16)>,
}

struct HeaderLayout {
    agent_parts: Vec<HeaderPart>,
    agent_hits: Vec<(AgentId, u16, u16)>,
    left_arrow_hit: Option<(u16, u16)>,
    right_arrow_hit: Option<(u16, u16)>,
    track_hit: Option<(u16, u16, u16)>,
    tab_spans: Vec<Span<'static>>,
    tabs: Vec<(Page, u16, u16)>,
    tab_start: u16,
    agent_render_width: u16,
    agent_scroll: u16,
    agent_scroll_max: u16,
}

impl HeaderLayout {
    fn compute(
        area_width: u16,
        current_page: Page,
        ctx: &RenderContext,
        requested_scroll: u16,
    ) -> Self {
        let (tab_spans, tabs, tab_start) = compute_tabs(area_width, current_page, ctx.hidden_pages);
        let max_agent_width = tab_start;
        let items = build_agent_items(ctx);
        let content_width = total_agent_width(&items);

        if max_agent_width == 0 || items.is_empty() {
            return Self {
                agent_parts: Vec::new(),
                agent_hits: Vec::new(),
                left_arrow_hit: None,
                right_arrow_hit: None,
                track_hit: None,
                tab_spans,
                tabs,
                tab_start,
                agent_render_width: 0,
                agent_scroll: 0,
                agent_scroll_max: 0,
            };
        }

        if content_width <= max_agent_width {
            let (parts, hits, width) = render_agent_window(&items, 0, max_agent_width, false, 0);
            return Self {
                agent_parts: parts,
                agent_hits: hits,
                left_arrow_hit: None,
                right_arrow_hit: None,
                track_hit: None,
                tab_spans,
                tabs,
                tab_start,
                agent_render_width: width,
                agent_scroll: 0,
                agent_scroll_max: 0,
            };
        }

        if max_agent_width <= SCROLLBAR_WIDTH {
            return Self {
                agent_parts: Vec::new(),
                agent_hits: Vec::new(),
                left_arrow_hit: None,
                right_arrow_hit: None,
                track_hit: None,
                tab_spans,
                tabs,
                tab_start,
                agent_render_width: 0,
                agent_scroll: 0,
                agent_scroll_max: 0,
            };
        }

        let track_width = SCROLLBAR_WIDTH;
        let visible_width = max_agent_width.saturating_sub(track_width);
        let scroll_max = content_width.saturating_sub(visible_width);
        let scroll = requested_scroll.min(scroll_max);

        let (mut parts, hits, width) =
            render_agent_window(&items, scroll, visible_width, true, content_width);
        let scrollbar = build_scrollbar(track_width, width, scroll, scroll_max);
        parts.extend(scrollbar.parts);

        Self {
            agent_parts: parts,
            agent_hits: hits,
            left_arrow_hit: scrollbar.left_arrow_hit,
            right_arrow_hit: scrollbar.right_arrow_hit,
            track_hit: scrollbar.track_hit,
            tab_spans,
            tabs,
            tab_start,
            agent_render_width: width + track_width,
            agent_scroll: scroll,
            agent_scroll_max: scroll_max,
        }
    }

    fn hit_test_agent_action(&self, x: u16) -> Option<HeaderAction> {
        for &(id, start, end) in &self.agent_hits {
            if x >= start && x < end {
                return Some(HeaderAction::FocusAgent(id));
            }
        }

        if let Some((start, end)) = self.left_arrow_hit {
            if x >= start && x < end {
                return Some(HeaderAction::ScrollLeft);
            }
        }

        if let Some((start, thumb_start, thumb_end)) = self.track_hit {
            let track_end = self.right_arrow_hit.map(|(s, _)| s).unwrap_or(start);
            if x >= start && x < track_end {
                if x < thumb_start {
                    return Some(HeaderAction::PageLeft);
                }
                if x >= thumb_end {
                    return Some(HeaderAction::PageRight);
                }
                return None;
            }
        }

        if let Some((start, end)) = self.right_arrow_hit {
            if x >= start && x < end {
                return Some(HeaderAction::ScrollRight);
            }
        }

        None
    }

    fn hit_test_page(&self, x: u16) -> Option<Page> {
        for &(page, start, end) in &self.tabs {
            if x >= start && x < end {
                return Some(page);
            }
        }
        None
    }
}

enum HeaderAction {
    FocusAgent(AgentId),
    ScrollLeft,
    ScrollRight,
    PageLeft,
    PageRight,
}

impl HeaderAction {
    fn next_scroll(&self, current: u16, max: u16) -> u16 {
        match self {
            HeaderAction::FocusAgent(_) => current,
            HeaderAction::ScrollLeft => current.saturating_sub(4),
            HeaderAction::ScrollRight => (current + 4).min(max),
            HeaderAction::PageLeft => current.saturating_sub(12),
            HeaderAction::PageRight => (current + 12).min(max),
        }
    }

    fn into_widget_action(self) -> Option<WidgetAction> {
        match self {
            HeaderAction::FocusAgent(id) => Some(WidgetAction::FocusAgent(id)),
            HeaderAction::ScrollLeft
            | HeaderAction::ScrollRight
            | HeaderAction::PageLeft
            | HeaderAction::PageRight => None,
        }
    }
}

struct AgentItem {
    id: AgentId,
    text: String,
    width: u16,
    style: Style,
}

fn build_agent_items(ctx: &RenderContext) -> Vec<AgentItem> {
    ctx.agents
        .list()
        .iter()
        .enumerate()
        .map(|(idx, agent)| {
            let indicator = match agent.status {
                AgentStatus::Working => "●",
                AgentStatus::Idle => "○",
            };
            let status_color = match agent.status {
                AgentStatus::Working => Color::Green,
                AgentStatus::Idle => Color::DarkGray,
            };
            let is_active = ctx.focus.active_agent == Some(agent.id);
            let label = format!("{}:{} {}", idx + 1, agent.provider, indicator);
            let width = label.chars().count() as u16;
            let style = if is_active {
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(status_color)
            };

            AgentItem {
                id: agent.id,
                text: label,
                width,
                style,
            }
        })
        .collect()
}

fn total_agent_width(items: &[AgentItem]) -> u16 {
    let gaps = items.len().saturating_sub(1) as u16 * AGENT_GAP.chars().count() as u16;
    items.iter().map(|item| item.width).sum::<u16>() + gaps
}

fn render_agent_window(
    items: &[AgentItem],
    scroll: u16,
    visible_width: u16,
    append_gap: bool,
    content_width: u16,
) -> (Vec<HeaderPart>, Vec<(AgentId, u16, u16)>, u16) {
    if visible_width == 0 {
        return (Vec::new(), Vec::new(), 0);
    }

    let mut parts = Vec::new();
    let mut hits = Vec::new();
    let mut x = 0u16;
    let mut cursor = 0u16;
    let end = scroll.saturating_add(visible_width);

    for (idx, item) in items.iter().enumerate() {
        let text_width = item.width;
        let item_start = cursor;
        let item_end = cursor + text_width;

        if item_end > scroll && item_start < end {
            let visible_start = scroll.max(item_start);
            let visible_end = end.min(item_end);
            let local_start = visible_start - item_start;
            let local_end = visible_end - item_start;
            let text = slice_chars(&item.text, local_start as usize, local_end as usize);
            let render_start = x;
            x += text.chars().count() as u16;
            parts.push(HeaderPart::AgentText {
                text,
                style: item.style,
            });
            hits.push((item.id, render_start, x));
        }

        cursor = item_end;

        if idx + 1 < items.len() {
            let gap_start = cursor;
            let gap_width = AGENT_GAP.chars().count() as u16;
            let gap_end = gap_start + gap_width;
            if gap_end > scroll && gap_start < end {
                let visible_start = scroll.max(gap_start);
                let visible_end = end.min(gap_end);
                let local_start = visible_start - gap_start;
                let local_end = visible_end - gap_start;
                let text = slice_chars(AGENT_GAP, local_start as usize, local_end as usize);
                x += text.chars().count() as u16;
                parts.push(HeaderPart::Raw(text));
            }
            cursor = gap_end;
        }
    }

    if append_gap && content_width > visible_width {
        parts.push(HeaderPart::Raw(" ".to_string()));
        x += 1;
    }

    (parts, hits, x)
}

fn slice_chars(text: &str, start: usize, end: usize) -> String {
    text.chars()
        .skip(start)
        .take(end.saturating_sub(start))
        .collect()
}

fn build_scrollbar(width: u16, start_x: u16, scroll: u16, scroll_max: u16) -> ScrollbarResult {
    if width < SCROLLBAR_WIDTH {
        return ScrollbarResult {
            parts: Vec::new(),
            left_arrow_hit: None,
            track_hit: None,
            right_arrow_hit: None,
        };
    }

    let mut parts = Vec::new();
    let mut x = start_x;

    parts.push(HeaderPart::Raw("◀".to_string()));
    let left_arrow_hit = Some((x, x + 1));
    x += 1;

    let track_len = width.saturating_sub(2);
    let thumb_len = 1.max(track_len / 3);
    let thumb_room = track_len.saturating_sub(thumb_len);
    let thumb_offset = if scroll_max == 0 || thumb_room == 0 {
        0
    } else {
        ((scroll as f32 / scroll_max as f32) * thumb_room as f32).round() as u16
    };

    let thumb_start = x + thumb_offset;
    let thumb_end = thumb_start + thumb_len;

    for idx in 0..track_len {
        let ch = if idx >= thumb_offset && idx < thumb_offset + thumb_len {
            "▓"
        } else {
            "━"
        };
        parts.push(HeaderPart::Raw(ch.to_string()));
        x += 1;
    }

    let track_hit = Some((start_x + 1, thumb_start, thumb_end));

    parts.push(HeaderPart::Raw("▶".to_string()));
    let right_arrow_hit = Some((x, x + 1));

    ScrollbarResult {
        parts,
        left_arrow_hit,
        track_hit,
        right_arrow_hit,
    }
}

fn compute_tabs(
    area_width: u16,
    current_page: Page,
    hidden: &[Page],
) -> (Vec<Span<'static>>, Vec<(Page, u16, u16)>, u16) {
    let tab_style = |page: Page| {
        if page == current_page {
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::DarkGray)
        }
    };

    let mut tab_spans = Vec::new();
    let visible: Vec<_> = Page::ALL
        .iter()
        .filter(|p| !hidden.contains(p))
        .copied()
        .collect();
    for (i, page) in visible.iter().enumerate() {
        if i > 0 {
            tab_spans.push(Span::styled(" │ ", Style::default().fg(Color::DarkGray)));
        }
        tab_spans.push(Span::styled(page.label(), tab_style(*page)));
    }
    tab_spans.push(Span::raw(" "));

    let tabs_width: u16 = tab_spans.iter().map(|s| s.width() as u16).sum();
    let tab_start = area_width.saturating_sub(tabs_width);

    let mut tabs = Vec::new();
    let mut col = tab_start;
    for (i, page) in visible.iter().enumerate() {
        if i > 0 {
            col += 3;
        }
        let label_len = page.label().len() as u16;
        tabs.push((*page, col, col + label_len));
        col += label_len;
    }

    (tab_spans, tabs, tab_start)
}

#[cfg(test)]
mod tests {
    use super::*;
    use auriga_core::{AgentStore, FileTree, FocusState, TraceStore, TurnStore};
    use ratatui::buffer::Buffer;
    use std::path::PathBuf;

    fn make_ctx<'a>(agents: &'a AgentStore, focus: &'a FocusState) -> RenderContext<'a> {
        static HIDDEN: [Page; 0] = [];
        let turns = Box::leak(Box::new(TurnStore::new()));
        let traces = Box::leak(Box::new(TraceStore::new()));
        let file_tree = Box::leak(Box::new(FileTree::new(PathBuf::from("/tmp"))));
        let render_term = Box::leak(Box::new(|_id: AgentId, _buf: &mut Buffer, _area: Rect| {}));
        RenderContext {
            agents,
            turns,
            traces,
            focus,
            file_tree,
            render_term,
            hidden_pages: &HIDDEN,
        }
    }

    #[test]
    fn tabs_remain_clickable() {
        let agents = AgentStore::new();
        let focus = FocusState::new();
        let ctx = make_ctx(&agents, &focus);
        let layout = HeaderLayout::compute(80, Page::Home, &ctx, 0);
        assert_eq!(layout.hit_test_page(layout.tab_start), Some(Page::Home));
    }

    #[test]
    fn agent_click_focuses_visible_agent() {
        let mut agents = AgentStore::new();
        let id = agents.create("claude");
        let focus = FocusState::new();
        let ctx = make_ctx(&agents, &focus);
        let layout = HeaderLayout::compute(120, Page::Home, &ctx, 0);
        let x = layout.agent_hits[0].1;
        match layout.hit_test_agent_action(x) {
            Some(HeaderAction::FocusAgent(clicked)) => assert_eq!(clicked, id),
            _ => panic!("expected focused agent hit"),
        }
    }

    #[test]
    fn overflowing_agents_show_scrollbar() {
        let mut agents = AgentStore::new();
        for _ in 0..12 {
            agents.create("claude");
        }
        let focus = FocusState::new();
        let ctx = make_ctx(&agents, &focus);
        let layout = HeaderLayout::compute(110, Page::Home, &ctx, 0);
        assert!(layout.left_arrow_hit.is_some());
        assert!(layout.right_arrow_hit.is_some());
        assert!(layout.track_hit.is_some());
    }

    #[test]
    fn scroll_is_clamped_to_max() {
        let mut agents = AgentStore::new();
        for _ in 0..8 {
            agents.create("codex");
        }
        let focus = FocusState::new();
        let ctx = make_ctx(&agents, &focus);
        let layout = HeaderLayout::compute(60, Page::Home, &ctx, u16::MAX);
        assert_eq!(layout.agent_scroll, layout.agent_scroll_max);
    }
}
