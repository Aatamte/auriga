---
last_verified: 2026-04-06
---

# Design Principles

## Code Quality Bar

"Done" means:
1. cargo test passes — no new failures
2. cargo clippy clean — no warnings
3. Matches existing patterns — look at neighboring code before writing
4. No dead code — nothing "for later"
5. No speculative features — only what was requested

## How to Write Code Here

- Read before you write. Open the file, understand what's there, then change it.
- One change at a time. Make it, test it, move on.
- Fix root causes. Don't layer patches on symptoms.
- Match the codebase. If existing code uses X pattern, use X pattern. Don't introduce Y.

## Prohibited Patterns

These are the specific ways agents produce bad code in this project:

- Don't add error handling for impossible cases. If a function can't fail, don't wrap it in Result.
- Don't add comments that restate the code. `// increment counter` above `counter += 1` is noise.
- Don't create abstractions for single use cases. Three similar lines is better than a premature trait.
- Don't add type annotations the compiler infers. Let type inference work.
- Don't refactor code you weren't asked to touch. A bug fix is a bug fix, not a cleanup opportunity.
- Don't add backwards-compatibility shims. If something is unused, delete it.
- Don't add feature flags or configuration for things that have one value.

## Error Handling

- Use anyhow::Result for application code (app, main).
- Use specific error enums for library code (agent::GenerateError).
- Propagate errors with ?. Don't match+log+ignore.
- Only validate at system boundaries (user input, file I/O, network). Trust internal types.

## Testing

- Unit tests in the same file, in a `#[cfg(test)] mod tests` block.
- Test behavior, not compilation. A test that just constructs a struct proves nothing.
- Use deterministic IDs in tests: `AgentId::from_u128(1)`.
