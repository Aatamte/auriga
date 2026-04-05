# Major

## 1. Render path allocations

Hot path runs every 8ms. Multiple `format!()` and `.to_string()` calls allocate on every frame.

**Files:**
- `crates/app/src/app.rs:395` — `format!("{:?}", a.status)` in poll loop
- `crates/widgets/src/file_tree_widget.rs:43,54` — string allocs every render
- `crates/widgets/src/settings_page.rs:173,177,198` — format! per field per frame
- `crates/widgets/src/classifiers_page.rs:176,222` — string format every render

**Fix:** Pre-format and cache display strings. Use `Cow<str>` or store formatted values on state change instead of recomputing every frame.

---

## 2. Excessive cloning

~20 `.clone()` calls in `app.rs`, many in hot paths. Full structs cloned during event processing.

**Files:**
- `crates/app/src/app.rs:365` — full `Trace` struct cloned
- `crates/app/src/app.rs:260,268` — `PathBuf` cloned per file event
- `crates/app/src/app.rs:121,162,210,290,328,330,357,473,548` — various clones

**Fix:** Use references where possible. For channel sends that require ownership, consider `Arc` for large structs or restructure to avoid the clone.

---

## 3. Monolithic app.rs

659 lines handling PTY lifecycle, file events, MCP requests, Claude logs, DB queries, widget dispatch, and trace flushing. Violates single responsibility.

**Files:**
- `crates/app/src/app.rs`

**Fix:** Extract into modules:
- `app/pty_manager.rs` — PTY lifecycle, resize, output polling
- `app/event_handler.rs` — file events, diff results
- `app/mcp_handler.rs` — MCP request processing
- `app/trace_manager.rs` — trace flushing and storage

---

## 4. Public fields bypass invariants

Direct field access allows breaking internal consistency without going through methods that enforce bounds or invalidate caches.

**Files:**
- `crates/core/src/scrollable.rs:11-12` — `pub offset`, `pub selected`
- `crates/core/src/file_tree.rs:8-11,72` — `pub path`, `pub expanded`, `pub root`

**Fix:** Make fields private. Add accessor methods. `set_expanded()` should invalidate caches. `ScrollableState` setters should enforce bounds.

---

## 5. Unwraps in non-test code

Production code that panics on unexpected input instead of returning errors.

**Files:**
- `crates/core/src/file_tree.rs:182-183` — unwrap in sort comparison
- `crates/widgets/src/database_page.rs:147` — `self.current_query().unwrap()`
- `crates/mcp/src/lib.rs:64,89-90` — unwraps in JSON serialization

**Fix:** Replace with proper error handling. Use `unwrap_or`, `?` operator, or `expect()` with a message explaining why it's safe.

---

## 6. PTY state split across 3 HashMaps

`ptys`, `terms`, and `vte_parsers` must stay in sync manually. Adding or removing an agent requires updating all three. Missing entry causes panic.

**Files:**
- `crates/app/src/app.rs` — three separate HashMaps

**Fix:** Create an `AgentTerminal` struct bundling `PtyHandle`, `Term`, and `Processor`. Single `HashMap<AgentId, AgentTerminal>`.
