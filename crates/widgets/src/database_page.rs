use crate::{RenderContext, Widget, WidgetAction};
use auriga_core::ScrollDirection;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;

const PAGE_SIZE: u64 = 50;

/// Minimal copies of storage types so the widgets crate doesn't depend on storage.
#[derive(Debug, Clone)]
pub struct TableInfo {
    pub name: String,
    pub row_count: u64,
}

#[derive(Debug, Clone)]
pub struct DbMetadata {
    pub file_size_bytes: u64,
    pub tables: Vec<TableInfo>,
    pub total_rows: u64,
}

#[derive(Debug, Clone)]
pub struct QueryResult {
    pub columns: Vec<String>,
    pub rows: Vec<Vec<String>>,
    pub total_rows: u64,
}

pub struct DatabasePage {
    pub metadata: Option<DbMetadata>,
    pub selected_table: usize,
    pub rows: Option<QueryResult>,
    pub selected_row: Option<usize>,
    pub page_offset: u64,
}

impl DatabasePage {
    pub fn new() -> Self {
        Self {
            metadata: None,
            selected_table: 0,
            rows: None,
            selected_row: None,
            page_offset: 0,
        }
    }

    pub fn set_metadata(&mut self, metadata: DbMetadata) {
        self.metadata = Some(metadata);
    }

    pub fn set_rows(&mut self, result: QueryResult) {
        self.rows = Some(result);
        self.selected_row = if self.rows.as_ref().is_none_or(|r| r.rows.is_empty()) {
            None
        } else {
            Some(0)
        };
    }

    /// Returns (table_name, limit, offset) for the currently selected table query.
    pub fn current_query(&self) -> Option<(String, u64, u64)> {
        let meta = self.metadata.as_ref()?;
        let table = meta.tables.get(self.selected_table)?;
        Some((table.name.clone(), PAGE_SIZE, self.page_offset))
    }

    fn table_count(&self) -> usize {
        self.metadata.as_ref().map(|m| m.tables.len()).unwrap_or(0)
    }

    fn total_pages(&self) -> u64 {
        self.rows
            .as_ref()
            .map(|r| r.total_rows.div_ceil(PAGE_SIZE))
            .unwrap_or(1)
    }

    fn current_page(&self) -> u64 {
        self.page_offset / PAGE_SIZE + 1
    }

    pub fn handle_key(&mut self, key: KeyEvent) -> Option<WidgetAction> {
        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('r') {
            return Some(WidgetAction::RefreshDatabase);
        }

        match key.code {
            KeyCode::Left => {
                if self.table_count() > 0 && self.selected_table > 0 {
                    self.selected_table -= 1;
                    self.page_offset = 0;
                    self.rows = None;
                    self.selected_row = None;
                    return self.query_action();
                }
            }
            KeyCode::Right => {
                if self.selected_table + 1 < self.table_count() {
                    self.selected_table += 1;
                    self.page_offset = 0;
                    self.rows = None;
                    self.selected_row = None;
                    return self.query_action();
                }
            }
            KeyCode::Up => {
                if let Some(sel) = self.selected_row {
                    if sel > 0 {
                        self.selected_row = Some(sel - 1);
                    }
                }
            }
            KeyCode::Down => {
                if let Some(sel) = self.selected_row {
                    let max = self.rows.as_ref().map(|r| r.rows.len()).unwrap_or(0);
                    if sel + 1 < max {
                        self.selected_row = Some(sel + 1);
                    }
                }
            }
            KeyCode::PageDown => {
                let total = self.rows.as_ref().map(|r| r.total_rows).unwrap_or(0);
                if self.page_offset + PAGE_SIZE < total {
                    self.page_offset += PAGE_SIZE;
                    self.selected_row = Some(0);
                    return self.query_action();
                }
            }
            KeyCode::PageUp => {
                if self.page_offset >= PAGE_SIZE {
                    self.page_offset -= PAGE_SIZE;
                    self.selected_row = Some(0);
                    return self.query_action();
                }
            }
            _ => {}
        }
        None
    }

    fn query_action(&self) -> Option<WidgetAction> {
        let (table, limit, offset) = self.current_query()?;
        Some(WidgetAction::QueryTable {
            table,
            limit,
            offset,
        })
    }
}

impl Default for DatabasePage {
    fn default() -> Self {
        Self::new()
    }
}

