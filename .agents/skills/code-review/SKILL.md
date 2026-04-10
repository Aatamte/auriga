---
name: code-review
description: "Review recent code changes for correctness, edge cases, error handling, tests, and style. Use before declaring a task done."
---

# Code Review

Use this skill whenever you are about to declare a task done. Review every change you made in this session before finishing.

For each changed file, check:

1. **Correctness** — does the logic actually do what was intended? Trace the flow with concrete values.
2. **Edge cases** — inputs or states that would break it. Empty collections, missing optionals, zero, boundary indices.
3. **Error handling** — failures are handled, not silenced. No `unwrap` on user input, no swallowed errors.
4. **Tests** — existing tests still pass. New logic has new tests.
5. **Style** — matches the surrounding code conventions.

List any issues you find. If everything looks good, say so briefly. Do not perform the review silently — produce a short written summary so the user can see what you checked.
