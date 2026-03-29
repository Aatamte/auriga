# Architecture

## Overview

Agent Orchestrator is a native Rust TUI. It spawns AI CLI agents in PTYs, displays them in a grid-based dashboard, and lets the user interact with them directly.

## Layers

```
Input → Action → State Mutation → Render
```

| Layer | Responsibility | Does NOT touch |
|-------|---------------|----------------|
| Input | Raw terminal events → keybinding resolution → actions | App state |
| State | Agents, focus, config — all mutations via methods | UI, layout |
| Render | Grid → widgets → frame. Pure, read-only | State mutation |
| I/O | PTY spawning, reader threads, filesystem | UI, domain logic |

## Concurrency

- Main thread: owns state, renders, drains channels
- One background thread per agent PTY reader
- Communication: `mpsc::channel`, main thread polls non-blocking

## State

```rust
struct AppState {
    agents: AgentStore,
    focus: FocusState,
    grid: Grid,
    running: bool,
}
```

Single source of truth. Widgets read from it. Actions mutate it. Nothing else.

## Event Loop

```
loop {
    poll crossterm events
    drain PTY channels → update agent terminals
    if event → resolve action → mutate state
    render grid with current state
}
```

## File Structure

```
src/
├── main.rs              # entry point, terminal setup, event loop
├── app.rs               # AppState, action dispatch
├── agent/
│   ├── mod.rs           # Agent, AgentId, AgentStatus
│   ├── store.rs         # AgentStore — owns all agents
│   └── pty.rs           # PTY spawning, reader thread
├── grid/
│   ├── mod.rs           # Grid, Row, Cell, Size
│   └── layout.rs        # load layout.json, compute rects
├── widget/
│   ├── mod.rs           # Widget trait, widget registry
│   ├── agent_list.rs    # sidebar agent list
│   ├── agent_pane.rs    # terminal output display
│   └── status_bar.rs    # keybinding hints
├── input/
│   ├── mod.rs           # Action enum
│   └── keybindings.rs   # KeyEvent → Action resolution
└── terminal/
    └── mod.rs           # vt100::Screen → ratatui Lines
```
