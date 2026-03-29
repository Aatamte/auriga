# Type System

## Principles

- Types represent domain concepts, not implementation details.
- A type should be valid by construction — if you have an instance of it, it's in a valid state.
- Types are serializable where persistence is needed, but serialization format is not part of the type's identity.
- Enums over booleans. A `Role` enum is better than `is_manager: bool` because it's extensible and self-documenting.

## Categories

### Entity Types
Long-lived, identified by a unique ID. Examples: an agent instance, a task. Entities have lifecycles — they are created, transition through states, and are destroyed. State transitions must be explicit and validated.

### Value Types
Immutable, compared by value. Examples: a configuration setting, a keybinding, a color. No identity — two values with the same data are interchangeable.

### State Types
Mutable containers that own entities or track UI state. There is exactly one of each state type in the application. All mutation goes through methods that enforce invariants.

### Event Types
Represent something that happened. Immutable once created. Flow in one direction: from the source (keyboard, mouse, PTY, filesystem) toward the state layer. Events are never stored — they are processed and discarded.

### Action Types
Represent user intent. Derived from events via keybinding resolution. The action layer is the boundary between "what happened" (event) and "what should change" (state mutation). Actions are exhaustive — every user-facing operation has an action variant.

## Rules

1. **No stringly-typed data.** If something has a fixed set of values, it's an enum.
2. **IDs are opaque.** Entity IDs are typed — you can't accidentally pass a task ID where an instance ID is expected.
3. **Collections are encapsulated.** External code never directly indexes into a `Vec`. The owning state type provides methods that enforce bounds and invariants.
4. **Optional means optional.** `Option<T>` means the absence is a normal, expected state — not an error. If absence is an error, the type should not be optional.
5. **No type aliases for primitives.** A `u16` used for width and a `u16` used for height should be distinguishable — use wrapper types or named struct fields, not bare primitives passed positionally.
