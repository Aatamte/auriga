# Crates

The workspace contains 14 crates organized by responsibility. Dependencies flow downward — `core` depends on nothing, `app` depends on everything.

## Layers

**Domain layer** — `core` defines all shared types, entity stores, and state containers. Every other crate depends on it. It has zero dependencies on other workspace crates.

**Infrastructure layer** — crates that handle I/O and external concerns:
- `pty` — process spawning and terminal I/O
- `terminal` — terminal emulation rendering
- `mcp` — HTTP server for agent coordination
- `claude-log` — Claude Code session file monitoring
- `storage` — SQLite persistence

**Analysis layer** — crates that analyze agent behavior:
- `classifier` — trait and registry for trace classification
- `ml` — machine learning models implementing the classifier trait
- `skills` — trait and registry for agent capabilities

**Presentation layer** — crates that handle UI:
- `grid` — layout engine
- `widgets` — all UI components

**Entry points** — binary crates:
- `app` — the TUI binary, wires everything together
- `cli` — the `aorch` binary, launches the TUI and handles updates

**Testing** — `benches` for criterion benchmarks.

## Rules

- `core` has zero dependencies on other workspace crates.
- Only `app` imports all crates — it is the top-level compositor.
- No circular dependencies. If two crates need to share a type, it goes in `core`.
- Each crate has a clean `lib.rs` that re-exports only the public API.
