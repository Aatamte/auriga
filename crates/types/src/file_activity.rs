use crate::AgentId;
use std::path::PathBuf;
use std::time::Instant;

#[derive(Debug)]
pub struct FileActivity {
    pub path: PathBuf,
    pub last_modified: Instant,
    pub modify_count: usize,
    pub modified_by: Option<AgentId>,
}

impl FileActivity {
    pub fn new(path: PathBuf, agent: Option<AgentId>) -> Self {
        Self {
            path,
            last_modified: Instant::now(),
            modify_count: 1,
            modified_by: agent,
        }
    }

    pub fn touch(&mut self, agent: Option<AgentId>) {
        self.last_modified = Instant::now();
        self.modify_count += 1;
        if agent.is_some() {
            self.modified_by = agent;
        }
    }

    pub fn age_secs(&self) -> f64 {
        self.last_modified.elapsed().as_secs_f64()
    }
}
