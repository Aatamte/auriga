use orchestrator_core::Page;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

pub struct NavBarWidget;

impl NavBarWidget {
    pub fn new() -> Self {
        Self
    }

    pub fn render(&self, frame: &mut Frame, area: Rect, current_page: Page, hidden: &[Page]) {
        if area.height == 0 || area.width == 0 {
            return;
        }

        let tab_style = |page: Page| {
            if page == current_page {
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::DarkGray)
            }
        };

        // Left group
        let mut left_spans = Vec::new();
        left_spans.push(Span::raw(" "));
        let left_visible: Vec<_> = Page::LEFT
            .iter()
            .filter(|p| !hidden.contains(p))
            .copied()
            .collect();
        for (i, page) in left_visible.iter().enumerate() {
            if i > 0 {
                left_spans.push(Span::styled(" │ ", Style::default().fg(Color::DarkGray)));
            }
            left_spans.push(Span::styled(page.label(), tab_style(*page)));
        }

        // Right group
        let mut right_spans = Vec::new();
        let right_visible: Vec<_> = Page::RIGHT
            .iter()
            .filter(|p| !hidden.contains(p))
            .copied()
            .collect();
        for (i, page) in right_visible.iter().enumerate() {
            if i > 0 {
                right_spans.push(Span::styled(" │ ", Style::default().fg(Color::DarkGray)));
            }
            right_spans.push(Span::styled(page.label(), tab_style(*page)));
        }
        right_spans.push(Span::raw(" "));

        // Compute widths
        let left_width: u16 = left_spans.iter().map(|s| s.width() as u16).sum();
        let right_width: u16 = right_spans.iter().map(|s| s.width() as u16).sum();
        let gap = area.width.saturating_sub(left_width + right_width);

        // Combine: left + flexible gap + right
        let mut spans = left_spans;
        if gap > 0 {
            spans.push(Span::raw(" ".repeat(gap as usize)));
        }
        spans.extend(right_spans);

        let paragraph = Paragraph::new(Line::from(spans));
        frame.render_widget(paragraph, area);
    }

    /// Hit-test a click at the given column. Returns the page if a tab was clicked.
    pub fn handle_click(&self, col: u16, area: Rect, hidden: &[Page]) -> Option<Page> {
        TabLayout::compute(area.width, hidden).hit_test(col, area)
    }
}

impl Default for NavBarWidget {
    fn default() -> Self {
        Self::new()
    }
}

/// Computed tab positions for hit-testing, matching the render layout.
struct TabLayout {
    tabs: Vec<(Page, u16, u16)>, // (page, start_col, end_col) relative to area.x
}

impl TabLayout {
    fn compute(area_width: u16, hidden: &[Page]) -> Self {
        let mut tabs = Vec::new();

        // Left group
        let mut col: u16 = 1; // leading space
        let left_visible: Vec<_> = Page::LEFT
            .iter()
            .filter(|p| !hidden.contains(p))
            .copied()
            .collect();
        for (i, page) in left_visible.iter().enumerate() {
            if i > 0 {
                col += 3; // " │ "
            }
            let label_len = page.label().len() as u16;
            tabs.push((*page, col, col + label_len));
            col += label_len;
        }
        let left_width = col;

        // Right group: compute width first, then position from right edge
        let right_visible: Vec<_> = Page::RIGHT
            .iter()
            .filter(|p| !hidden.contains(p))
            .copied()
            .collect();
        let mut right_width: u16 = 1; // trailing space
        for (i, page) in right_visible.iter().enumerate() {
            if i > 0 {
                right_width += 3;
            }
            right_width += page.label().len() as u16;
        }

        let mut col = area_width.saturating_sub(right_width);
        for (i, page) in right_visible.iter().enumerate() {
            if i > 0 {
                col += 3;
            }
            let label_len = page.label().len() as u16;
            // Only add if it doesn't overlap left group
            if col >= left_width {
                tabs.push((*page, col, col + label_len));
            }
            col += label_len;
        }

        Self { tabs }
    }

    fn hit_test(&self, col: u16, area: Rect) -> Option<Page> {
        let x = col.checked_sub(area.x)?;
        for &(page, start, end) in &self.tabs {
            if x >= start && x < end {
                return Some(page);
            }
        }
        None
    }
}
