# Agent

## What It Is

An agent is a running CLI process (e.g. `claude`, `codex`) managed by the orchestrator. The user talks to agents directly through their terminal. The orchestrator spawns, tracks, and displays them — it does not intercept or modify their input/output.

## State

```rust
struct Agent {
    id: AgentId,
    name: String,
    status: AgentStatus,
    provider: String,       // "claude", "codex", etc.
    working_dir: PathBuf,   // where the agent was spawned
    pty: PtyHandle,         // writer + reader channel
    terminal: vt100::Parser,
}

struct AgentId(usize);

enum AgentStatus {
    Idle,
    Working,
}
```

## Operations

| Operation | Description |
|-----------|-------------|
| `spawn(provider, working_dir)` | Start a new agent process in a PTY |
| `kill(id)` | Terminate the agent's process |
| `write(id, bytes)` | Send raw input to the agent's stdin |
| `poll(id)` | Drain new output from the PTY reader channel into the terminal parser |
| `screen(id)` | Get the current vt100 screen for rendering |
| `status(id)` | Get the agent's current status |

## Store

```rust
struct AgentStore {
    agents: Vec<Agent>,
    next_id: usize,
}
```

All access goes through `AgentStore`. External code never holds a direct reference to an `Agent` — it uses `AgentId` and asks the store.

| Method | Description |
|--------|-------------|
| `create(provider, working_dir) -> AgentId` | Spawn agent, add to store, return ID |
| `remove(id)` | Kill process, remove from store |
| `get(id) -> Option<&Agent>` | Read-only access |
| `get_mut(id) -> Option<&mut Agent>` | Mutable access (for polling output) |
| `list() -> &[Agent]` | All agents |
| `count() -> usize` | Number of active agents |

## Lifecycle

```
spawn → Idle → Working → Idle → ... → kill
```

Status detection is based on PTY output activity. If an agent is producing output, it's `Working`. When output stops, it returns to `Idle`. The orchestrator does not parse the content — just the presence/absence of output over a time window.

## PTY Model

Each agent owns:
- A **writer** handle — sends bytes to the agent's stdin
- A **reader** channel — receives output chunks from a dedicated background thread
- A **terminal parser** — processes raw bytes into a virtual screen

The reader thread is spawned when the agent is created and dies when the agent is killed. It sends `Vec<u8>` chunks over an `mpsc::channel`. The main thread drains this non-blocking via `try_recv()`.

## Persistence

Agents are **session state** — they exist only while the orchestrator is running. No serialization to disk in v1. When the orchestrator exits, all agent processes are killed.
