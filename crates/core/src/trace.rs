use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{AgentId, TokenUsage};

// ---------------------------------------------------------------------------
// IDs
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TraceId(pub Uuid);

impl TraceId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    pub fn from_u128(val: u128) -> Self {
        Self(Uuid::from_u128(val))
    }
}

impl Default for TraceId {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Enums
// ---------------------------------------------------------------------------

/// Trace lifecycle status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TraceStatus {
    Active,
    Complete,
    Aborted,
}

// ---------------------------------------------------------------------------
// Trace entity
// ---------------------------------------------------------------------------

/// A session-level grouping of turns for one agent conversation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Trace {
    pub id: TraceId,
    pub agent_id: AgentId,
    pub session_id: String,
    pub status: TraceStatus,
    pub started_at: String,
    pub completed_at: Option<String>,
    pub turn_count: u32,
    pub token_usage: TokenUsage,
    pub provider: String,
    pub model: Option<String>,
}

// ---------------------------------------------------------------------------
// TraceStore
// ---------------------------------------------------------------------------

#[derive(Debug)]
pub struct TraceStore {
    traces: Vec<Trace>,
}

impl TraceStore {
    pub fn new() -> Self {
        Self { traces: Vec::new() }
    }

    /// Create a new active trace. Returns its ID.
    pub fn create(
        &mut self,
        agent_id: AgentId,
        session_id: String,
        provider: String,
        started_at: String,
    ) -> TraceId {
        let id = TraceId::new();
        let trace = Trace {
            id,
            agent_id,
            session_id,
            status: TraceStatus::Active,
            started_at,
            completed_at: None,
            turn_count: 0,
            token_usage: TokenUsage {
                input_tokens: 0,
                output_tokens: 0,
                cache_creation_input_tokens: None,
                cache_read_input_tokens: None,
            },
            provider,
            model: None,
        };
        self.traces.push(trace);
        id
    }

    pub fn get(&self, id: TraceId) -> Option<&Trace> {
        self.traces.iter().find(|t| t.id == id)
    }

    pub fn get_mut(&mut self, id: TraceId) -> Option<&mut Trace> {
        self.traces.iter_mut().find(|t| t.id == id)
    }

    /// The currently active trace for an agent, if any.
    pub fn active_trace(&self, agent_id: AgentId) -> Option<&Trace> {
        self.traces
            .iter()
            .rfind(|t| t.agent_id == agent_id && t.status == TraceStatus::Active)
    }

    /// Mutable access to the active trace for an agent.
    pub fn active_trace_mut(&mut self, agent_id: AgentId) -> Option<&mut Trace> {
        self.traces
            .iter_mut()
            .rfind(|t| t.agent_id == agent_id && t.status == TraceStatus::Active)
    }

    /// Find a trace by agent and session ID.
    pub fn find_by_session(&self, agent_id: AgentId, session_id: &str) -> Option<&Trace> {
        self.traces
            .iter()
            .find(|t| t.agent_id == agent_id && t.session_id == session_id)
    }

    /// Transition an active trace to complete. Returns false if not found or not active.
    pub fn complete(&mut self, id: TraceId, completed_at: String) -> bool {
        if let Some(trace) = self.traces.iter_mut().find(|t| t.id == id) {
            if trace.status == TraceStatus::Active {
                trace.status = TraceStatus::Complete;
                trace.completed_at = Some(completed_at);
                return true;
            }
        }
        false
    }

    /// Transition an active trace to aborted. Returns false if not found or not active.
    pub fn abort(&mut self, id: TraceId, completed_at: String) -> bool {
        if let Some(trace) = self.traces.iter_mut().find(|t| t.id == id) {
            if trace.status == TraceStatus::Active {
                trace.status = TraceStatus::Aborted;
                trace.completed_at = Some(completed_at);
                return true;
            }
        }
        false
    }

    /// All traces for a given agent, in insertion order.
    pub fn traces_for(&self, agent_id: AgentId) -> Vec<&Trace> {
        self.traces
            .iter()
            .filter(|t| t.agent_id == agent_id)
            .collect()
    }

    /// Remove all traces for an agent. Prevents orphaned state.
    pub fn remove_agent_traces(&mut self, agent_id: AgentId) {
        self.traces.retain(|t| t.agent_id != agent_id);
    }

    /// Drain completed/aborted traces out of the store for persistence.
    pub fn take_finished(&mut self) -> Vec<Trace> {
        let mut finished = Vec::new();
        let mut i = 0;
        while i < self.traces.len() {
            if self.traces[i].status != TraceStatus::Active {
                finished.push(self.traces.remove(i));
            } else {
                i += 1;
            }
        }
        finished
    }

    pub fn count(&self) -> usize {
        self.traces.len()
    }
}

impl Default for TraceStore {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn agent(n: u128) -> AgentId {
        AgentId::from_u128(n)
    }

    // -- Creation --

    #[test]
    fn create_returns_unique_ids() {
        let mut store = TraceStore::new();
        let a = store.create(
            agent(1),
            "s1".into(),
            "claude".into(),
            "2026-01-01T00:00:00Z".into(),
        );
        let b = store.create(
            agent(1),
            "s2".into(),
            "claude".into(),
            "2026-01-01T00:01:00Z".into(),
        );
        assert_ne!(a, b);
    }

