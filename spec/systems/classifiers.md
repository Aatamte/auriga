# Classifiers — Technical Specification

## Classifier Trait

```rust
pub trait Classifier: Send + Sync {
    fn name(&self) -> &str;
    fn trigger(&self) -> ClassifierTrigger;
    fn classify(&self, trace: &Trace, turns: &[Turn]) -> Vec<ClassificationResult>;
}
```

## Types

```rust
pub enum ClassifierTrigger {
    Incremental,    // runs_incremental() = true
    OnComplete,     // runs_on_complete() = true
    Both,           // both = true
}

pub struct ClassificationResult {
    pub id: ClassificationId,       // UUID
    pub trace_id: TraceId,
    pub classifier_name: String,
    pub timestamp: String,          // ISO 8601
    pub payload: serde_json::Value,
}

pub struct ClassifierStatus {
    pub name: String,
    pub trigger: ClassifierTrigger,
    pub enabled: bool,
}
```

## Registry

```rust
struct ClassifierEntry {
    classifier: Box<dyn Classifier>,
    enabled: bool,
}

pub struct ClassifierRegistry {
    entries: Vec<ClassifierEntry>,
}
```

Methods:
```
register(classifier)                    — panics on duplicate name
count() → usize
names() → Vec<&str>
set_enabled(name, bool) → bool         — returns false if not found
is_enabled(name) → bool
classifiers_info() → Vec<ClassifierStatus>
run_incremental(trace, turns) → Vec<ClassificationResult>
run_on_complete(trace, turns) → Vec<ClassificationResult>
```

Dispatch logic: iterate entries, skip disabled, skip non-matching trigger, call `classify()`, collect results.

## Trigger Points in App

**Incremental** — `app.rs::poll_claude_logs()`:
```
new turn inserted → trace updated →
  let results = classifier_registry.run_incremental(&trace, &turns);
  for r in results { storage.save_classification(r); }
```

**On-complete** — `app.rs::flush_finished_traces()`:
```
trace marked Complete/Aborted →
  let results = classifier_registry.run_on_complete(&trace, &turns);
  for r in results { storage.save_classification(r); }
```

## Feature Extraction (ML)

14 features extracted by `extract_features(trace, turns) -> Vec<f64>`:

| Index | Feature | Source |
|---|---|---|
| 0 | turn_count | trace.turn_count |
| 1 | input_tokens | trace.token_usage |
| 2 | output_tokens | trace.token_usage |
| 3 | total_tokens | input + output |
| 4 | duration_secs | completed_at - started_at |
| 5 | assistant_turn_count | count MessageType::Assistant |
| 6 | user_turn_count | count MessageType::User |
| 7 | tool_use_count | count ContentBlock::ToolUse |
| 8 | tool_error_count | count ToolResult with is_error |
| 9 | thinking_block_count | count ContentBlock::Thinking |
| 10 | unique_tool_count | distinct tool names |
| 11 | avg_output_per_assistant | output_tokens / assistant_turns |
| 12 | error_rate | tool_errors / tool_uses |
| 13 | text_length_total | sum of all text content lengths |

## Training Pipeline

```
train_decision_tree(data: Vec<(Trace, Vec<Turn>, String)>) → TrainResult

TrainParams { max_depth: Option<usize>, min_weight_split: f64, test_ratio: f64 }
TrainResult { model: SavedModel, accuracy: f64, n_train, n_test, label_distribution }

SavedModel {
    id: String,
    classifier_name: String,
    version: u32,
    created_at: String,
    feature_names: Vec<String>,
    model_data: serde_json::Value,   // serialized linfa DecisionTree
    accuracy: f64,
}
```

## Persistence

SQL schema defined in `CLASSIFICATIONS_TABLE_SQL`:
```sql
CREATE TABLE IF NOT EXISTS classifications (
    id TEXT PRIMARY KEY,
    trace_id TEXT NOT NULL REFERENCES traces(id),
    classifier_name TEXT NOT NULL,
    timestamp TEXT NOT NULL,
    payload TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_classifications_trace_id ON classifications(trace_id);
CREATE INDEX IF NOT EXISTS idx_classifications_classifier_name ON classifications(classifier_name);
```

ML schema defined in `ML_SCHEMA_SQL`:
```sql
CREATE TABLE IF NOT EXISTS models (
    id TEXT PRIMARY KEY,
    classifier_name TEXT NOT NULL,
    version INTEGER NOT NULL,
    created_at TEXT NOT NULL,
    feature_names TEXT NOT NULL,
    model_data TEXT NOT NULL,
    accuracy REAL NOT NULL
);
CREATE TABLE IF NOT EXISTS training_labels (
    trace_id TEXT NOT NULL,
    classifier_name TEXT NOT NULL,
    label TEXT NOT NULL,
    PRIMARY KEY (trace_id, classifier_name)
);
```
