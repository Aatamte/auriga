# Auriga

A terminal UI for running and managing multiple AI coding agents in parallel. Monitor output, token usage, and file activity across all your agents from a single screen.

![CI](https://github.com/Aatamte/auriga/actions/workflows/ci.yml/badge.svg)

## Features

- **Multi-agent terminal multiplexer** — run up to 6 AI agents side by side with live terminal output
- **Real-time monitoring** — track token usage, turn counts, and model info per agent
- **File activity tracking** — see which files each agent is modifying, with line-level diffs
- **MCP server** — built-in [Model Context Protocol](https://modelcontextprotocol.io/) server so agents can discover and message each other
- **Trace recording** — every agent session is logged to a local SQLite database for review
- **Classifiers** — attach classifiers to agent traces to detect patterns (looping, budget overruns, etc.)
- **Customizable layout** — 12-column grid layout, configurable via `.agent-orchestrator/layout.json`

## Requirements

- macOS or Linux
- Rust 1.75+ (for building from source)

## Install

**Pre-built binary:**

```bash
curl -fsSL https://github.com/Aatamte/auriga/releases/latest/download/install.sh | bash
```

Set `INSTALL_DIR` to change the install location (default: `~/.local/bin`):

```bash
INSTALL_DIR=/usr/local/bin bash <(curl -fsSL https://github.com/Aatamte/auriga/releases/latest/download/install.sh)
```

**From source:**

```bash
git clone https://github.com/Aatamte/auriga.git
cd auriga
cargo build --release
cp target/release/aorch target/release/orchestrator-app ~/.local/bin/
```

## Quick start

```bash
aorch
```

This opens the TUI. From there:

- `Ctrl+N` — spawn a new agent
- `Ctrl+W` — close the focused agent
- Click an agent to focus it, or use the grid view to see all agents at once
- `Ctrl+Q` — quit

Navigate between pages using the tab bar at the top: **Home**, **Classifiers**, **Database**, **Settings**.

## Updating

```bash
aorch update
```

## Configuration

On first run, `aorch` creates a `.agent-orchestrator/` directory in your project root:

| File | Purpose |
|---|---|
| `config.json` | MCP port (default: 7850), disabled classifiers |
| `layout.json` | Grid layout configuration |
| `orchestrator.db` | SQLite database with traces, turns, and classifications |

## MCP integration

The orchestrator runs an MCP server on `127.0.0.1:7850` (configurable). It exposes two tools:

- `list_agents` — returns all running agents with their UUID, name, and status
- `send_message` — send a message from one agent to another

A `.mcp.json` is written to your project root while the orchestrator is running, so agents using Claude Code automatically connect.

## Contributing

```bash
cargo test       # run tests
cargo clippy     # lint
cargo fmt        # format
```

See [CLAUDE.md](CLAUDE.md) for development guidelines and [docs/](docs/) for architecture documentation.

## License

MIT