    #[test]
    fn create_sets_active_status() {
        let mut store = TraceStore::new();
        let id = store.create(
            agent(1),
            "s1".into(),
            "claude".into(),
            "2026-01-01T00:00:00Z".into(),
        );
        assert_eq!(store.get(id).unwrap().status, TraceStatus::Active);
    }

    #[test]
    fn create_initializes_zero_counters() {
        let mut store = TraceStore::new();
        let id = store.create(
            agent(1),
            "s1".into(),
            "claude".into(),
            "2026-01-01T00:00:00Z".into(),
        );
        let trace = store.get(id).unwrap();
        assert_eq!(trace.turn_count, 0);
        assert_eq!(trace.token_usage.input_tokens, 0);
        assert_eq!(trace.token_usage.output_tokens, 0);
        assert!(trace.completed_at.is_none());
        assert!(trace.model.is_none());
    }

    #[test]
    fn create_preserves_fields() {
        let mut store = TraceStore::new();
        let id = store.create(
            agent(42),
            "sess-abc".into(),
            "codex".into(),
            "2026-03-15T12:00:00Z".into(),
        );
        let trace = store.get(id).unwrap();
        assert_eq!(trace.agent_id, agent(42));
        assert_eq!(trace.session_id, "sess-abc");
        assert_eq!(trace.provider, "codex");
        assert_eq!(trace.started_at, "2026-03-15T12:00:00Z");
    }

    // -- Get / Get mut --

    #[test]
    fn get_nonexistent_returns_none() {
        let store = TraceStore::new();
        assert!(store.get(TraceId::from_u128(999)).is_none());
    }

    #[test]
    fn get_mut_allows_modification() {
        let mut store = TraceStore::new();
        let id = store.create(
            agent(1),
            "s1".into(),
            "claude".into(),
            "2026-01-01T00:00:00Z".into(),
        );
        store.get_mut(id).unwrap().turn_count = 5;
        assert_eq!(store.get(id).unwrap().turn_count, 5);
    }

    // -- Active trace --

    #[test]
    fn active_trace_returns_active() {
        let mut store = TraceStore::new();
        let id = store.create(
            agent(1),
            "s1".into(),
            "claude".into(),
            "2026-01-01T00:00:00Z".into(),
        );
        let active = store.active_trace(agent(1)).unwrap();
        assert_eq!(active.id, id);
    }

    #[test]
    fn active_trace_returns_none_when_all_complete() {
        let mut store = TraceStore::new();
        let id = store.create(
            agent(1),
            "s1".into(),
            "claude".into(),
            "2026-01-01T00:00:00Z".into(),
        );
        store.complete(id, "2026-01-01T00:05:00Z".into());
        assert!(store.active_trace(agent(1)).is_none());
    }

    #[test]
    fn active_trace_mut_allows_modification() {
        let mut store = TraceStore::new();
        store.create(
            agent(1),
            "s1".into(),
            "claude".into(),
            "2026-01-01T00:00:00Z".into(),
        );
        store.active_trace_mut(agent(1)).unwrap().turn_count += 1;
        assert_eq!(store.active_trace(agent(1)).unwrap().turn_count, 1);
    }

    // -- Find by session --

    #[test]
    fn find_by_session_returns_match() {
        let mut store = TraceStore::new();
        store.create(
            agent(1),
            "sess-x".into(),
            "claude".into(),
            "2026-01-01T00:00:00Z".into(),
        );
        assert!(store.find_by_session(agent(1), "sess-x").is_some());
    }

    #[test]
    fn find_by_session_returns_none_for_wrong_agent() {
        let mut store = TraceStore::new();
        store.create(
            agent(1),
            "sess-x".into(),
            "claude".into(),
            "2026-01-01T00:00:00Z".into(),
        );
        assert!(store.find_by_session(agent(2), "sess-x").is_none());
    }

    #[test]
    fn find_by_session_returns_none_for_unknown() {
        let store = TraceStore::new();
        assert!(store.find_by_session(agent(1), "nope").is_none());
    }

    // -- Complete / Abort --

    #[test]
    fn complete_transitions_status() {
        let mut store = TraceStore::new();
        let id = store.create(
            agent(1),
            "s1".into(),
            "claude".into(),
            "2026-01-01T00:00:00Z".into(),
        );
        assert!(store.complete(id, "2026-01-01T00:05:00Z".into()));
        let trace = store.get(id).unwrap();
        assert_eq!(trace.status, TraceStatus::Complete);
        assert_eq!(trace.completed_at.as_deref(), Some("2026-01-01T00:05:00Z"));
    }

    #[test]
    fn complete_already_complete_returns_false() {
        let mut store = TraceStore::new();
        let id = store.create(
            agent(1),
            "s1".into(),
            "claude".into(),
            "2026-01-01T00:00:00Z".into(),
        );
        store.complete(id, "2026-01-01T00:05:00Z".into());
        assert!(!store.complete(id, "2026-01-01T00:06:00Z".into()));
    }

