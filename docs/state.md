# State Management

## Principles

- There is a single source of truth for all application state. No duplicated state across modules.
- State is owned, not shared. One module owns each piece of state. Other modules read it through defined interfaces.
- All mutations go through methods that enforce invariants. External code never directly modifies fields.
- State transitions are explicit. If an entity moves from one status to another, there is a method for that transition — not raw field assignment.

## Layers

### Persistent State
Survives application restarts. Serialized to disk on change or on shutdown. Loaded on startup. Missing or corrupt files fall back to defaults — never crash.

### Session State
Lives for the duration of a single application run. Not serialized. Includes things like which panel is focused, scroll offsets, transient UI state.

### Derived State
Computed from other state. Never stored — recalculated when needed. If something can be derived, it must not be cached in a way that can go stale.

## Rules

1. **No orphaned state.** If an entity is removed, all references to it are cleaned up in the same operation. No dangling IDs.
2. **No implicit coupling.** If two pieces of state must stay in sync, they are managed by the same owner. Never rely on two separate modules independently maintaining consistency.
3. **Mutations are atomic from the caller's perspective.** A method either fully succeeds or fully fails. No partial mutations that leave state inconsistent.
4. **State flows downward.** State owners pass read-only views to rendering. Rendering never mutates application state.
5. **Persistence is not the state's job.** The state layer provides serializable snapshots. A separate persistence layer handles I/O. State types are not coupled to file formats.
6. **Defaults are always valid.** A freshly initialized state container is in a usable state. There is no "uninitialized" state that requires a second setup step.
