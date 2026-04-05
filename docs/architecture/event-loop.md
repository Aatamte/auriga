# Event Loop

## Main Loop

The application runs a continuous loop on the main thread. Each iteration drains all background channels non-blocking, refreshes any active page data, renders the current frame, and sleeps briefly to avoid busy-spinning.

The poll order is: input → PTY output → file events → diff results → MCP requests → Claude logs → page refresh → render → resize check.

All channel drains are non-blocking. If nothing is available, the step completes immediately.

## Background Threads

The main thread owns all state. Background threads handle blocking I/O and communicate exclusively through channels:

- **Input thread** — polls crossterm for keyboard and mouse events
- **File watcher** — monitors the project directory for filesystem changes using the notify crate, filtered by gitignore rules
- **Diff processor** — calculates line-level diffs for modified files via git
- **Storage thread** — executes SQLite writes on a dedicated connection
- **MCP server** — accepts HTTP requests from agents, translates to events for the main thread
- **Claude log watcher** — monitors Claude Code session files for new conversation entries
- **PTY readers** — one per agent, reads terminal output

## Channel Patterns

Three patterns are used:

**Fire-and-forget** — the main thread sends a command and moves on without waiting for confirmation. Used for storage writes.

**Request-response** — an event includes a one-shot response channel. The main thread processes the request and sends a response back. Used for MCP requests.

**Continuous stream** — background threads push data continuously. The main loop drains all available messages each iteration. Used for PTY output, file events, and Claude log entries.

## Lifecycle

On startup, the app loads configuration, enters raw terminal mode, spawns all background threads, and enters the main loop.

On shutdown (ctrl+q or terminal close), it flushes all active traces to the database, shuts down the storage thread, cleans up temporary files, and restores the terminal.
