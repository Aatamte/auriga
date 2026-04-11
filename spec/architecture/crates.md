# Crates — Technical Specification

## Dependency Graph

```
                        app (TUI binary)
                       / | \  \   \   \
                      /  |  \  \   \   \
                 widgets |  pty \   \   \
                  /  |   |   |   \   \   \
                 /   | terminal  \   \   \
                /    |            \   \   \
              grid   |         storage \   \
                     |        / |  \    \   \
                     |       /  |   \    \   \
                     |   classifier ml  mcp  claude-log
                     |       |   |
                     |       |   |
                     +-------+---+---- core
                                        |
                                       (no deps)

                    cli (binary)        benches
                     |                   |
                    (standalone)        core
```

## Per-Crate Public API

### core (`auriga-core`)
```
AgentId, AgentStore, AgentStatus
TraceId, Trace, TraceStore, TraceStatus
TurnId, Turn, TurnStore, TurnBuilder, TurnStatus
TokenUsage, MessageContent, ContentBlock, TurnRole, TurnMeta
MessageType, AssistantMeta, UserMeta, SystemMeta, StopReason
FileTree, FileEntry
FocusState, Page, Panel
ScrollableState
```

### grid (`auriga-grid`)
```
Grid, CellRect, WidgetId, Size
```

### widgets (`auriga-widgets`)
```
Widget (trait), WidgetRegistry, WidgetAction, RenderContext, ScrollDirection
DbMetadataView, QueryResultView, TableInfoView
SettingsField, ClassifierStatusView, ClassificationResultView
```

### terminal (`auriga-terminal`)
```
render_term(term, buf, area)
```

### pty (`auriga-pty`)
```
PtyHandle, spawn_pty(cmd, size) -> PtyHandle
```

### mcp (`auriga-mcp`)
```
start_mcp_server(port) -> McpServer
McpServer { port, rx }
McpEvent { request, response_tx }
McpRequest { ListAgents, SendMessage { from, to, message } }
McpResponse { Agents(Vec<AgentInfo>), MessageSent, Error(String) }
AgentInfo { id, name, status }
```

### claude-log (`auriga-claude-log`)
```
start_claude_watcher(project_dir, sessions_dir) -> ClaudeWatchHandle
ClaudeWatchHandle { rx }
ClaudeWatchEvent { entries: Vec<LogEntry> }
LogEntry { session_id, uuid, parent_uuid, ... }
to_turn_builder(entry, agent_id) -> TurnBuilder
claude_project_dir() -> Option<PathBuf>
claude_sessions_dir() -> Option<PathBuf>
```

### storage (`auriga-storage`)
```
Database { open(path), open_in_memory() }
  — save_trace, load_trace, list_traces, load_turns, list_agent_traces
  — save_classification, load_classifications, list_recent_classifications
  — save_model, load_latest_model, list_models
  — save_training_label, load_training_labels
  — metadata, query_table
StorageHandle { save_trace, save_classification, save_model, shutdown }
start_storage_thread(path) -> StorageHandle
DbMetadata, QueryResult, TableInfo
```

### classifier (`auriga-classifier`)
```
Classifier (trait) { name, trigger, classify }
ClassifierRegistry { register, set_enabled, run_incremental, run_on_complete, classifiers_info }
ClassifierTrigger { Incremental, OnComplete, Both }
ClassificationResult { id, trace_id, classifier_name, timestamp, payload }
ClassificationId, ClassifierStatus
CLASSIFICATIONS_TABLE_SQL
```

### ml (`auriga-ml`)
```
DecisionTreeClassifier (implements Classifier)
SavedModel { id, classifier_name, version, feature_names, model_data, accuracy }
TrainResult { model, accuracy, n_train, n_test, label_distribution }
TrainParams { max_depth, min_weight_split, test_ratio }
extract_features(trace, turns) -> Vec<f64>
train_decision_tree(data) -> TrainResult
FEATURE_NAMES: &[&str]  (14 features)
ML_SCHEMA_SQL
```

### skills (`auriga-skills`)
```
Skill (trait) { name, description, trigger, execute }
SkillRegistry { register, execute, run_session_start, run_session_end, set_enabled, skills_info }
SkillTrigger { OnDemand, OnSessionStart, OnSessionEnd }
SkillContext { agent_id, arguments }
SkillResult { id, skill_name, agent_id, timestamp, success, payload }
SkillId, SkillStatus
```

### cli (`auriga-cli`)
```
Binary: auriga
Commands: (none) → launch, update, version, help
Deps: anyhow, self_update
```

### app (`auriga-app`)
```
Binary: auriga-app
Deps: all workspace crates
```

### benches (`auriga-benches`)
```
Criterion benchmarks
Deps: core, criterion
```
