# Storage — Technical Specification

## Schema Versioning

```rust
const CURRENT_VERSION: i64 = 3;

pub fn init(conn: &Connection) -> rusqlite::Result<()> {
    let version = get_version(conn);
    if version < 1 { conn.execute_batch(SCHEMA_V1)?; }
    if version < 2 { conn.execute_batch(CLASSIFICATIONS_TABLE_SQL)?; }
    if version < 3 { conn.execute_batch(ML_SCHEMA_SQL)?; }
    if version < CURRENT_VERSION { set_version(conn, CURRENT_VERSION)?; }
    Ok(())
}
```

## Tables

**schema_version**
```sql
version INTEGER NOT NULL
```

**traces**
```sql
id TEXT PRIMARY KEY,
agent_id TEXT NOT NULL,
session_id TEXT NOT NULL,
status TEXT NOT NULL,
started_at TEXT NOT NULL,
completed_at TEXT,
turn_count INTEGER NOT NULL,
input_tokens INTEGER NOT NULL,
output_tokens INTEGER NOT NULL,
cache_creation_input_tokens INTEGER,
cache_read_input_tokens INTEGER,
provider TEXT NOT NULL,
model TEXT
```

**turns**
```sql
id INTEGER PRIMARY KEY,
trace_id TEXT NOT NULL REFERENCES traces(id),
agent_id TEXT NOT NULL,
number INTEGER NOT NULL,
status TEXT NOT NULL,
uuid TEXT NOT NULL,
parent_uuid TEXT,
session_id TEXT,
timestamp TEXT NOT NULL,
message_type TEXT NOT NULL,
cwd TEXT,
git_branch TEXT,
role TEXT NOT NULL,
content TEXT NOT NULL,    -- JSON (MessageContent)
meta TEXT NOT NULL,       -- JSON (TurnMeta)
extra TEXT NOT NULL       -- JSON (passthrough)
```

Indexes: `idx_turns_trace_id`, `idx_traces_agent_id`

**classifications** — see classifiers.md

**models**, **training_labels** — see classifiers.md

## StorageCommand

```rust
pub enum StorageCommand {
    SaveTrace { trace: Box<Trace>, turns: Vec<Turn> },
    SaveClassification { result: ClassificationResult },
    SaveModel { model: SavedModel },
    SaveTrainingLabel { trace_id: TraceId, classifier_name: String, label: String },
    Shutdown,
}
```

## StorageHandle

```rust
pub struct StorageHandle {
    tx: mpsc::Sender<StorageCommand>,
    join: Option<thread::JoinHandle<()>>,
}
```

Methods — all non-blocking channel sends:
```
save_trace(trace, turns)
save_classification(result)
save_model(model)
save_training_label(trace_id, classifier_name, label)
shutdown()                  — sends Shutdown, joins thread
```

Drop impl calls `shutdown()` automatically.

## Database Read Methods

```
// Traces
list_traces(limit, offset) → Vec<Trace>
load_trace(id) → Option<Trace>
load_turns(trace_id) → Vec<Turn>
list_agent_traces(agent_id) → Vec<Trace>

// Classifications
save_classification(result)
load_classifications(trace_id) → Vec<ClassificationResult>
list_recent_classifications(limit) → Vec<ClassificationResult>

// ML
save_model(model)
load_latest_model(classifier_name) → Option<SavedModel>
list_models(classifier_name) → Vec<SavedModel>
save_training_label(trace_id, classifier_name, label)
load_training_labels(classifier_name) → Vec<(TraceId, String)>

// General
metadata(db_path) → DbMetadata { file_size_bytes, tables: Vec<TableInfo>, total_rows }
query_table(table, limit, offset) → QueryResult { columns, rows, total_rows }
```

## Write Semantics

`save_trace` uses `INSERT OR REPLACE` within a transaction — upserts both the trace row and all turn rows atomically.

`save_classification` uses `INSERT OR REPLACE` — safe to call multiple times with the same ID.

## Error Handling

Background thread errors are logged via `eprintln!` and the thread continues processing. The thread loop exits when all senders are dropped or a `Shutdown` command is received.
