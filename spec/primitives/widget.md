# Widget

## What It Is

A widget is a self-contained UI component that renders into a rectangular area. Widgets auto-fit to their given area and are scrollable when content overflows. They support mouse events natively.

## Trait

```rust
trait Widget {
    fn render(&self, frame: &mut Frame, area: Rect, ctx: &RenderContext);
    fn handle_scroll(&mut self, direction: ScrollDirection);
    fn handle_click(&mut self, row: u16, col: u16);
    fn content_height(&self, ctx: &RenderContext) -> usize;
}

enum ScrollDirection { Up, Down }
```

Every widget:
- **Auto-fits** to whatever `Rect` it receives — no hardcoded sizes
- **Scrolls** when content exceeds the available area
- **Handles mouse clicks** for selection, focus, interaction
- Reports its **content height** so the scroll system knows when to show indicators

## Scrollable

A composable building block that any widget can use internally to manage scroll state.

```rust
struct Scrollable {
    offset: usize,
    selected: Option<usize>,
    item_count: usize,
    visible_height: usize,  // set during render, not by caller
}
```

Methods: `scroll_up()`, `scroll_down()`, `select(idx)`, `select_next()`, `select_prev()`, `ensure_visible()`, `visible_range() -> Range<usize>`.

`visible_height` is set internally during render. Callers never pass it — this prevents mismatches between caller and renderer.

## Mouse Support

The app enables mouse capture via crossterm. Mouse events are resolved by hit-testing against grid cell rects:

- **Click** → determine which widget's area was clicked → call `widget.handle_click(row, col)`
- **Scroll wheel** → determine which widget's area the cursor is in → call `widget.handle_scroll(direction)`
- **Drag** → future concern

Widgets receive mouse coordinates relative to their own area, not the terminal.

## Built-in Widgets (v1)

### `AgentList`
Renders the sidebar list of agents with status indicators. Scrollable when agents exceed visible height. Click to select agent.

```
● Agent #1    working
  src/auth/
○ Agent #2    idle
○ Agent #3    idle
```

Reads from: `AgentStore`

### `AgentPane`
Renders the focused agent's terminal output. Scrollable through terminal scrollback. Click to focus. This is the main interaction surface — the user types into this.

Reads from: `AgentStore` (the focused agent's vt100 screen)

### `StatusBar`
Renders keybinding hints at the bottom. Auto-fits to one row.

```
ctrl+n new   ctrl+j/k navigate   ctrl+w close   ctrl+q quit
```

Reads from: `KeybindingConfig`

## Widget Registration

Widgets are identified by string name in `layout.json`. The app maps names to widget instances:

```rust
fn get_widget(name: &str) -> Option<Box<dyn Widget>> {
    match name {
        "agent-list" => Some(Box::new(AgentList)),
        "agent-pane" => Some(Box::new(AgentPane)),
        "status-bar" => Some(Box::new(StatusBar)),
        _ => None,
    }
}
```

## Rules

1. All widgets auto-fit to their assigned area. No hardcoded dimensions.
2. All widgets are scrollable when content overflows.
3. All widgets handle mouse clicks and scroll events.
4. Widgets don't know their position or size until `render()` is called. `visible_height` is set during render.
5. Widgets never mutate app state. Mouse/scroll events update widget-local scroll state only. Selection changes are routed through actions.
6. A widget that gets a zero-size area renders nothing.
