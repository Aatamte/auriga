# Critical

## 1. No logging framework

Errors go to `eprintln!` or get silently swallowed with `let _ =`. No way to control log levels, redirect output, or integrate with observability tools.

**Files:**
- `crates/storage/src/thread.rs:95,100,105,114` — eprintln for DB errors
- `crates/app/src/main.rs:57` — `let _ = write_mcp_json()` silently fails
- `crates/app/src/app.rs:260,268` — silent channel send failures

**Fix:** Add `tracing` crate to workspace. Replace all `eprintln!` with `tracing::error!`/`warn!`. Replace `let _ =` with logged errors. Add a subscriber in `main.rs` that writes to a log file.

---

## 2. SQL string interpolation

Table names are interpolated into SQL via `format!()` instead of parameterized queries. Validated by an existence check against `sqlite_master`, but still a dangerous pattern.

**Files:**
- `crates/storage/src/query.rs:70,73-75`

**Fix:** Add identifier quoting/validation helper. Document why this is safe (table names come from `sqlite_master`, not user input). Consider using a whitelist of known table names.

---

## 3. String-based widget dispatch

Widgets are selected by hardcoded strings like `"agent-pane"`, `"token-chart"`. No compile-time checking for typos. A renamed widget silently breaks at runtime.

**Files:**
- `crates/widgets/src/lib.rs:105-118`

**Fix:** Replace with a `WidgetId` enum. Match on enum variants instead of strings. Compiler catches missing cases.
