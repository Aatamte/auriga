use orchestrator_core::Page;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

/// Tab labels with a separator. Each tab has a fixed column range for hit-testing.
/// Layout: " Home │ Settings │ Database "
struct TabLayout {
    tabs: Vec<(Page, u16, u16)>, // (page, start_col, end_col) relative to area.x
}

impl TabLayout {
    fn compute() -> Self {
        let mut tabs = Vec::new();
        let mut col: u16 = 1; // leading space
        for &page in Page::ALL {
            let label_len = page.label().len() as u16;
            tabs.push((page, col, col + label_len));
            col += label_len + 3; // " │ " separator
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

pub struct NavBarWidget;

impl NavBarWidget {
    pub fn new() -> Self {
        Self
    }

    pub fn render(&self, frame: &mut Frame, area: Rect, current_page: Page) {
        if area.height == 0 || area.width == 0 {
            return;
        }

        let mut spans = Vec::new();
        spans.push(Span::raw(" "));

        for (i, &page) in Page::ALL.iter().enumerate() {
            if i > 0 {
                spans.push(Span::styled(" │ ", Style::default().fg(Color::DarkGray)));
            }
            let style = if page == current_page {
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::DarkGray)
            };
            spans.push(Span::styled(page.label(), style));
        }

        let paragraph = Paragraph::new(Line::from(spans));
        frame.render_widget(paragraph, area);
    }

    /// Hit-test a click at the given column. Returns the page if a tab was clicked.
    pub fn handle_click(&self, col: u16, area: Rect) -> Option<Page> {
        TabLayout::compute().hit_test(col, area)
    }
}

impl Default for NavBarWidget {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn area() -> Rect {
        Rect::new(0, 0, 80, 1)
    }

    #[test]
    fn click_home_tab() {
        let widget = NavBarWidget::new();
        // " Home" starts at col 1
        assert_eq!(widget.handle_click(1, area()), Some(Page::Home));
        assert_eq!(widget.handle_click(4, area()), Some(Page::Home));
    }

    #[test]
    fn click_settings_tab() {
        let widget = NavBarWidget::new();
        // "Home" = 4 chars at col 1-4, then " │ " = 3 chars, "Settings" starts at col 8
        assert_eq!(widget.handle_click(8, area()), Some(Page::Settings));
        assert_eq!(widget.handle_click(14, area()), Some(Page::Settings));
    }

    #[test]
    fn click_database_tab() {
        let widget = NavBarWidget::new();
        // "Settings" = 8 chars at col 8-15, then " │ " = 3, "Database" starts at col 19
        assert_eq!(widget.handle_click(19, area()), Some(Page::Database));
    }

    #[test]
    fn click_separator_returns_none() {
        let widget = NavBarWidget::new();
        // Separator " │ " is at cols 5-7
        assert_eq!(widget.handle_click(5, area()), None);
        assert_eq!(widget.handle_click(6, area()), None);
    }

    #[test]
    fn click_outside_returns_none() {
        let widget = NavBarWidget::new();
        assert_eq!(widget.handle_click(50, area()), None);
    }

    #[test]
    fn click_with_offset_area() {
        let widget = NavBarWidget::new();
        let offset_area = Rect::new(10, 0, 80, 1);
        // "Home" at col 11 (area.x + 1)
        assert_eq!(widget.handle_click(11, offset_area), Some(Page::Home));
        // Col 1 is before the area
        assert_eq!(widget.handle_click(1, offset_area), None);
    }
}
