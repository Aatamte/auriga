# Design System

## Architecture Style

The application is **event-driven**. Nothing happens proactively — everything is a response to an event. The main loop waits for events, dispatches them, and renders the result.

## Event Flow

```
Source → Event → Dispatch → State Mutation → Render
```

- **Sources** produce raw events: keyboard, mouse (click, scroll, drag), PTY output, filesystem changes, MCP requests, Claude log updates, storage responses.
- **Events** are values that describe what happened. They carry no logic.
- **Dispatch** maps events to state mutations. Keyboard events resolve through keybindings to actions. Mouse events are hit-tested against grid cell rects to determine the target widget — clicks select/focus, scroll wheel scrolls the widget under the cursor. Background events (PTY, filesystem, MCP, Claude logs) are drained from channels.
- **State mutation** is the only place application state changes. All mutations happen in one place per state owner.
- **Render** reads the current state and produces a frame. Rendering is pure — it has no side effects on application state.

## Separation of Concerns

### Input layer
Knows about raw events (keyboard, mouse, scroll). Does not know about application state. Translates raw events into domain actions or delegates to the appropriate widget via hit-testing.

### State layer
Knows about domain concepts (agents, traces, turns, classifications). Does not know about UI layout or rendering. Exposes query methods for reading and mutation methods for writing.

### Rendering layer
Knows about layout, styles, and terminal output. Does not mutate application state. Reads state through immutable references. All widgets auto-fit to their assigned area and scroll when content overflows.

### I/O layer
Knows about files, processes, databases, and the network. Does not know about UI or domain logic. Provides channels for async communication with the main loop. Includes:
- PTY readers (one per agent process)
- Filesystem watcher (file change notifications)
- Diff processor (line-level change calculations)
- Storage thread (SQLite writes)
- MCP server (HTTP/JSON-RPC for agent coordination)
- Claude log watcher (session file monitoring)

## Concurrency Model

The main thread owns all state and handles all rendering. Background threads handle blocking I/O:
- One thread per PTY reader
- One thread for filesystem watching
- One thread for diff processing
- One thread for SQLite persistence
- One thread for the MCP HTTP server
- One thread for Claude log watching

Communication is through bounded or unbounded channels. The main loop drains channels non-blocking — it never waits on a background thread.

## Performance Principles

1. **Do nothing unless something changed.** Don't recompute, re-walk, or re-scan on every frame. Track dirty flags or use event-driven updates.
2. **Amortize expensive work.** Directory walks, screen scraping, and serialization happen in response to specific events — never on a timer or frame tick.
3. **Minimize allocations in the render path.** Batch similar items. Reuse buffers where possible. The hot path is render → every 8ms — it must be cheap.
4. **Background threads for blocking I/O.** The main thread never blocks on a read, write, or process spawn.

## Error Handling

- **Recoverable errors** (file not found, process spawn failure, classification error) are reported to the user via the UI. The application continues.
- **Unrecoverable errors** (terminal setup failure, database corruption) cause a clean exit with an error message.
- **Background thread errors** are sent through channels. The main thread handles them. A background thread never panics silently.
- **State corruption** is prevented by atomic mutations. If an error occurs mid-operation, state rolls back to the previous valid state.

## Testing Philosophy

- **Unit tests** for every module with logic. Tests live in the same file as the code (`#[cfg(test)] mod tests`).
- **Test behavior, not implementation.** Tests call public methods and assert on results. They don't depend on internal data structures.
- **Mock external boundaries.** Traits exist at I/O boundaries so tests can substitute fakes. Tests never spawn real processes or touch the filesystem.
- **Tests run fast.** No sleeps, no network calls, no file I/O. If a test needs external resources, it uses in-memory fakes.
