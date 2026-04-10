use orchestrator_types::{AgentId, FileEntry};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

#[derive(Debug)]
pub struct FileTree {
    pub root: PathBuf,
    entries: Vec<FileEntry>,
    // Path → index for O(1) lookup
    index: HashMap<PathBuf, usize>,
    // Cached visible entries (invalidated on mutation)
    visible_cache: Option<Vec<usize>>,
    // Cached recent activity (invalidated on mutation)
    recent_cache: Option<Vec<usize>>,
}

impl FileTree {
    pub fn new(root: PathBuf) -> Self {
        Self {
            root,
            entries: Vec::new(),
            index: HashMap::new(),
            visible_cache: None,
            recent_cache: None,
        }
    }

    fn rebuild_index(&mut self) {
        self.index.clear();
        for (i, entry) in self.entries.iter().enumerate() {
            self.index.insert(entry.path.clone(), i);
        }
    }

    fn invalidate_caches(&mut self) {
        self.visible_cache = None;
        self.recent_cache = None;
    }

    pub fn set_entries(&mut self, entries: Vec<FileEntry>) {
        self.entries = entries;
        self.rebuild_index();
        self.invalidate_caches();
    }

    pub fn entries(&self) -> &[FileEntry] {
        &self.entries
    }

    /// Returns indices of visible entries (parents are expanded)
    fn compute_visible(&self) -> Vec<usize> {
        let mut visible = Vec::new();
        let mut collapsed_depth: Option<usize> = None;

        for (i, entry) in self.entries.iter().enumerate() {
            if let Some(cd) = collapsed_depth {
                if entry.depth > cd {
                    continue;
                }
                collapsed_depth = None;
            }

            visible.push(i);

            if entry.is_dir && !entry.expanded {
                collapsed_depth = Some(entry.depth);
            }
        }

        visible
    }

    /// Call after mutations, before rendering. Rebuilds caches if invalidated.
    pub fn refresh_caches(&mut self) {
        if self.visible_cache.is_none() {
            self.visible_cache = Some(self.compute_visible());
        }
        if self.recent_cache.is_none() {
            self.recent_cache = Some(self.compute_recent(10));
        }
    }

    pub fn visible_entries(&self) -> Vec<&FileEntry> {
        match &self.visible_cache {
            Some(indices) => indices.iter().map(|&i| &self.entries[i]).collect(),
            None => {
                // Fallback: compute on the fly (shouldn't happen if refresh_caches called)
                let indices = self.compute_visible();
                indices.iter().map(|&i| &self.entries[i]).collect()
            }
        }
    }

    pub fn visible_count(&self) -> usize {
        match &self.visible_cache {
            Some(indices) => indices.len(),
            None => self.compute_visible().len(),
        }
    }

    pub fn visible_entry_at(&self, idx: usize) -> Option<&FileEntry> {
        let indices = self.visible_cache.as_ref()?;
        indices.get(idx).map(|&i| &self.entries[i])
    }

    fn compute_recent(&self, limit: usize) -> Vec<usize> {
        let mut modified: Vec<usize> = self
            .entries
            .iter()
            .enumerate()
            .filter(|(_, e)| !e.is_dir && e.last_modified.is_some())
            .map(|(i, _)| i)
            .collect();
        modified.sort_by(|&a, &b| {
            self.entries[b]
                .last_modified
                .unwrap()
                .cmp(&self.entries[a].last_modified.unwrap())
        });
        modified.truncate(limit);
        modified
    }

    pub fn recent_activity(&self, limit: usize) -> Vec<&FileEntry> {
        match &self.recent_cache {
            Some(indices) => indices
                .iter()
                .take(limit)
                .map(|&i| &self.entries[i])
                .collect(),
            None => {
                let indices = self.compute_recent(limit);
                indices.iter().map(|&i| &self.entries[i]).collect()
            }
        }
    }

    pub fn recent_count(&self, limit: usize) -> usize {
        match &self.recent_cache {
            Some(indices) => indices.len().min(limit),
            None => self.compute_recent(limit).len(),
        }
    }

    pub fn recent_entry_at(&self, idx: usize) -> Option<&FileEntry> {
        let indices = self.recent_cache.as_ref()?;
        indices.get(idx).map(|&i| &self.entries[i])
    }

