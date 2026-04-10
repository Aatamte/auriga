use orchestrator_types::{Agent, AgentId};

#[derive(Debug)]
pub struct AgentStore {
    agents: Vec<Agent>,
}

impl AgentStore {
    pub fn new() -> Self {
        Self { agents: Vec::new() }
    }

    pub fn create(&mut self, provider: &str) -> AgentId {
        let id = AgentId::new();
        let name = format!("{} #{}", provider, &id.0.simple().to_string()[..8]);
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
    use orchestrator_types::AgentStatus;

    #[test]
    fn create_assigns_unique_ids() {
        let mut store = AgentStore::new();
        let a = store.create("claude");
        let b = store.create("claude");
        assert_ne!(a, b);
    }

    #[test]
    fn create_sets_name_from_provider_and_hex() {
        let mut store = AgentStore::new();
        let id = store.create("codex");
        let agent = store.get(id).unwrap();
        assert!(agent.name.starts_with("codex #"));
        assert_eq!(agent.name.len(), "codex #".len() + 8);
        assert_eq!(agent.provider, "codex");
    }

    #[test]
    fn create_assigns_unique_names() {
        let mut store = AgentStore::new();
        let a = store.create("claude");
        let b = store.create("claude");
        assert_ne!(store.get(a).unwrap().name, store.get(b).unwrap().name);
    }

    #[test]
    fn new_agent_is_idle() {
        let mut store = AgentStore::new();
        let id = store.create("claude");
        assert_eq!(store.get(id).unwrap().status, AgentStatus::Idle);
    }

    #[test]
    fn new_agent_has_no_session() {
        let mut store = AgentStore::new();
        let id = store.create("claude");
        assert!(store.get(id).unwrap().session_id.is_none());
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
        assert!(!store.remove(AgentId::new()));
    }

    #[test]
    fn get_nonexistent_returns_none() {
        let store = AgentStore::new();
        assert!(store.get(AgentId::new()).is_none());
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
        let ids = store.ids();
        assert!(ids.contains(&a));
        assert!(ids.contains(&b));
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
