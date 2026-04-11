# State Management

## Principles

- There is a single source of truth for all application state. No duplicated state across modules.
- State is owned, not shared. One module owns each piece of state. Other modules read it through defined interfaces.
- All mutations go through methods that enforce invariants. External code never directly modifies fields.
- State transitions are explicit. If an entity moves from one status to another, there is a method for that transition — not raw field assignment.

## Layers

### Persistent State
Survives application restarts. Includes:
- **SQLite database** — traces, turns, classifications, ML models, training labels. Managed by the storage thread. Schema versioned with migrations.
- **Config file** — `.auriga/config.json`. MCP port, disabled classifiers. Loaded on startup, saved on change.
- **Layout file** — `.auriga/layout.json`. Grid layout configuration. Loaded on startup.

Missing or corrupt files fall back to defaults — never crash.

### Session State
Lives for the duration of a single application run. Not serialized. Includes:
- `FocusState` — current page, focused panel, active agent
- `ScrollableState` — scroll offsets and selections per widget
- `FileTree` — in-memory file hierarchy built from filesystem events
- `AgentStore`, `TraceStore`, `TurnStore` — in-memory entity stores
- `ClassifierRegistry`, `SkillRegistry` — registered plugins
- PTY handles, terminal emulators, VTE parsers
- Session/PID maps for Claude log association

### Derived State
Computed from other state. Never stored — recalculated when needed.
- File tree visible entries (filtered by expand/collapse state, cached with invalidation)
- Recent activity lists (sorted by modification time)
- Token usage aggregations per agent
- Widget display data (formatted strings, status views)

## State Owners

The `App` struct is the top-level state owner. It holds all stores, registries, and channels. No state lives outside of `App` except within background threads (which communicate via channels).

| Owner | State | Mutation Pattern |
|---|---|---|
| `AgentStore` | Agents | `create()`, `remove()`, `get_mut()` |
| `TraceStore` | Traces | `create()`, `complete()`, `abort()`, `take_finished()` |
| `TurnStore` | Turns | `insert()`, `complete()`, `remove_agent_turns()` |
| `FileTree` | File entries | `record_activity()`, `toggle_dir()`, `remove_entry()` |
| `ClassifierRegistry` | Classifiers | `register()`, `set_enabled()` |
| `SkillRegistry` | Skills | `register()`, `set_enabled()` |
| `StorageHandle` | DB writes | `save_trace()`, `save_classification()` (non-blocking channel sends) |

## Rules

1. **No orphaned state.** If an entity is removed, all references to it are cleaned up in the same operation. No dangling IDs.
2. **No implicit coupling.** If two pieces of state must stay in sync, they are managed by the same owner. Never rely on two separate modules independently maintaining consistency.
3. **Mutations are atomic from the caller's perspective.** A method either fully succeeds or fully fails. No partial mutations that leave state inconsistent.
4. **State flows downward.** State owners pass read-only views to rendering. Rendering never mutates application state.
5. **Persistence is not the state's job.** The state layer provides serializable snapshots. A separate persistence layer handles I/O. State types are not coupled to file formats.
6. **Defaults are always valid.** A freshly initialized state container is in a usable state. There is no "uninitialized" state that requires a second setup step.
