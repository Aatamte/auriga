use crate::AgentId;
use std::path::PathBuf;
use std::time::Instant;

#[derive(Debug)]
pub struct FileEntry {
    pub path: PathBuf,
    pub is_dir: bool,
    pub depth: usize,
    pub expanded: bool,
    pub last_modified: Option<Instant>,
    pub modify_count: usize,
    pub modified_by: Option<AgentId>,
    pub lines_added: usize,
    pub lines_removed: usize,
}

impl FileEntry {
    pub fn dir(path: PathBuf, depth: usize) -> Self {
        Self {
            path,
            is_dir: true,
            depth,
            expanded: depth == 0,
            last_modified: None,
            modify_count: 0,
            modified_by: None,
            lines_added: 0,
            lines_removed: 0,
        }
    }

    pub fn file(path: PathBuf, depth: usize) -> Self {
        Self {
            path,
            is_dir: false,
            depth,
            expanded: false,
            last_modified: None,
            modify_count: 0,
            modified_by: None,
            lines_added: 0,
            lines_removed: 0,
        }
    }

    pub fn touch(&mut self, agent: Option<AgentId>) {
        self.last_modified = Some(Instant::now());
        self.modify_count += 1;
        if agent.is_some() {
            self.modified_by = agent;
        }
    }

    pub fn set_diff(&mut self, added: usize, removed: usize) {
        self.lines_added = added;
        self.lines_removed = removed;
    }

    pub fn age_secs(&self) -> Option<f64> {
        self.last_modified.map(|t| t.elapsed().as_secs_f64())
    }

    pub fn display_name(&self) -> &str {
        self.path.file_name().and_then(|n| n.to_str()).unwrap_or("")
    }
}
