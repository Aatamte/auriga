use notify::{Event, EventKind, RecursiveMode, Watcher};
use std::collections::HashMap;
use std::fs::File;
use std::io::{BufRead, BufReader, Seek, SeekFrom};
use std::path::PathBuf;
use std::sync::mpsc;

use crate::parser::{parse_log_line, parse_session_file};
use crate::types::ClaudeWatchEvent;

pub struct ClaudeWatchHandle {
    pub rx: mpsc::Receiver<ClaudeWatchEvent>,
    _watcher: Box<dyn Watcher + Send>,
}

impl ClaudeWatchHandle {
    pub fn try_recv(&self) -> Option<ClaudeWatchEvent> {
        self.rx.try_recv().ok()
    }
}

/// Start watching Claude's project JSONL directory and sessions directory.
/// Returns a handle with a receiver for parsed events.
pub fn start_claude_watcher(
    project_dir: PathBuf,
    sessions_dir: PathBuf,
) -> anyhow::Result<ClaudeWatchHandle> {
    let (tx, rx) = mpsc::channel();

    // Track file read positions to only parse new lines
    let mut positions: HashMap<PathBuf, u64> = HashMap::new();

    // Initialize positions for existing JSONL files (skip existing content)
    if project_dir.exists() {
        if let Ok(entries) = std::fs::read_dir(&project_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().is_some_and(|e| e == "jsonl") {
                    if let Ok(meta) = std::fs::metadata(&path) {
                        positions.insert(path, meta.len());
                    }
                }
            }
        }
    }

    let project_dir_clone = project_dir.clone();
    let sessions_dir_clone = sessions_dir.clone();
    let tx_clone = tx.clone();

    let mut watcher = notify::recommended_watcher(move |res: notify::Result<Event>| {
        let Ok(event) = res else { return };

        for path in &event.paths {
            // Session file created → discover session
            if path.parent() == Some(sessions_dir_clone.as_path()) {
                if let EventKind::Create(_) | EventKind::Modify(_) = event.kind {
                    if path.extension().is_some_and(|e| e == "json") {
                        if let Ok(info) = parse_session_file(path) {
                            let _ = tx_clone.send(ClaudeWatchEvent::SessionDiscovered(info));
                        }
                    }
                }
                continue;
            }

            // JSONL file modified → read new lines
            if path.parent() == Some(project_dir_clone.as_path()) {
                if let EventKind::Create(_) | EventKind::Modify(_) = event.kind {
                    if path.extension().is_some_and(|e| e == "jsonl") {
                        read_new_lines(path, &mut positions, &tx_clone);
                    }
                }
            }
        }
    })?;

    if project_dir.exists() {
        watcher.watch(&project_dir, RecursiveMode::NonRecursive)?;
    }
    if sessions_dir.exists() {
        watcher.watch(&sessions_dir, RecursiveMode::NonRecursive)?;
    }

    Ok(ClaudeWatchHandle {
        rx,
        _watcher: Box::new(watcher),
    })
}

fn read_new_lines(
    path: &PathBuf,
    positions: &mut HashMap<PathBuf, u64>,
    tx: &mpsc::Sender<ClaudeWatchEvent>,
) {
    let pos = positions.get(path).copied().unwrap_or(0);

    let Ok(mut file) = File::open(path) else {
        return;
    };
    if file.seek(SeekFrom::Start(pos)).is_err() {
        return;
    }

    let reader = BufReader::new(&mut file);
    let mut new_pos = pos;

    for line in reader.lines() {
        let Ok(line) = line else { break };
        new_pos += line.len() as u64 + 1; // +1 for newline
        if line.trim().is_empty() {
            continue;
        }
        if let Ok(entry) = parse_log_line(&line) {
            let _ = tx.send(ClaudeWatchEvent::LogEntry(entry));
        }
    }

    positions.insert(path.clone(), new_pos);
}

/// Compute the Claude project directory for the current working directory.
pub fn claude_project_dir() -> Option<PathBuf> {
    let cwd = std::env::current_dir().ok()?;
    let encoded = cwd.to_string_lossy().replace('/', "-");
    let home = dirs::home_dir()?;
    Some(home.join(".claude").join("projects").join(encoded))
}

/// Path to Claude's sessions directory.
pub fn claude_sessions_dir() -> Option<PathBuf> {
    let home = dirs::home_dir()?;
    Some(home.join(".claude").join("sessions"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn claude_project_dir_encodes_path() {
        // This test just checks the function doesn't panic
        // The actual path depends on the current directory
        let dir = claude_project_dir();
        assert!(dir.is_some());
        let dir = dir.unwrap();
        assert!(dir.to_string_lossy().contains(".claude/projects/"));
    }

    #[test]
    fn claude_sessions_dir_returns_path() {
        let dir = claude_sessions_dir();
        assert!(dir.is_some());
        assert!(dir.unwrap().to_string_lossy().contains(".claude/sessions"));
    }
}