    pub fn toggle_dir(&mut self, idx: usize) {
        let visible = self
            .visible_cache
            .take()
            .unwrap_or_else(|| self.compute_visible());
        if let Some(&actual_idx) = visible.get(idx) {
            if self.entries[actual_idx].is_dir {
                self.entries[actual_idx].expanded = !self.entries[actual_idx].expanded;
            }
        }
        self.invalidate_caches();
    }

    pub fn record_activity(&mut self, path: &Path, agent: Option<AgentId>) {
        if let Some(&idx) = self.index.get(path) {
            self.entries[idx].touch(agent);
        } else {
            self.insert_entry(path, agent);
        }

        // Mark parent dirs via path ancestry (not linear scan)
        let mut parent = path.parent();
        while let Some(p) = parent {
            if p == self.root {
                break;
            }
            if let Some(&idx) = self.index.get(p) {
                self.entries[idx].touch(agent);
            }
            parent = p.parent();
        }

        self.invalidate_caches();
    }

    fn insert_entry(&mut self, path: &Path, agent: Option<AgentId>) {
        let is_dir = path.is_dir();
        let depth = path
            .strip_prefix(&self.root)
            .map(|p| p.components().count().saturating_sub(1))
            .unwrap_or(0);

        let mut entry = if is_dir {
            FileEntry::dir(path.to_path_buf(), depth)
        } else {
            FileEntry::file(path.to_path_buf(), depth)
        };
        entry.touch(agent);

        // Ensure parent dirs exist
        if let Some(parent) = path.parent() {
            if parent != self.root && !self.index.contains_key(parent) {
                self.insert_entry(parent, agent);
            }
        }

        // Find insertion point
        let parent_path = path.parent().unwrap_or(Path::new(""));
        let insert_at = if parent_path == self.root {
            // Root-level: find sorted position among depth-0 entries
            self.find_sorted_position(0, is_dir, path, 0)
        } else if let Some(&parent_idx) = self.index.get(parent_path) {
            // After parent, find sorted position among siblings
            self.find_sorted_position(parent_idx + 1, is_dir, path, depth)
        } else {
            self.entries.len()
        };

        self.entries.insert(insert_at, entry);
        // Rebuild index after insert (indices shifted)
        self.rebuild_index();
    }

    fn find_sorted_position(&self, start: usize, is_dir: bool, path: &Path, depth: usize) -> usize {
        let mut i = start;
        while i < self.entries.len() {
            let e = &self.entries[i];
            if e.depth < depth {
                break; // Left the parent's children
            }
            if e.depth == depth {
                match (is_dir, e.is_dir) {
                    (true, false) => return i, // Dirs before files
                    (false, true) => {}        // Skip dirs
                    _ => {
                        if path < e.path.as_path() {
                            return i;
                        }
                    }
                }
            }
            i += 1;
        }
        i
    }

    pub fn update_diff(&mut self, path: &Path, added: usize, removed: usize) {
        if let Some(&idx) = self.index.get(path) {
            self.entries[idx].set_diff(added, removed);
        }
    }

    pub fn remove_entry(&mut self, path: &Path) {
        self.entries
            .retain(|e| e.path != path && !e.path.starts_with(path.join("")));
        self.rebuild_index();
        self.invalidate_caches();
    }

    pub fn count(&self) -> usize {
        self.entries.len()
    }
}

