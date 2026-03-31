use serde::Deserialize;

/// Parsed from one line of a Claude JSONL session file.
#[derive(Debug, Clone)]
pub struct ClaudeLogEntry {
    pub uuid: String,
    pub parent_uuid: Option<String>,
    pub entry_type: String,
    pub session_id: String,
    pub timestamp: String,
    pub cwd: Option<String>,
    pub git_branch: Option<String>,
    pub message: Option<serde_json::Value>,
    pub is_meta: bool,
    pub extra: serde_json::Value,
}

/// Parsed from `~/.claude/sessions/<PID>.json`.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionInfo {
    pub pid: u32,
    pub session_id: String,
    pub cwd: String,
    pub started_at: u64,
    #[serde(default)]
    pub kind: Option<String>,
}

/// Events sent from the watcher thread to the main thread.
#[derive(Debug)]
pub enum ClaudeWatchEvent {
    /// A new turn was appended to a session JSONL file.
    LogEntry(ClaudeLogEntry),
    /// A session file appeared for a PID (Claude started).
    SessionDiscovered(SessionInfo),
}