    #[test]
    fn complete_nonexistent_returns_false() {
        let mut store = TraceStore::new();
        assert!(!store.complete(TraceId::from_u128(999), "2026-01-01T00:05:00Z".into()));
    }

    #[test]
    fn abort_transitions_status() {
        let mut store = TraceStore::new();
        let id = store.create(
            agent(1),
            "s1".into(),
            "claude".into(),
            "2026-01-01T00:00:00Z".into(),
        );
        assert!(store.abort(id, "2026-01-01T00:05:00Z".into()));
        let trace = store.get(id).unwrap();
        assert_eq!(trace.status, TraceStatus::Aborted);
        assert_eq!(trace.completed_at.as_deref(), Some("2026-01-01T00:05:00Z"));
    }

    #[test]
    fn abort_already_aborted_returns_false() {
        let mut store = TraceStore::new();
        let id = store.create(
            agent(1),
            "s1".into(),
            "claude".into(),
            "2026-01-01T00:00:00Z".into(),
        );
        store.abort(id, "2026-01-01T00:05:00Z".into());
        assert!(!store.abort(id, "2026-01-01T00:06:00Z".into()));
    }

    // -- Traces for agent --

    #[test]
    fn traces_for_returns_only_matching_agent() {
        let mut store = TraceStore::new();
        store.create(
            agent(1),
            "s1".into(),
            "claude".into(),
            "2026-01-01T00:00:00Z".into(),
        );
        store.create(
            agent(2),
            "s2".into(),
            "claude".into(),
            "2026-01-01T00:01:00Z".into(),
        );
        store.create(
            agent(1),
            "s3".into(),
            "claude".into(),
            "2026-01-01T00:02:00Z".into(),
        );
        assert_eq!(store.traces_for(agent(1)).len(), 2);
        assert_eq!(store.traces_for(agent(2)).len(), 1);
    }

    // -- Remove agent traces --

    #[test]
    fn remove_agent_traces_cleans_up() {
        let mut store = TraceStore::new();
        store.create(
            agent(1),
            "s1".into(),
            "claude".into(),
            "2026-01-01T00:00:00Z".into(),
        );
        store.create(
            agent(2),
            "s2".into(),
            "claude".into(),
            "2026-01-01T00:01:00Z".into(),
        );
        store.remove_agent_traces(agent(1));
        assert_eq!(store.traces_for(agent(1)).len(), 0);
        assert_eq!(store.traces_for(agent(2)).len(), 1);
    }

    #[test]
    fn remove_agent_traces_for_unknown_is_noop() {
        let mut store = TraceStore::new();
        store.create(
            agent(1),
            "s1".into(),
            "claude".into(),
            "2026-01-01T00:00:00Z".into(),
        );
        store.remove_agent_traces(agent(99));
        assert_eq!(store.count(), 1);
    }

    // -- Take finished --

    #[test]
    fn take_finished_drains_completed_and_aborted() {
        let mut store = TraceStore::new();
        let a = store.create(
            agent(1),
            "s1".into(),
            "claude".into(),
            "2026-01-01T00:00:00Z".into(),
        );
        let b = store.create(
            agent(1),
            "s2".into(),
            "claude".into(),
            "2026-01-01T00:01:00Z".into(),
        );
        store.create(
            agent(1),
            "s3".into(),
            "claude".into(),
            "2026-01-01T00:02:00Z".into(),
        );

        store.complete(a, "2026-01-01T00:05:00Z".into());
        store.abort(b, "2026-01-01T00:05:00Z".into());

        let finished = store.take_finished();
        assert_eq!(finished.len(), 2);
        assert_eq!(store.count(), 1); // only active one remains
    }

    #[test]
    fn take_finished_returns_empty_when_all_active() {
        let mut store = TraceStore::new();
        store.create(
            agent(1),
            "s1".into(),
            "claude".into(),
            "2026-01-01T00:00:00Z".into(),
        );
        let finished = store.take_finished();
        assert!(finished.is_empty());
        assert_eq!(store.count(), 1);
    }

    // -- Count --

    #[test]
    fn count_returns_total() {
        let mut store = TraceStore::new();
        store.create(
            agent(1),
            "s1".into(),
            "claude".into(),
            "2026-01-01T00:00:00Z".into(),
        );
        store.create(
            agent(2),
            "s2".into(),
            "claude".into(),
            "2026-01-01T00:01:00Z".into(),
        );
        assert_eq!(store.count(), 2);
    }

    // -- Serialization round-trip --

    #[test]
    fn trace_serializes_and_deserializes() {
        let mut store = TraceStore::new();
        let id = store.create(
            agent(1),
            "s1".into(),
            "claude".into(),
            "2026-01-01T00:00:00Z".into(),
        );
        let trace = store.get(id).unwrap();
        let json = serde_json::to_string(trace).unwrap();
        let parsed: Trace = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.id, trace.id);
        assert_eq!(parsed.session_id, trace.session_id);
        assert_eq!(parsed.status, trace.status);
    }
}