impl Widget for DatabasePage {
    fn render(&mut self, frame: &mut Frame, area: Rect, _ctx: &RenderContext) {
        let block = Block::default()
            .title(" Database ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::DarkGray));

        let inner = block.inner(area);
        frame.render_widget(block, area);

        if inner.height == 0 || inner.width == 0 {
            return;
        }

        let mut lines: Vec<Line> = Vec::new();

        // Metadata header
        if let Some(ref meta) = self.metadata {
            let size_str = format_size(meta.file_size_bytes);
            lines.push(Line::from(vec![
                Span::styled(
                    "  auriga.db",
                    Style::default()
                        .fg(Color::White)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled("  │  ", Style::default().fg(Color::DarkGray)),
                Span::styled(size_str, Style::default().fg(Color::Green)),
                Span::styled("  │  ", Style::default().fg(Color::DarkGray)),
                Span::styled(
                    format!("{} tables", meta.tables.len()),
                    Style::default().fg(Color::Cyan),
                ),
                Span::styled("  │  ", Style::default().fg(Color::DarkGray)),
                Span::styled(
                    format!("{} rows", meta.total_rows),
                    Style::default().fg(Color::Yellow),
                ),
            ]));

            // Table tabs
            lines.push(Line::from(""));
            let mut tab_spans = vec![Span::raw("  ")];
            for (i, table) in meta.tables.iter().enumerate() {
                if i > 0 {
                    tab_spans.push(Span::styled("   ", Style::default().fg(Color::DarkGray)));
                }
                let style = if i == self.selected_table {
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(Color::DarkGray)
                };
                tab_spans.push(Span::styled(
                    format!("{} ({})", table.name, table.row_count),
                    style,
                ));
            }
            lines.push(Line::from(tab_spans));
            lines.push(Line::styled(
                format!("  {}", "─".repeat(inner.width.saturating_sub(4) as usize)),
                Style::default().fg(Color::DarkGray),
            ));
        } else {
            lines.push(Line::styled(
                "  Loading...",
                Style::default().fg(Color::DarkGray),
            ));
        }

        // Table rows
        if let Some(ref result) = self.rows {
            // Page indicator
            lines.push(Line::from(vec![
                Span::raw("  "),
                Span::styled(
                    format!("Page {} / {}", self.current_page(), self.total_pages()),
                    Style::default().fg(Color::DarkGray),
                ),
            ]));
            lines.push(Line::from(""));

            // Column headers
            let col_width = compute_col_width(inner.width.saturating_sub(4), result.columns.len());
            let mut header_spans = vec![Span::raw("  ")];
            for col in &result.columns {
                header_spans.push(Span::styled(
                    truncate_pad(col, col_width),
                    Style::default()
                        .fg(Color::White)
                        .add_modifier(Modifier::BOLD),
                ));
            }
            lines.push(Line::from(header_spans));

            // Separator
            let mut sep_spans = vec![Span::raw("  ")];
            for _ in &result.columns {
                sep_spans.push(Span::styled(
                    truncate_pad(&"─".repeat(col_width), col_width),
                    Style::default().fg(Color::DarkGray),
                ));
            }
            lines.push(Line::from(sep_spans));

            // Data rows
            for (i, row) in result.rows.iter().enumerate() {
                let is_selected = self.selected_row == Some(i);
                let row_style = if is_selected {
                    Style::default().fg(Color::Cyan)
                } else {
                    Style::default().fg(Color::White)
                };

                let mut row_spans = vec![Span::raw("  ")];
                for val in row {
                    row_spans.push(Span::styled(truncate_pad(val, col_width), row_style));
                }
                lines.push(Line::from(row_spans));
            }

            if result.rows.is_empty() {
                lines.push(Line::styled(
                    "  (empty table)",
                    Style::default().fg(Color::DarkGray),
                ));
            }
        }

        // Help line at bottom
        lines.push(Line::from(""));
        lines.push(Line::styled(
            "  ←→ table  │  ↑↓ row  │  PgUp/PgDn page  │  Ctrl+R refresh",
            Style::default().fg(Color::DarkGray),
        ));

        let paragraph = Paragraph::new(lines);
        frame.render_widget(paragraph, inner);
    }

    fn handle_scroll(&mut self, _direction: ScrollDirection) {}

    fn handle_click(&mut self, row: u16, _col: u16, _ctx: &RenderContext) -> Option<WidgetAction> {
        // Click on a data row to select it
        // Header takes ~7 lines (metadata + tabs + separator + page + blank + col header + separator)
        let data_start: u16 = 8;
        if row >= data_start {
            let idx = (row - data_start) as usize;
            let max = self.rows.as_ref().map(|r| r.rows.len()).unwrap_or(0);
            if idx < max {
                self.selected_row = Some(idx);
            }
        }
        None
    }
}

fn format_size(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{} B", bytes)
    } else if bytes < 1024 * 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else {
        format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
    }
}

fn compute_col_width(available: u16, col_count: usize) -> usize {
    if col_count == 0 {
        return 0;
    }
    (available as usize / col_count).clamp(6, 20)
}

fn truncate_pad(s: &str, width: usize) -> String {
    let char_count = s.chars().count();
    if char_count >= width {
        let truncated: String = s.chars().take(width.saturating_sub(1)).collect();
        format!("{:<width$}", truncated, width = width)
    } else {
        format!("{:<width$}", s, width = width)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_page_with_data() -> DatabasePage {
        let mut page = DatabasePage::new();
        page.set_metadata(DbMetadata {
            file_size_bytes: 24576,
            tables: vec![
                TableInfo {
                    name: "traces".into(),
                    row_count: 5,
                },
                TableInfo {
                    name: "turns".into(),
                    row_count: 20,
                },
            ],
            total_rows: 25,
        });
        page.set_rows(QueryResult {
            columns: vec!["id".into(), "status".into()],
            rows: vec![
                vec!["t1".into(), "Complete".into()],
                vec!["t2".into(), "Active".into()],
            ],
            total_rows: 5,
        });
        page
    }

    #[test]
    fn new_page_has_no_metadata() {
        let page = DatabasePage::new();
        assert!(page.metadata.is_none());
    }

    #[test]
    fn set_metadata_stores_data() {
        let mut page = DatabasePage::new();
        page.set_metadata(DbMetadata {
            file_size_bytes: 0,
            tables: vec![],
            total_rows: 0,
        });
        assert!(page.metadata.is_some());
    }

    #[test]
    fn set_rows_selects_first() {
        let mut page = make_page_with_data();
        assert_eq!(page.selected_row, Some(0));

        page.set_rows(QueryResult {
            columns: vec![],
            rows: vec![],
            total_rows: 0,
        });
        assert_eq!(page.selected_row, None);
    }

    #[test]
    fn left_right_switches_table() {
        let mut page = make_page_with_data();
        assert_eq!(page.selected_table, 0);

        page.handle_key(make_key(KeyCode::Right, KeyModifiers::NONE));
        assert_eq!(page.selected_table, 1);

        page.handle_key(make_key(KeyCode::Left, KeyModifiers::NONE));
        assert_eq!(page.selected_table, 0);
    }

    #[test]
    fn right_returns_query_action() {
        let mut page = make_page_with_data();
        let action = page.handle_key(make_key(KeyCode::Right, KeyModifiers::NONE));
        assert!(matches!(action, Some(WidgetAction::QueryTable { .. })));
    }

    #[test]
    fn up_down_selects_row() {
        let mut page = make_page_with_data();
        page.selected_row = Some(0);

        page.handle_key(make_key(KeyCode::Down, KeyModifiers::NONE));
        assert_eq!(page.selected_row, Some(1));

        page.handle_key(make_key(KeyCode::Up, KeyModifiers::NONE));
        assert_eq!(page.selected_row, Some(0));
    }

    #[test]
    fn down_stops_at_end() {
        let mut page = make_page_with_data();
        page.selected_row = Some(1); // last row
        page.handle_key(make_key(KeyCode::Down, KeyModifiers::NONE));
        assert_eq!(page.selected_row, Some(1));
    }

    #[test]
    fn ctrl_r_returns_refresh() {
        let mut page = make_page_with_data();
        let action = page.handle_key(make_key(KeyCode::Char('r'), KeyModifiers::CONTROL));
        assert!(matches!(action, Some(WidgetAction::RefreshDatabase)));
    }

    #[test]
    fn current_query_returns_table_info() {
        let page = make_page_with_data();
        let (table, limit, offset) = page.current_query().unwrap();
        assert_eq!(table, "traces");
        assert_eq!(limit, PAGE_SIZE);
        assert_eq!(offset, 0);
    }

    #[test]
    fn format_size_formats_correctly() {
        assert_eq!(format_size(500), "500 B");
        assert_eq!(format_size(2048), "2.0 KB");
        assert_eq!(format_size(1_500_000), "1.4 MB");
    }

    #[test]
    fn truncate_pad_works() {
        assert_eq!(truncate_pad("hello", 10), "hello     ");
        assert_eq!(truncate_pad("hello world long", 10), "hello wor ");
    }

    fn make_key(code: KeyCode, modifiers: KeyModifiers) -> KeyEvent {
        use crossterm::event::KeyEventState;
        KeyEvent {
            code,
            modifiers,
            kind: crossterm::event::KeyEventKind::Press,
            state: KeyEventState::NONE,
        }
    }
}
