# File Activity — Technical Specification

## FileEntry

```rust
pub struct FileEntry {
    pub path: PathBuf,
    pub is_dir: bool,
    pub depth: usize,
    pub expanded: bool,
    pub lines_added: usize,
    pub lines_removed: usize,
    pub last_modified: Option<Instant>,
    pub agent: Option<AgentId>,
}
```

## FileTree

```rust
pub struct FileTree {
    root: PathBuf,
    entries: Vec<FileEntry>,
    // Cached derived state
    visible_cache: Vec<usize>,       // indices into entries
    recent_cache: Vec<usize>,        // indices, sorted by last_modified
    dirty: bool,
}
```

Methods:
```
record_activity(path, agent_id)      — upsert file, create parent dirs, update timestamps
toggle_dir(visible_index)            — expand/collapse directory
remove_entry(path)                   — remove file and children
rename_entry(from, to, agent_id)     — update path

visible_entries() → &[usize]         — cached, invalidated on mutation
recent_activity(limit) → Vec<usize>  — cached, files only, sorted by recency
refresh_caches()                     — recompute if dirty flag set

visible_entry_at(index) → Option<&FileEntry>
index_of(path) → Option<usize>
```

Cache invalidation: any mutation sets `dirty = true`. `refresh_caches()` is called once per frame in the main loop before rendering.

## Diff Thread Protocol

```
Main thread → Sender<PathBuf>     — file path to diff
Diff thread → Receiver<DiffResult> — result

DiffResult {
    path: PathBuf,
    lines_added: usize,
    lines_removed: usize,
}
```

The diff thread runs `git diff --numstat` on each received path and parses the output.

## File Watcher

Uses `notify::RecommendedWatcher` with `ignore::WalkBuilder` filter rules. Events mapped to:

```rust
pub enum FileEvent {
    Created(PathBuf),
    Modified(PathBuf),
    Removed(PathBuf),
    Renamed { from: PathBuf, to: PathBuf },
}
```

Filtered paths: `.git/`, `node_modules/`, `target/`, and patterns from `.gitignore`.

## Activity Aging Thresholds

| Elapsed | Color |
|---|---|
| < 5s | `Color::Red` |
| < 30s | `Color::Yellow` |
| < 2min | `Color::Green` |
| < 10min | `Color::Cyan` |
| ≥ 10min | `Color::DarkGray` |

Computed via `age_color(elapsed: Duration) -> Color` in the widgets crate.
