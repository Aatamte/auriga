# Agent Orchestrator

Native Rust TUI for managing multiple AI agent instances.

## Build & Run

```bash
cargo build                        # dev
cargo test                         # run before every change
cargo run                          # launch
uv run python scripts/build.py     # release binary
```

## Foundation Documents

Before writing any code, read and follow these:

- **[docs/foundation/design.md](docs/foundation/design.md)** — Architecture style, event flow, concurrency model, performance principles, error handling, testing philosophy.
- **[docs/foundation/types.md](docs/foundation/types.md)** — Type system rules. Entity vs value vs state vs event types. Validity by construction.
- **[docs/foundation/interfaces.md](docs/foundation/interfaces.md)** — Interface design rules. Contracts, module boundaries, composition.
- **[docs/foundation/state.md](docs/foundation/state.md)** — State management. Single source of truth, mutation rules, persistence layers.

## Documentation

Full docs are organized under `docs/`:

- **`docs/foundation/`** — Design philosophy, type system, interfaces, state management
- **`docs/architecture/`** — Crate structure, event loop, UI system
- **`docs/systems/`** — Traces, classifiers, storage, MCP, skills, file activity
- **`docs/cli/`** — CLI usage, install, update

## Process

1. **Read the foundation docs before making changes.**
2. **Think before coding.** Trace the logic. Ask: "What are the actual values when this runs? Does it work?"
3. **One change at a time.** Make one change, `cargo test`, verify, move on.
4. **Tests are mandatory.** Every module with logic gets unit tests in the same file. Test behavior, not compilation.
5. **Fix root causes.** Don't layer patches. Understand why something broke.
6. **No dead code.** Don't write code "for later."
7. **No speculative features.** Only build what was explicitly requested.

## Linting

```bash
cargo clippy
cargo fmt --check
uv run ruff check scripts/
```
