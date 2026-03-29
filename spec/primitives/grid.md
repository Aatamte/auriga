# Grid

## What It Is

A 12-column grid layout system. The dashboard is composed of rows. Each row has a height and contains cells that span columns. The layout is defined in `layout.json` and loaded at startup.

## Schema

```json
{
  "columns": 12,
  "rows": [
    {
      "height": "80%",
      "cells": [
        { "widget": "agent-list", "span": 3 },
        { "widget": "agent-pane", "span": 9 }
      ]
    },
    {
      "height": "20%",
      "cells": [
        { "widget": "status-bar", "span": 12 }
      ]
    }
  ]
}
```

## Types

```rust
struct Grid {
    columns: u16,       // always 12
    rows: Vec<Row>,
}

struct Row {
    height: Size,
    cells: Vec<Cell>,
}

struct Cell {
    widget: String,     // widget name, resolved at render time
    span: u16,          // how many columns this cell occupies
}

enum Size {
    Percent(u16),       // percentage of total height
    Fixed(u16),         // fixed number of terminal rows
}
```

## Rendering

Given a terminal area `Rect`:

1. Divide the height among rows according to their `Size` values
2. For each row, divide the width into `columns` equal units
3. Each cell gets `span * column_width` pixels of width
4. Look up the widget by name, call `widget.render(frame, cell_rect, state)`

```rust
impl Grid {
    fn render(&self, frame: &mut Frame, area: Rect, state: &AppState) {
        // 1. compute row rects from area.height and row sizes
        // 2. for each row, compute cell rects from area.width and cell spans
        // 3. for each cell, resolve widget name, render into rect
    }
}
```

## Constraints

- Cell spans in a row must sum to `columns` (12). If they don't, leftover space is empty.
- Row heights must sum to 100% (or use fixed sizes that fit). Overflow is clipped.
- A cell with span 0 is invalid and ignored.

## File Location

```
~/.agent-orchestrator/layout.json
```

Missing file → default layout (agent-list 3 cols, agent-pane 9 cols, status-bar full width).

## Rules

1. Grid is loaded once at startup. Hot-reloading is a future concern.
2. Grid does not own widgets. It resolves widget names to instances at render time.
3. Grid handles the math. Widgets don't know about the grid.
4. Grid is pure layout — no styling, no borders, no decoration. Widgets handle their own appearance.