impl Default for FileTree {
    fn default() -> Self {
        Self::new(PathBuf::new())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_tree() -> FileTree {
        let mut tree = FileTree::new(PathBuf::from("/project"));
        tree.set_entries(vec![
            FileEntry::dir(PathBuf::from("/project/src"), 0),
            FileEntry::file(PathBuf::from("/project/src/main.rs"), 1),
            FileEntry::file(PathBuf::from("/project/src/lib.rs"), 1),
            FileEntry::dir(PathBuf::from("/project/tests"), 0),
            FileEntry::file(PathBuf::from("/project/tests/test.rs"), 1),
            FileEntry::file(PathBuf::from("/project/Cargo.toml"), 0),
        ]);
        tree
    }

    #[test]
    fn visible_entries_shows_all_when_expanded() {
        let tree = sample_tree();
        let visible = tree.visible_entries();
        assert_eq!(visible.len(), 6);
    }

    #[test]
    fn collapsing_dir_hides_children() {
        let mut tree = sample_tree();
        tree.toggle_dir(0);
        let visible = tree.visible_entries();
        assert_eq!(visible.len(), 4);
        assert_eq!(visible[0].display_name(), "src");
        assert_eq!(visible[1].display_name(), "tests");
    }

    #[test]
    fn toggle_dir_expands_again() {
        let mut tree = sample_tree();
        tree.toggle_dir(0);
        tree.toggle_dir(0);
        let visible = tree.visible_entries();
        assert_eq!(visible.len(), 6);
    }

    #[test]
    fn record_activity_updates_file_and_parents() {
        let mut tree = sample_tree();
        tree.record_activity(
            &PathBuf::from("/project/src/main.rs"),
            Some(AgentId::from_u128(1)),
        );
        let entries = tree.entries();
        let main = entries
            .iter()
            .find(|e| e.display_name() == "main.rs")
            .unwrap();
        assert_eq!(main.modify_count, 1);
        assert_eq!(main.modified_by, Some(AgentId::from_u128(1)));
        let src = entries.iter().find(|e| e.display_name() == "src").unwrap();
        assert_eq!(src.modify_count, 1);
    }

    #[test]
    fn record_activity_inserts_new_file() {
        let mut tree = sample_tree();
        tree.record_activity(&PathBuf::from("/project/src/new_file.rs"), None);
        assert!(tree
            .entries()
            .iter()
            .any(|e| e.display_name() == "new_file.rs"));
        assert_eq!(tree.count(), 7);
    }

    #[test]
    fn remove_entry_works() {
        let mut tree = sample_tree();
        tree.remove_entry(&PathBuf::from("/project/src/lib.rs"));
        assert_eq!(tree.count(), 5);
    }

    #[test]
    fn display_name_returns_filename() {
        let entry = FileEntry::file(PathBuf::from("/project/src/main.rs"), 1);
        assert_eq!(entry.display_name(), "main.rs");
    }

    #[test]
    fn age_secs_none_when_not_modified() {
        let entry = FileEntry::file(PathBuf::from("test.rs"), 0);
        assert!(entry.age_secs().is_none());
    }

    #[test]
    fn age_secs_some_after_touch() {
        let mut entry = FileEntry::file(PathBuf::from("test.rs"), 0);
        entry.touch(None);
        assert!(entry.age_secs().is_some());
    }

    #[test]
    fn recent_activity_returns_modified_files_sorted() {
        let mut tree = sample_tree();
        tree.record_activity(
            &PathBuf::from("/project/src/lib.rs"),
            Some(AgentId::from_u128(1)),
        );
        tree.record_activity(
            &PathBuf::from("/project/src/main.rs"),
            Some(AgentId::from_u128(2)),
        );
        let recent = tree.recent_activity(10);
        assert_eq!(recent.len(), 2);
        assert_eq!(recent[0].display_name(), "main.rs");
        assert_eq!(recent[1].display_name(), "lib.rs");
    }

    #[test]
    fn recent_activity_excludes_dirs() {
        let mut tree = sample_tree();
        tree.record_activity(&PathBuf::from("/project/src/main.rs"), None);
        let recent = tree.recent_activity(10);
        assert!(recent.iter().all(|e| !e.is_dir));
    }

    #[test]
    fn recent_activity_respects_limit() {
        let mut tree = sample_tree();
        tree.record_activity(&PathBuf::from("/project/src/lib.rs"), None);
        tree.record_activity(&PathBuf::from("/project/src/main.rs"), None);
        tree.record_activity(&PathBuf::from("/project/Cargo.toml"), None);
        let recent = tree.recent_activity(2);
        assert_eq!(recent.len(), 2);
    }

    #[test]
    fn index_lookup_is_correct() {
        let tree = sample_tree();
        assert!(tree.index.contains_key(&PathBuf::from("/project/src")));
        assert!(tree
            .index
            .contains_key(&PathBuf::from("/project/src/main.rs")));
        assert!(!tree
            .index
            .contains_key(&PathBuf::from("/project/nonexistent")));
    }

    #[test]
    fn caches_invalidate_on_mutation() {
        let mut tree = sample_tree();
        // Prime caches
        tree.refresh_caches();
        assert!(tree.visible_cache.is_some());
        assert!(tree.recent_cache.is_some());
        // Mutate
        tree.record_activity(&PathBuf::from("/project/src/main.rs"), None);
        assert!(tree.visible_cache.is_none());
        assert!(tree.recent_cache.is_none());
    }
}
