# UI — Technical Specification

## Widget Trait

```rust
pub trait Widget {
    fn render(&mut self, frame: &mut Frame, area: Rect, ctx: &RenderContext);
    fn handle_scroll(&mut self, direction: ScrollDirection);
    fn handle_click(&mut self, row: u16, col: u16, ctx: &RenderContext) -> Option<WidgetAction>;
}
```

`render` takes `&mut self` because widgets maintain internal scroll state.

## RenderContext

```rust
pub struct RenderContext<'a> {
    pub agents: &'a AgentStore,
    pub turns: &'a TurnStore,
    pub traces: &'a TraceStore,
    pub focus: &'a FocusState,
    pub file_tree: &'a FileTree,
    pub render_term: &'a dyn Fn(AgentId, &mut Buffer, Rect),
}
```

Read-only view of all application state. The `render_term` closure bridges widget rendering with the terminal emulator without exposing the `Term` type.

## WidgetAction

```rust
pub enum WidgetAction {
    SelectAgent(AgentId),
    FocusAgent(AgentId),
    UnfocusAgent,
    ToggleClassifier(String),
    SaveSettings(Vec<SettingsField>),
    SwitchPage(Page),
}
```

Returned by `handle_click()`. The `App` receives these in `handle_widget_action()` and performs corresponding state mutations.

## Grid Layout Schema

`.auriga/layout.json`:

```json
{
  "columns": 12,
  "rows": [
    {
      "height": {"Percent": 15},
      "cells": [
        {"col_start": 0, "col_span": 2, "widget": "agent-list"},
        {"col_start": 2, "col_span": 10, "widget": "agent-pane", "row_span": 5}
      ]
    },
    {"height": {"Percent": 15}, "cells": [{"col_start": 0, "col_span": 2, "widget": "token-chart"}]},
    {"height": {"Percent": 20}, "cells": [{"col_start": 0, "col_span": 2, "widget": "recent-activity"}]},
    {"height": {"Percent": 35}, "cells": [{"col_start": 0, "col_span": 2, "widget": "file-tree"}]},
    {"height": {"Percent": 15}, "cells": [{"col_start": 0, "col_span": 2, "widget": "status-bar"}]}
  ]
}
```

Grid engine computes `CellRect { widget: WidgetId, rect: Rect }` for each cell. These are used for rendering and mouse hit-testing.

## FocusState

```rust
pub struct FocusState {
    pub panel: Panel,              // AgentList | AgentPane
    pub page: Page,                // Home | Classifiers | Database | Settings
    pub active_agent: Option<AgentId>,
}
```

## Page Enum

```rust
pub enum Page {
    Home,
    Classifiers,
    Database,
    Settings,
}

impl Page {
    pub const ALL: &[Page] = &[Page::Home, Page::Classifiers, Page::Database, Page::Settings];
}
```

Tab order is defined by `Page::ALL`.

## Input Dispatch

**Keyboard (`handle_key`):**
1. Check global shortcuts: ctrl+n (new agent), ctrl+w (close), ctrl+q (quit), tab/shift+tab (page)
2. If on Home page and AgentPane focused, forward key to active agent's PTY via `pty.write(key_to_bytes(key))`

**Mouse (`handle_mouse`):**
1. Hit-test click against `last_nav_rect` → page switch
2. Hit-test click against `last_cell_rects` → find matching `CellRect`
3. Call `widget.handle_click(row - rect.y, col - rect.x, ctx)`
4. If returns `Some(action)`, dispatch via `handle_widget_action(action)`

**Scroll:**
1. Find widget under cursor via `last_cell_rects`
2. Call `widget.handle_scroll(direction)`

## Agent Pane Modes

**Grid mode:** Up to 6 agents in a responsive 2-column layout. Each cell shows: status indicator, agent name, model, turn count, token count, and live terminal output.

**Focused mode:** Single agent full-screen. Entered by clicking an agent in grid mode. Back button (◀) returns to grid.
