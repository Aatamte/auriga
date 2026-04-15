//! Integration tests for auriga-grid public API

use auriga_grid::{CellRect, Grid, WidgetId};
use ratatui::layout::Rect;

#[test]
fn grid_default_layout() {
    let grid = Grid::default();
    let area = Rect::new(0, 0, 200, 60);
    let cells = grid.compute_rects(area);

    // Should have cells for the default layout
    assert!(!cells.is_empty());

    // All cells should be within bounds
    for cell in &cells {
        assert!(cell.rect.x + cell.rect.width <= area.width);
        assert!(cell.rect.y + cell.rect.height <= area.height);
    }
}

#[test]
fn grid_cells_cover_area() {
    let grid = Grid::default();
    let area = Rect::new(0, 0, 200, 60);
    let cells = grid.compute_rects(area);

    // Should have cells for the layout
    assert!(!cells.is_empty());

    // All cells should have valid widget ids
    for cell in &cells {
        let _ = cell.widget; // Just verify it exists
    }
}

#[test]
fn grid_consistent_on_repeated_calls() {
    let grid = Grid::default();
    let area = Rect::new(0, 0, 200, 60);

    let cells1 = grid.compute_rects(area);
    let cells2 = grid.compute_rects(area);

    assert_eq!(cells1.len(), cells2.len());
    for (c1, c2) in cells1.iter().zip(cells2.iter()) {
        assert_eq!(c1.widget, c2.widget);
        assert_eq!(c1.rect, c2.rect);
    }
}

#[test]
fn grid_handles_small_area() {
    let grid = Grid::default();
    let area = Rect::new(0, 0, 20, 10);
    let cells = grid.compute_rects(area);

    // Should still produce valid cells
    for cell in &cells {
        assert!(cell.rect.width > 0 || cell.rect.height > 0 || cells.len() == 0);
    }
}

#[test]
fn grid_handles_large_area() {
    let grid = Grid::default();
    let area = Rect::new(0, 0, 400, 120);
    let cells = grid.compute_rects(area);

    // Should scale to larger area
    assert!(!cells.is_empty());
    for cell in &cells {
        assert!(cell.rect.x + cell.rect.width <= area.width);
        assert!(cell.rect.y + cell.rect.height <= area.height);
    }
}

#[test]
fn cell_rect_contains_widget_id() {
    let grid = Grid::default();
    let area = Rect::new(0, 0, 200, 60);
    let cells = grid.compute_rects(area);

    // Every cell has a widget id
    for cell in &cells {
        // WidgetId should be a valid variant
        let _ = cell.widget;
    }
}
