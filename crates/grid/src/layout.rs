use ratatui::layout::Rect;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum WidgetId {
    AgentList,
    AgentPane,
    TokenChart,
    RecentActivity,
    FileTree,
    StatusBar,
    SettingsPage,
    DatabasePage,
    ClassifiersPage,
    ClassifierPanel,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Grid {
    pub columns: u16,
    pub rows: Vec<Row>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Row {
    pub height: Size,
    pub cells: Vec<Cell>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Cell {
    pub widget: WidgetId,
    pub span: u16,
    #[serde(default = "default_rowspan")]
    pub rowspan: u16,
}

fn default_rowspan() -> u16 {
    1
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Size {
    Percent(String),
    Fixed(u16),
}

impl Size {
    pub fn resolve(&self, total: u16) -> u16 {
        match self {
            Size::Percent(s) => {
                let pct: u16 = s.trim_end_matches('%').parse().unwrap_or(0);
                total * pct / 100
            }
            Size::Fixed(n) => *n,
        }
    }
}

impl Default for Grid {
    fn default() -> Self {
        Self {
            columns: 12,
            rows: vec![
                Row {
                    height: Size::Percent("15%".to_string()),
                    cells: vec![
                        Cell {
                            widget: WidgetId::AgentList,
                            span: 2,
                            rowspan: 1,
                        },
                        Cell {
                            widget: WidgetId::AgentPane,
                            span: 10,
                            rowspan: 6,
                        },
                    ],
                },
                Row {
                    height: Size::Percent("12%".to_string()),
                    cells: vec![Cell {
                        widget: WidgetId::TokenChart,
                        span: 2,
                        rowspan: 1,
                    }],
                },
                Row {
                    height: Size::Percent("15%".to_string()),
                    cells: vec![Cell {
                        widget: WidgetId::ClassifierPanel,
                        span: 2,
                        rowspan: 1,
                    }],
                },
                Row {
                    height: Size::Percent("15%".to_string()),
                    cells: vec![Cell {
                        widget: WidgetId::RecentActivity,
                        span: 2,
                        rowspan: 1,
                    }],
                },
                Row {
                    height: Size::Percent("30%".to_string()),
                    cells: vec![Cell {
                        widget: WidgetId::FileTree,
                        span: 2,
                        rowspan: 1,
                    }],
                },
                Row {
                    height: Size::Percent("13%".to_string()),
                    cells: vec![Cell {
                        widget: WidgetId::StatusBar,
                        span: 2,
                        rowspan: 1,
                    }],
                },
            ],
        }
    }
}

/// Resolved cell position: widget name + rectangle
pub struct CellRect {
    pub widget: WidgetId,
    pub rect: Rect,
}

/// Tracks a column range occupied by a rowspanning cell
struct OccupiedSpan {
    col_start: u16,
    col_end: u16,
    end_row: usize, // exclusive: occupied through rows [start..end_row)
}

impl Grid {
    pub fn compute_rects(&self, area: Rect) -> Vec<CellRect> {
        let mut result = Vec::new();
        let col_width = area.width / self.columns;

        // Pre-compute row heights
        let mut row_heights: Vec<u16> = Vec::new();
        let mut y = area.y;
        for (ri, row) in self.rows.iter().enumerate() {
            let is_last_row = ri == self.rows.len() - 1;
            let h = if is_last_row {
                (area.y + area.height).saturating_sub(y)
            } else {
                row.height.resolve(area.height)
            };
            row_heights.push(h);
            y += h;
        }

        let mut occupied: Vec<OccupiedSpan> = Vec::new();
        y = area.y;

        for (ri, row) in self.rows.iter().enumerate() {
            // Remove spans that have expired before this row
            occupied.retain(|o| o.end_row > ri);

            let row_height = row_heights[ri];
            let mut col_cursor: u16 = 0;
            let mut cell_iter = row.cells.iter().peekable();

            while col_cursor < self.columns {
                // Skip columns occupied by rowspans
                if let Some(span) = occupied
                    .iter()
                    .find(|o| col_cursor >= o.col_start && col_cursor < o.col_end)
                {
                    col_cursor = span.col_end;
                    continue;
                }

                let Some(cell) = cell_iter.next() else {
                    break;
                };

                let x = area.x + col_cursor * col_width;
                let is_last_cell = cell_iter.peek().is_none();

                // Find boundary for last-cell remainder absorption
                let next_boundary = occupied
                    .iter()
                    .filter(|o| o.col_start > col_cursor)
                    .map(|o| o.col_start)
                    .min()
                    .unwrap_or(self.columns);

                let cell_width = if is_last_cell {
                    let end_x = if next_boundary >= self.columns {
                        area.x + area.width
                    } else {
                        area.x + next_boundary * col_width
                    };
                    end_x.saturating_sub(x)
                } else {
                    col_width * cell.span
                };

                let cell_height = if cell.rowspan > 1 {
                    let end_row = (ri + cell.rowspan as usize).min(self.rows.len());
                    row_heights[ri..end_row].iter().sum()
                } else {
                    row_height
                };

                result.push(CellRect {
                    widget: cell.widget,
                    rect: Rect::new(x, y, cell_width, cell_height),
                });

                if cell.rowspan > 1 {
                    occupied.push(OccupiedSpan {
                        col_start: col_cursor,
                        col_end: col_cursor + cell.span,
                        end_row: ri + cell.rowspan as usize,
                    });
                }

                col_cursor += cell.span;
            }

            y += row_height;
        }

        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_grid_has_six_rows() {
        let grid = Grid::default();
        assert_eq!(grid.columns, 12);
        assert_eq!(grid.rows.len(), 6);
    }

    #[test]
    fn default_grid_layout_correct() {
        let grid = Grid::default();
        assert_eq!(grid.rows[0].cells[0].widget, WidgetId::AgentList);
        assert_eq!(grid.rows[0].cells[0].span, 2);
        assert_eq!(grid.rows[0].cells[1].widget, WidgetId::AgentPane);
        assert_eq!(grid.rows[0].cells[1].span, 10);
        assert_eq!(grid.rows[0].cells[1].rowspan, 6);
        assert_eq!(grid.rows[1].cells[0].widget, WidgetId::TokenChart);
        assert_eq!(grid.rows[1].cells[0].span, 2);
        assert_eq!(grid.rows[2].cells[0].widget, WidgetId::ClassifierPanel);
        assert_eq!(grid.rows[2].cells[0].span, 2);
        assert_eq!(grid.rows[3].cells[0].widget, WidgetId::RecentActivity);
        assert_eq!(grid.rows[3].cells[0].span, 2);
        assert_eq!(grid.rows[4].cells[0].widget, WidgetId::FileTree);
        assert_eq!(grid.rows[4].cells[0].span, 2);
        assert_eq!(grid.rows[5].cells[0].widget, WidgetId::StatusBar);
        assert_eq!(grid.rows[5].cells[0].span, 2);
    }

    #[test]
    fn size_percent_resolves_correctly() {
        let size = Size::Percent("80%".to_string());
        assert_eq!(size.resolve(100), 80);
        assert_eq!(size.resolve(50), 40);
    }

    #[test]
    fn size_fixed_returns_value() {
        let size = Size::Fixed(10);
        assert_eq!(size.resolve(100), 10);
        assert_eq!(size.resolve(999), 10);
    }

    #[test]
    fn rowspan_merges_heights() {
        let grid = Grid {
            columns: 12,
            rows: vec![
                Row {
                    height: Size::Fixed(10),
                    cells: vec![
                        Cell {
                            widget: WidgetId::AgentList,
                            span: 4,
                            rowspan: 1,
                        },
                        Cell {
                            widget: WidgetId::AgentPane,
                            span: 8,
                            rowspan: 2,
                        },
                    ],
                },
                Row {
                    height: Size::Fixed(20),
                    cells: vec![Cell {
                        widget: WidgetId::TokenChart,
                        span: 4,
                        rowspan: 1,
                    }],
                },
            ],
        };

        let area = Rect::new(0, 0, 120, 30);
        let rects = grid.compute_rects(area);

        assert_eq!(rects.len(), 3);
        assert_eq!(rects[0].widget, WidgetId::AgentList);
        assert_eq!(rects[0].rect, Rect::new(0, 0, 40, 10));
        assert_eq!(rects[1].widget, WidgetId::AgentPane);
        assert_eq!(rects[1].rect, Rect::new(40, 0, 80, 30));
        assert_eq!(rects[2].widget, WidgetId::TokenChart);
        assert_eq!(rects[2].rect, Rect::new(0, 10, 40, 20));
    }

    #[test]
    fn no_rowspan_works_as_before() {
        let grid = Grid {
            columns: 12,
            rows: vec![
                Row {
                    height: Size::Fixed(10),
                    cells: vec![
                        Cell {
                            widget: WidgetId::AgentList,
                            span: 4,
                            rowspan: 1,
                        },
                        Cell {
                            widget: WidgetId::AgentPane,
                            span: 8,
                            rowspan: 1,
                        },
                    ],
                },
                Row {
                    height: Size::Fixed(30),
                    cells: vec![Cell {
                        widget: WidgetId::TokenChart,
                        span: 12,
                        rowspan: 1,
                    }],
                },
            ],
        };

        let area = Rect::new(0, 0, 120, 40);
        let rects = grid.compute_rects(area);

        assert_eq!(rects.len(), 3);
        assert_eq!(rects[0].rect, Rect::new(0, 0, 40, 10));
        assert_eq!(rects[1].rect, Rect::new(40, 0, 80, 10));
        assert_eq!(rects[2].rect, Rect::new(0, 10, 120, 30));
    }

    #[test]
    fn last_cell_absorbs_remainder_width() {
        let grid = Grid {
            columns: 12,
            rows: vec![Row {
                height: Size::Fixed(10),
                cells: vec![
                    Cell {
                        widget: WidgetId::AgentList,
                        span: 5,
                        rowspan: 1,
                    },
                    Cell {
                        widget: WidgetId::AgentPane,
                        span: 7,
                        rowspan: 1,
                    },
                ],
            }],
        };

        let area = Rect::new(0, 0, 121, 10);
        let rects = grid.compute_rects(area);

        assert_eq!(rects[0].rect.width, 50);
        assert_eq!(rects[1].rect.width, 71);
    }
}
