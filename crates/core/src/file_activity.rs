use auriga_types::{AgentId, FileActivity};
use std::path::PathBuf;

#[derive(Debug)]
pub struct FileActivityStore {
    entries: Vec<FileActivity>,
}

impl FileActivityStore {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    pub fn record(&mut self, path: PathBuf, agent: Option<AgentId>) {
        if let Some(entry) = self.entries.iter_mut().find(|e| e.path == path) {
            entry.touch(agent);
        } else {
            self.entries.push(FileActivity::new(path, agent));
        }
    }

    pub fn remove(&mut self, path: &PathBuf) {
        self.entries.retain(|e| &e.path != path);
    }

    pub fn rename(&mut self, from: &PathBuf, to: PathBuf, agent: Option<AgentId>) {
        if let Some(entry) = self.entries.iter_mut().find(|e| &e.path == from) {
            entry.path = to;
            entry.touch(agent);
        } else {
            self.entries.push(FileActivity::new(to, agent));
        }
    }

    /// Returns entries sorted by last_modified (most recent first)
    pub fn sorted(&self) -> Vec<&FileActivity> {
        let mut sorted: Vec<&FileActivity> = self.entries.iter().collect();
        sorted.sort_by(|a, b| b.last_modified.cmp(&a.last_modified));
        sorted
    }

    pub fn count(&self) -> usize {
        self.entries.len()
    }
}

impl Default for FileActivityStore {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn record_creates_new_entry() {
        let mut store = FileActivityStore::new();
        store.record(PathBuf::from("src/main.rs"), None);
        assert_eq!(store.count(), 1);
    }

    #[test]
    fn record_same_path_increments_count() {
        let mut store = FileActivityStore::new();
        store.record(PathBuf::from("src/main.rs"), None);
        store.record(PathBuf::from("src/main.rs"), None);
        let sorted = store.sorted();
        assert_eq!(sorted.len(), 1);
        assert_eq!(sorted[0].modify_count, 2);
    }

    #[test]
    fn record_updates_agent() {
        let mut store = FileActivityStore::new();
        store.record(PathBuf::from("src/main.rs"), Some(AgentId::from_u128(1)));
        store.record(PathBuf::from("src/main.rs"), Some(AgentId::from_u128(2)));
        let sorted = store.sorted();
        assert_eq!(sorted[0].modified_by, Some(AgentId::from_u128(2)));
    }

    #[test]
    fn remove_deletes_entry() {
        let mut store = FileActivityStore::new();
        store.record(PathBuf::from("src/main.rs"), None);
        store.remove(&PathBuf::from("src/main.rs"));
        assert_eq!(store.count(), 0);
    }

    #[test]
    fn rename_updates_path() {
        let mut store = FileActivityStore::new();
        store.record(PathBuf::from("old.rs"), None);
        store.rename(
            &PathBuf::from("old.rs"),
            PathBuf::from("new.rs"),
            Some(AgentId::from_u128(1)),
        );
        assert_eq!(store.count(), 1);
        let sorted = store.sorted();
        assert_eq!(sorted[0].path, PathBuf::from("new.rs"));
        assert_eq!(sorted[0].modified_by, Some(AgentId::from_u128(1)));
    }
}
