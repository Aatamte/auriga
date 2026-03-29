# Crates

## Structure

```
crates/
├── core/          # Agent, AgentId, AgentStatus, AgentStore, AppState, FocusState
├── grid/          # Grid, Row, Cell, Size, layout.json loading/parsing
├── widgets/       # Widget trait, AgentList, AgentPane, StatusBar, widget registry
├── terminal/      # vt100::Screen → ratatui Lines conversion
├── pty/           # PTY spawning, reader thread, PtyHandle
└── app/           # main.rs, event loop, input handling, wires everything together
```

## Dependencies

```
app → widgets → grid → core
                     → terminal
             → core
    → pty → core
    → core
```

- `core` depends on nothing internal. Pure domain types.
- `pty` depends on `core` (for AgentId) and `portable-pty`.
- `terminal` depends on `vt100` and `ratatui` (for Line/Span types).
- `grid` depends on `core` (for AppState) and `serde` (for layout.json).
- `widgets` depends on `core`, `grid`, `terminal`, and `ratatui`.
- `app` depends on everything. It's the binary crate.

## Rules

1. `core` has zero external dependencies beyond `serde`. It compiles fast and tests fast.
2. Only `app` knows how to wire crates together. No crate reaches across to another's internals.
3. Each crate has its own `#[cfg(test)] mod tests`.
4. `pty` is the only crate that spawns processes or threads.
