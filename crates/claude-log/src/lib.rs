mod parser;
mod types;
pub mod watcher;

pub use parser::{parse_log_line, parse_session_file, to_turn_builder};
pub use types::{ClaudeLogEntry, ClaudeWatchEvent, SessionInfo};
pub use watcher::{
    claude_project_dir, claude_sessions_dir, start_claude_watcher, ClaudeWatchHandle,
};
