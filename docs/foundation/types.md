# Type System

## Principles

- Types represent domain concepts, not implementation details.
- A type should be valid by construction — if you have an instance of it, it's in a valid state.
- Types are serializable where persistence is needed, but serialization format is not part of the type's identity.
- Enums over booleans. A `Role` enum is better than `is_manager: bool` because it's extensible and self-documenting.

## Categories

### Entity Types
Long-lived, identified by a unique ID. Entities have lifecycles — they are created, transition through states, and are destroyed. State transitions must be explicit and validated.

| Entity | ID Type | Lifecycle |
|---|---|---|
| Agent | `AgentId` (UUID) | Created → Idle/Working → Destroyed |
| Trace | `TraceId` (UUID) | Active → Complete/Aborted |
| Turn | `TurnId` (usize) | Created → Active → Complete |

### Value Types
Immutable, compared by value. No identity — two values with the same data are interchangeable.

| Value | Purpose |
|---|---|
| `TokenUsage` | Token counts (input, output, cache) |
| `ClassificationResult` | Output of a classifier run |
| `SkillResult` | Output of a skill execution |
| `FileEntry` | A file/directory in the tree with metadata |
| `ContentBlock` | A piece of message content (text, tool use, thinking, etc.) |
| `AgentInfo` | Summary of agent state for MCP responses |

### State Types
Mutable containers that own entities or track UI state. There is exactly one of each state type in the application. All mutation goes through methods that enforce invariants.

| State | Owns | Purpose |
|---|---|---|
| `AgentStore` | Agents | Agent creation, lookup, removal |
| `TraceStore` | Traces | Trace lifecycle, per-agent queries |
| `TurnStore` | Turns | Turn insertion, per-agent queries, token aggregation |
| `FileTree` | FileEntries | File hierarchy with expand/collapse and cache invalidation |
| `FocusState` | — | Current page, panel, active agent |
| `ClassifierRegistry` | Classifiers | Classifier dispatch by trigger type |
| `SkillRegistry` | Skills | Skill dispatch by name and trigger |
| `ScrollableState` | — | Scroll offset and selection for lists |

### Event Types
Represent something that happened. Immutable once created. Flow in one direction: from the source toward the state layer. Events are never stored — they are processed and discarded.

### Action Types
Represent user intent. Derived from events via keybinding resolution or widget interaction. The action layer is the boundary between "what happened" (event) and "what should change" (state mutation). Includes `WidgetAction` variants for UI-driven state changes.

## ID Types

All entity IDs are opaque UUID wrappers. You cannot accidentally pass one ID type where another is expected.

```rust
pub struct AgentId(pub Uuid);
pub struct TraceId(pub Uuid);
pub struct ClassificationId(pub Uuid);
pub struct SkillId(pub Uuid);
pub struct TurnId(pub usize);  // Sequential within the store
```

## Rules

1. **No stringly-typed data.** If something has a fixed set of values, it's an enum.
2. **IDs are opaque.** Entity IDs are typed — you can't accidentally pass a task ID where an instance ID is expected.
3. **Collections are encapsulated.** External code never directly indexes into a `Vec`. The owning state type provides methods that enforce bounds and invariants.
4. **Optional means optional.** `Option<T>` means the absence is a normal, expected state — not an error. If absence is an error, the type should not be optional.
5. **No type aliases for primitives.** A `u16` used for width and a `u16` used for height should be distinguishable — use wrapper types or named struct fields, not bare primitives passed positionally.
