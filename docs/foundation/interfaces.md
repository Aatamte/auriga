# Interfaces

## Principles

- An interface defines a contract between two parts of the system. Neither side knows the other's implementation.
- Interfaces are defined by behavior, not by data. A trait describes what something can do, not what it contains.
- Every interface has a single responsibility. If a trait has methods that some implementors don't need, it should be split.
- Interfaces are the only way modules communicate. No module reaches into another module's internals.

## Design Rules

1. **Program to the interface, not the implementation.** Functions accept trait objects or generics, not concrete types, when the concrete type could vary.
2. **No leaky abstractions.** Implementation details stay behind the interface. The caller describes intent — the implementor decides how.
3. **Interfaces are tested via trait bounds.** Write tests with mock implementations. This verifies the contract, not the implementation.
4. **No god interfaces.** If a trait has more than 5-6 methods, it's doing too much. Split it.
5. **Composition over inheritance.** Components compose smaller building blocks rather than inheriting from a base. There are no base classes.
6. **Symmetry.** If an interface has a `create`, it has a `destroy`. If it has a `save`, it has a `load`. Partial lifecycles are bugs waiting to happen.

## Key Traits

### Classifier
Analyzes agent traces to detect patterns. 3 methods.
```
name() → &str
trigger() → ClassifierTrigger
classify(&self, trace, turns) → Vec<ClassificationResult>
```

### Skill
An executable capability available to agents. 4 methods.
```
name() → &str
description() → &str
trigger() → SkillTrigger
execute(&self, ctx) → Result<SkillResult>
```

### Widget
A UI component that renders into a terminal area. 3 methods.
```
render(&mut self, frame, area, ctx)
handle_scroll(&mut self, direction)
handle_click(&mut self, row, col, ctx) → Option<WidgetAction>
```

## Contract Expectations

- Every interface must document its preconditions and postconditions.
- Implementors must uphold all postconditions — the caller should never need to check whether the implementor did its job.
- Errors are part of the contract. If an operation can fail, the return type reflects that. If it can't fail, it doesn't return `Result`.
- Interfaces are stable. Once published, changing an interface signature is a breaking change that requires updating all implementors and callers. Design carefully before committing.

## Module Boundaries

- Each module exposes a public interface and hides everything else.
- A module's public interface is the smallest surface area that lets external code do what it needs.
- Circular dependencies between modules are forbidden. If A depends on B and B depends on A, there's a missing abstraction.
