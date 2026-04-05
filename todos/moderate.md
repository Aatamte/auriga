# Moderate

## 1. Missing test coverage

Widget rendering logic and input handling are largely untested.

**Files:**
- `crates/widgets/src/token_chart.rs` — 0 tests
- `crates/app/src/input.rs` — no tests for `handle_key()`, `handle_mouse()`
- Widget rendering in general — no snapshot or behavior tests

**Fix:** Add unit tests for widget state transitions (scroll, click, toggle). Test input dispatch maps correct keys to correct actions.

---

## 2. Inconsistent parameter types

Some methods take `&PathBuf` instead of the idiomatic `&Path`.

**Files:**
- `crates/core/src/file_activity.rs:56,60-61`

**Fix:** Change signatures to accept `&Path`. Callers with `PathBuf` can pass `&path` directly.

---

## 3. Claude log parser swallows errors

Missing required fields silently become empty strings instead of returning errors. Makes it impossible to distinguish valid entries from corrupted ones.

**Files:**
- `crates/claude-log/src/parser.rs:13-68`

**Fix:** Return `Result` for required fields (uuid, sessionId). Only default for truly optional fields.

---

## 4. TokenUsage.total() is misleading

Only sums `input_tokens + output_tokens`, ignoring cache creation and cache read tokens which are also billed.

**Files:**
- `crates/core/src/turn.rs:110-112`

**Fix:** Either include cache tokens in the total or rename to `base_tokens()` to make the semantics clear.

---

## 5. NavBar recomputes layout every call

`TabLayout::compute()` is called on every hit-test and render despite the tab layout being static.

**Files:**
- `crates/widgets/src/nav_bar.rs:14-34`

**Fix:** Compute once on resize and cache the result.
