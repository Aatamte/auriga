# File Activity Widget

## What It Is

A scrollable panel that shows files in the working directory, color-coded by recent activity. Hot files (recently/frequently modified) stand out visually. The user can see at a glance where agents are making changes.

## Activity Model

```rust
struct FileActivity {
    path: PathBuf,
    last_modified: Instant,
    modify_count: usize,
    modified_by: Option<AgentId>,
}
```

Activity is tracked per-file. Each filesystem event (create, modify, rename) updates the entry. The widget color-codes based on how recently and how frequently a file was touched.

## Color Scale

Based on time since last modification:

| Age | Color | Meaning |
|-----|-------|---------|
| < 5s | Red | Active right now |
| < 30s | Yellow | Recently changed |
| < 2min | Green | Changed not long ago |
| < 10min | Cyan | Earlier this session |
| > 10min | DarkGray | Stale |

Modify count amplifies intensity — a file modified 10 times in the last minute is "hotter" than one modified once.

## Display

```
 FILES
 ──────────────────────────
 src/auth/login.rs        ● Agent #1
 src/auth/session.rs      ● Agent #1
 src/main.rs
 Cargo.toml
 README.md
```

- Files sorted by last_modified (most recent first)
- Agent attribution shown when known
- Scrollable when file list exceeds visible height
- Respects .gitignore (uses `ignore` crate)

## Filesystem Watcher

- Background thread watches the working directory recursively via `notify`
- Sends events over `mpsc::channel` to the main loop
- Main loop correlates events to the currently active agent (the one most recently written to)
- Debounce: batch rapid events on the same file

## Data Store

```rust
struct FileActivityStore {
    entries: Vec<FileActivity>,
    watcher_rx: mpsc::Receiver<FileEvent>,
}
```

Lives in `AppState`. The main loop drains `watcher_rx` each tick, updates entries, and the widget reads from the store.

## Widget

```rust
struct FileActivityWidget {
    scroll: Scrollable,
}
```

- Auto-fits to assigned grid area
- Scrollable via mouse wheel and keyboard
- Click a file to... (future: jump to it, show diff, etc.)
- Renders using the color scale above based on `Instant::elapsed()`

## Crate Placement

Goes in `core` (FileActivity, FileActivityStore) and `widgets` (FileActivityWidget). Watcher thread setup goes in a new `watcher` crate or in `app` directly for now.

## Dependencies

- `notify` — filesystem event watching (already in workspace)
- `ignore` — .gitignore-aware filtering (already in workspace)
- `std::time::Instant` — activity timing
