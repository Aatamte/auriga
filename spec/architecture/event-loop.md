# Event Loop — Technical Specification

## Poll Order

```
1. poll_input()         — drain mpsc::Receiver<crossterm::event::Event>
2. poll_pty_output()    — drain per-agent PtyHandle read channels
3. poll_file_events()   — drain mpsc::Receiver<FileEvent>
4. poll_diff_results()  — drain mpsc::Receiver<DiffResult>
5. poll_mcp_requests()  — drain mpsc::Receiver<McpEvent>, send McpResponse
6. poll_claude_logs()   — drain ClaudeWatchHandle.rx, create turns/traces
7. refresh pages        — update Database/Classifiers page data if active
8. render               — terminal.draw(|frame| { ... })
9. resize check         — resize PTYs if terminal size changed
10. sleep 8ms           — thread::sleep(Duration::from_millis(8))
```

## Background Threads

| Thread | Spawn Function | Channel Type | Direction |
|---|---|---|---|
| Input | `threads::start_input_thread()` | `mpsc::Receiver<Event>` | → main |
| File watcher | `threads::start_file_watcher(tx)` | `mpsc::Receiver<FileEvent>` | → main |
| Diff processor | `threads::start_diff_thread()` | `Sender<PathBuf>` / `Receiver<DiffResult>` | main ↔ thread |
| Storage | `start_storage_thread(path)` | `Sender<StorageCommand>` | main → thread |
| MCP server | `start_mcp_server(port)` | `Receiver<McpEvent>` | → main |
| Claude watcher | `start_claude_watcher(proj, sess)` | `Receiver<ClaudeWatchEvent>` | → main |
| PTY readers | One per `spawn_pty()` | `Receiver<Vec<u8>>` (inside PtyHandle) | → main |

## Startup Sequence

```
1. config::init()                                    → Config
2. enable_raw_mode(), EnterAlternateScreen, EnableMouseCapture
3. threads::start_input_thread()                     → Receiver<Event>
4. threads::start_file_watcher(tx)                   → RecommendedWatcher
5. threads::start_diff_thread()                      → (Sender, Receiver)
6. start_mcp_server(config.mcp_port)                 → McpServer { port, rx }
7. write_mcp_json(port)                              → .mcp.json
8. start_storage_thread(db_path)                     → StorageHandle
9. Database::open(&db_path)                          → Database (read-only)
10. start_claude_watcher(project_dir, sessions_dir)  → Option<ClaudeWatchHandle>
11. App::new(all channels and state)
12. Compute initial layout rects
13. Enter main loop
```

## Shutdown Sequence

```
1. app.running = false
2. app.flush_all_traces()        — abort active, run on-complete classifiers, persist
3. app.storage.shutdown()        — send Shutdown command, join thread
4. fs::remove_file(".mcp.json")
5. DisableMouseCapture, LeaveAlternateScreen, disable_raw_mode()
```

## Channel Drain Pattern

All channels are drained with `try_recv()` in a loop:

```rust
while let Ok(event) = self.rx.try_recv() {
    // process event
}
```

This ensures all pending messages are processed each iteration without blocking.

## MCP Request-Response

```rust
// MCP thread sends:
McpEvent {
    request: McpRequest::ListAgents,
    response_tx: mpsc::Sender<McpResponse>,  // one-shot
}

// Main thread responds:
event.response_tx.send(McpResponse::Agents(agents));

// MCP thread receives response, serializes to JSON-RPC, returns HTTP response
```
