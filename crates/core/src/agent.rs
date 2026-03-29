use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct AgentId(pub usize);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AgentStatus {
    Idle,
    Working,
}

#[derive(Debug)]
pub struct Agent {
    pub id: AgentId,
    pub name: String,
    pub provider: String,
    pub status: AgentStatus,
}

impl Agent {
    pub fn new(id: AgentId, name: String, provider: String) -> Self {
        Self {
            id,
            name,
            provider,
            status: AgentStatus::Idle,
        }
    }
}

#[derive(Debug)]
pub struct AgentStore {
    agents: Vec<Agent>,
    next_id: usize,
}

impl AgentStore {
    pub fn new() -> Self {
        Self {
            agents: Vec::new(),
            next_id: 1,
        }
    }

    pub fn create(&mut self, provider: &str) -> AgentId {
        let id = AgentId(self.next_id);
        let name = format!("{} #{}", provider, self.next_id);
        self.next_id += 1;
        let agent = Agent::new(id, name, provider.to_string());
        self.agents.push(agent);
        id
    }

    pub fn remove(&mut self, id: AgentId) -> bool {
        let len = self.agents.len();
        self.agents.retain(|a| a.id != id);
        self.agents.len() < len
    }

    pub fn get(&self, id: AgentId) -> Option<&Agent> {
        self.agents.iter().find(|a| a.id == id)
    }

    pub fn get_mut(&mut self, id: AgentId) -> Option<&mut Agent> {
        self.agents.iter_mut().find(|a| a.id == id)
    }

    pub fn list(&self) -> &[Agent] {
        &self.agents
    }

    pub fn count(&self) -> usize {
        self.agents.len()
    }

    pub fn ids(&self) -> Vec<AgentId> {
        self.agents.iter().map(|a| a.id).collect()
    }
}

impl Default for AgentStore {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_assigns_incrementing_ids() {
        let mut store = AgentStore::new();
        let a = store.create("claude");
        let b = store.create("claude");
        assert_eq!(a, AgentId(1));
        assert_eq!(b, AgentId(2));
    }

    #[test]
    fn create_sets_name_from_provider() {
        let mut store = AgentStore::new();
        let id = store.create("codex");
        let agent = store.get(id).unwrap();
        assert_eq!(agent.name, "codex #1");
        assert_eq!(agent.provider, "codex");
    }

    #[test]
    fn new_agent_is_idle() {
        let mut store = AgentStore::new();
        let id = store.create("claude");
        assert_eq!(store.get(id).unwrap().status, AgentStatus::Idle);
    }

    #[test]
    fn remove_existing_returns_true() {
        let mut store = AgentStore::new();
        let id = store.create("claude");
        assert!(store.remove(id));
        assert_eq!(store.count(), 0);
    }

    #[test]
    fn remove_nonexistent_returns_false() {
        let mut store = AgentStore::new();
        assert!(!store.remove(AgentId(999)));
    }

    #[test]
    fn get_nonexistent_returns_none() {
        let store = AgentStore::new();
        assert!(store.get(AgentId(1)).is_none());
    }

    #[test]
    fn list_returns_all_agents() {
        let mut store = AgentStore::new();
        store.create("claude");
        store.create("codex");
        assert_eq!(store.list().len(), 2);
    }

    #[test]
    fn ids_returns_all_ids() {
        let mut store = AgentStore::new();
        let a = store.create("claude");
        let b = store.create("codex");
        assert_eq!(store.ids(), vec![a, b]);
    }

    #[test]
    fn remove_does_not_affect_other_agents() {
        let mut store = AgentStore::new();
        let a = store.create("claude");
        let b = store.create("codex");
        store.remove(a);
        assert!(store.get(a).is_none());
        assert!(store.get(b).is_some());
    }
}
