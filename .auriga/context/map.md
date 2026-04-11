---
name: map
description: Project architecture map. Modules, dependencies, entry points.
last_updated: 2026-04-06
---

# Auriga

Native Rust TUI for managing multiple AI coding agents working in the same repository.

## Tech Stack

- Language: Rust (2021 edition)
- UI: ratatui (terminal UI framework)
- Terminal: alacritty_terminal (PTY + terminal emulation)
- Storage: SQLite via rusqlite
- Build: cargo workspace

## Modules

- **core** — Domain types: Agent, AgentId, Trace, Turn, FocusState, Page. Zero dependencies. [crates/core/src/]
- **app** — Application state, event loop, input handling. Owns all stores. Entry point. [crates/app/src/]
- **widgets** — UI components: agent pane, nav bar, pages. Depends on core, grid. [crates/widgets/src/]
- **grid** — Layout engine. WidgetId enum, grid computation. [crates/grid/src/]
- **agent** — Provider trait, AgentConfig, Claude provider, sessions. [crates/agent/src/]
- **terminal** — Terminal emulator wrapper around alacritty_terminal. [crates/terminal/src/]
- **pty** — PTY process spawning and I/O. OS boundary. [crates/pty/src/]
- **storage** — SQLite persistence for traces, turns, training labels. [crates/storage/src/]
- **classifier** — Classifier registry, config loading, ML/CLI/LLM runtimes. [crates/classifier/src/]
- **skills** — Skill trait, registry, built-in skills. [crates/skills/src/]
- **ml** — ML model runtime for classifiers. [crates/ml/src/]
- **mcp** — MCP server (JSON-RPC over stdio) for external tool access. [crates/mcp/src/]
- **claude-log** — Watches Claude CLI log files, parses turns. [crates/claude-log/src/]
- **cli** — CLI binary entry point. [crates/cli/src/]
- **benches** — Benchmarks. [crates/benches/]

## Entry Points

- Main binary: crates/app/src/main.rs
- Config init: crates/app/src/config.rs → .auriga/config.json
- Agent spawn: crates/app/src/app.rs → App::spawn_agent()

## Dependency Direction

core has no dependencies. Everything else can depend on core.
widgets depends on core and grid, never on app.
app depends on everything.
agent is independent of app — app calls into agent, not the reverse.
storage is independent of app — app calls into storage.
