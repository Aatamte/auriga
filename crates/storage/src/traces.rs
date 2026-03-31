use orchestrator_core::{
    AgentId, MessageContent, MessageType, Trace, TraceId, TraceStatus, TokenUsage, Turn, TurnId,
    TurnMeta, TurnRole, TurnStatus,
};
use rusqlite::params;
use uuid::Uuid;

use crate::db::Database;

// ---------------------------------------------------------------------------
// Trace operations on Database
// ---------------------------------------------------------------------------

impl Database {
    pub fn save_trace(&self, trace: &Trace, turns: &[&Turn]) -> anyhow::Result<()> {
        let tx = self.conn.unchecked_transaction()?;

        tx.execute(
            "INSERT OR REPLACE INTO traces (id, agent_id, session_id, status, started_at, completed_at, turn_count, input_tokens, output_tokens, cache_creation_input_tokens, cache_read_input_tokens, provider, model) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
            params![
                trace.id.0.to_string(),
                trace.agent_id.0.to_string(),
                trace.session_id,
                status_to_str(trace.status),
                trace.started_at,
                trace.completed_at,
                trace.turn_count,
                trace.token_usage.input_tokens,
                trace.token_usage.output_tokens,
                trace.token_usage.cache_creation_input_tokens,
                trace.token_usage.cache_read_input_tokens,
                trace.provider,
                trace.model,
            ],
        )?;

        for turn in turns {
            let content_json = serde_json::to_string(&turn.content)?;
            let meta_json = serde_json::to_string(&turn.meta)?;
            let extra_json = serde_json::to_string(&turn.extra)?;

            tx.execute(
                "INSERT OR REPLACE INTO turns (id, trace_id, agent_id, number, status, uuid, parent_uuid, session_id, timestamp, message_type, cwd, git_branch, role, content, meta, extra) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16)",
                params![
                    turn.id.0,
                    trace.id.0.to_string(),
                    turn.agent_id.0.to_string(),
                    turn.number,
                    turn_status_to_str(turn.status),
                    turn.uuid,
                    turn.parent_uuid,
                    turn.session_id,
                    turn.timestamp,
                    message_type_to_str(turn.message_type),
                    turn.cwd,
                    turn.git_branch,
                    turn_role_to_str(turn.role),
                    content_json,
                    meta_json,
                    extra_json,
                ],
            )?;
        }

        tx.commit()?;
        Ok(())
    }

    pub fn load_trace(&self, id: TraceId) -> anyhow::Result<Option<Trace>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, agent_id, session_id, status, started_at, completed_at, turn_count, input_tokens, output_tokens, cache_creation_input_tokens, cache_read_input_tokens, provider, model FROM traces WHERE id = ?1",
        )?;

        let result = stmt.query_row(params![id.0.to_string()], |row| Ok(row_to_trace(row)));

        match result {
            Ok(trace) => Ok(Some(trace?)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    pub fn load_turns(&self, trace_id: TraceId) -> anyhow::Result<Vec<Turn>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, trace_id, agent_id, number, status, uuid, parent_uuid, session_id, timestamp, message_type, cwd, git_branch, role, content, meta, extra FROM turns WHERE trace_id = ?1 ORDER BY id",
        )?;

        let turns = stmt
            .query_map(params![trace_id.0.to_string()], |row| Ok(row_to_turn(row)))?
            .collect::<Result<Vec<_>, _>>()?
            .into_iter()
            .collect::<anyhow::Result<Vec<_>>>()?;

        Ok(turns)
    }

    pub fn list_traces(&self, limit: usize, offset: usize) -> anyhow::Result<Vec<Trace>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, agent_id, session_id, status, started_at, completed_at, turn_count, input_tokens, output_tokens, cache_creation_input_tokens, cache_read_input_tokens, provider, model FROM traces ORDER BY started_at DESC LIMIT ?1 OFFSET ?2",
        )?;

        let traces = stmt
            .query_map(params![limit, offset], |row| Ok(row_to_trace(row)))?
            .collect::<Result<Vec<_>, _>>()?
            .into_iter()
            .collect::<anyhow::Result<Vec<_>>>()?;

        Ok(traces)
    }

    pub fn list_agent_traces(&self, agent_id: AgentId) -> anyhow::Result<Vec<Trace>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, agent_id, session_id, status, started_at, completed_at, turn_count, input_tokens, output_tokens, cache_creation_input_tokens, cache_read_input_tokens, provider, model FROM traces WHERE agent_id = ?1 ORDER BY started_at DESC",
        )?;

        let traces = stmt
            .query_map(params![agent_id.0.to_string()], |row| {
                Ok(row_to_trace(row))
            })?
            .collect::<Result<Vec<_>, _>>()?
            .into_iter()
            .collect::<anyhow::Result<Vec<_>>>()?;

        Ok(traces)
    }
}

// ---------------------------------------------------------------------------
// Row mapping helpers
// ---------------------------------------------------------------------------

fn row_to_trace(row: &rusqlite::Row) -> anyhow::Result<Trace> {
    let id_str: String = row.get(0)?;
    let agent_id_str: String = row.get(1)?;

    Ok(Trace {
        id: TraceId(Uuid::parse_str(&id_str)?),
        agent_id: AgentId(Uuid::parse_str(&agent_id_str)?),
        session_id: row.get(2)?,
        status: str_to_status(row.get::<_, String>(3)?.as_str()),
        started_at: row.get(4)?,
        completed_at: row.get(5)?,
        turn_count: row.get::<_, u32>(6)?,
        token_usage: TokenUsage {
            input_tokens: row.get(7)?,
            output_tokens: row.get(8)?,
            cache_creation_input_tokens: row.get(9)?,
            cache_read_input_tokens: row.get(10)?,
        },
        provider: row.get(11)?,
        model: row.get(12)?,
    })
}

fn row_to_turn(row: &rusqlite::Row) -> anyhow::Result<Turn> {
    let id_val: usize = row.get(0)?;
    // column 1 is trace_id, skip
    let agent_id_str: String = row.get(2)?;
    let content_json: String = row.get(13)?;
    let meta_json: String = row.get(14)?;
    let extra_json: String = row.get(15)?;

    let agent_id = AgentId(Uuid::parse_str(&agent_id_str)?);
    let content: MessageContent = serde_json::from_str(&content_json)?;
    let meta: TurnMeta = serde_json::from_str(&meta_json)?;
    let extra: serde_json::Value = serde_json::from_str(&extra_json)?;

    Ok(Turn {
        id: TurnId(id_val),
        agent_id,
        number: row.get(3)?,
        status: str_to_turn_status(row.get::<_, String>(4)?.as_str()),
        uuid: row.get(5)?,
        parent_uuid: row.get(6)?,
        session_id: row.get(7)?,
        timestamp: row.get(8)?,
        message_type: str_to_message_type(row.get::<_, String>(9)?.as_str()),
        cwd: row.get(10)?,
        git_branch: row.get(11)?,
        role: str_to_turn_role(row.get::<_, String>(12)?.as_str()),
        content,
        meta,
        extra,
    })
}

// ---------------------------------------------------------------------------
// Enum <-> string conversions
// ---------------------------------------------------------------------------

fn status_to_str(s: TraceStatus) -> &'static str {
    match s {
        TraceStatus::Active => "Active",
        TraceStatus::Complete => "Complete",
        TraceStatus::Aborted => "Aborted",
    }
}

fn str_to_status(s: &str) -> TraceStatus {
    match s {
        "Complete" => TraceStatus::Complete,
        "Aborted" => TraceStatus::Aborted,
        _ => TraceStatus::Active,
    }
}

fn turn_status_to_str(s: TurnStatus) -> &'static str {
    match s {
        TurnStatus::Active => "Active",
        TurnStatus::Complete => "Complete",
    }
}

fn str_to_turn_status(s: &str) -> TurnStatus {
    match s {
        "Complete" => TurnStatus::Complete,
        _ => TurnStatus::Active,
    }
}

fn message_type_to_str(m: MessageType) -> &'static str {
    match m {
        MessageType::User => "User",
        MessageType::Assistant => "Assistant",
        MessageType::System => "System",
    }
}

fn str_to_message_type(s: &str) -> MessageType {
    match s {
        "Assistant" => MessageType::Assistant,
        "System" => MessageType::System,
        _ => MessageType::User,
    }
}

fn turn_role_to_str(r: TurnRole) -> &'static str {
    match r {
        TurnRole::User => "User",
        TurnRole::Assistant => "Assistant",
    }
}

fn str_to_turn_role(s: &str) -> TurnRole {
    match s {
        "Assistant" => TurnRole::Assistant,
        _ => TurnRole::User,
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use orchestrator_core::{
        AssistantMeta, MessageContent, StopReason, TurnBuilder, TurnMeta, TurnStore, UserMeta,
    };
    use serde_json::json;

    fn agent(n: u128) -> AgentId {
        AgentId::from_u128(n)
    }

    fn make_db() -> Database {
        Database::open_in_memory().unwrap()
    }

    fn sample_trace(agent_id: AgentId) -> Trace {
        Trace {
            id: TraceId::from_u128(100),
            agent_id,
            session_id: "sess-1".to_string(),
            status: TraceStatus::Complete,
            started_at: "2026-03-01T10:00:00Z".to_string(),
            completed_at: Some("2026-03-01T10:05:00Z".to_string()),
            turn_count: 2,
            token_usage: TokenUsage {
                input_tokens: 500,
                output_tokens: 200,
                cache_creation_input_tokens: Some(10),
                cache_read_input_tokens: None,
            },
            provider: "claude".to_string(),
            model: Some("claude-opus-4-6".to_string()),
        }
    }

    fn sample_turns(agent_id: AgentId) -> Vec<Turn> {
        let mut store = TurnStore::new();

        let user_builder = TurnBuilder {
            uuid: "uuid-u1".to_string(),
            parent_uuid: None,
            session_id: Some("sess-1".to_string()),
            timestamp: "2026-03-01T10:00:00Z".to_string(),
            message_type: MessageType::User,
            cwd: Some("/home/user".to_string()),
            git_branch: Some("main".to_string()),
            role: TurnRole::User,
            content: MessageContent::Text("hello".to_string()),
            meta: TurnMeta::User(UserMeta {
                is_meta: false,
                is_compact_summary: false,
                source_tool_assistant_uuid: None,
            }),
            status: TurnStatus::Complete,
            extra: json!({}),
        };

        let asst_builder = TurnBuilder {
            uuid: "uuid-a1".to_string(),
            parent_uuid: Some("uuid-u1".to_string()),
            session_id: Some("sess-1".to_string()),
            timestamp: "2026-03-01T10:00:01Z".to_string(),
            message_type: MessageType::Assistant,
            cwd: None,
            git_branch: None,
            role: TurnRole::Assistant,
            content: MessageContent::Text("hi there".to_string()),
            meta: TurnMeta::Assistant(AssistantMeta {
                model: Some("claude-opus-4-6".to_string()),
                stop_reason: Some(StopReason::EndTurn),
                stop_sequence: None,
                usage: Some(TokenUsage {
                    input_tokens: 500,
                    output_tokens: 200,
                    cache_creation_input_tokens: Some(10),
                    cache_read_input_tokens: None,
                }),
                request_id: Some("req-1".to_string()),
            }),
            status: TurnStatus::Complete,
            extra: json!({"permissionMode": "auto"}),
        };

        store.insert(agent_id, user_builder);
        store.insert(agent_id, asst_builder);

        store
            .turns_for(agent_id)
            .into_iter()
            .cloned()
            .collect()
    }

    #[test]
    fn save_and_load_trace_round_trips() {
        let db = make_db();
        let trace = sample_trace(agent(1));
        let turns = sample_turns(agent(1));
        let turn_refs: Vec<&Turn> = turns.iter().collect();

        db.save_trace(&trace, &turn_refs).unwrap();

        let loaded = db.load_trace(trace.id).unwrap().unwrap();
        assert_eq!(loaded.id, trace.id);
        assert_eq!(loaded.agent_id, trace.agent_id);
        assert_eq!(loaded.session_id, "sess-1");
        assert_eq!(loaded.status, TraceStatus::Complete);
        assert_eq!(loaded.turn_count, 2);
        assert_eq!(loaded.token_usage.input_tokens, 500);
        assert_eq!(loaded.token_usage.output_tokens, 200);
        assert_eq!(loaded.token_usage.cache_creation_input_tokens, Some(10));
        assert!(loaded.token_usage.cache_read_input_tokens.is_none());
        assert_eq!(loaded.provider, "claude");
        assert_eq!(loaded.model.as_deref(), Some("claude-opus-4-6"));
    }

    #[test]
    fn load_nonexistent_trace_returns_none() {
        let db = make_db();
        let result = db.load_trace(TraceId::from_u128(999)).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn save_and_load_turns_round_trips() {
        let db = make_db();
        let trace = sample_trace(agent(1));
        let turns = sample_turns(agent(1));
        let turn_refs: Vec<&Turn> = turns.iter().collect();

        db.save_trace(&trace, &turn_refs).unwrap();

        let loaded_turns = db.load_turns(trace.id).unwrap();
        assert_eq!(loaded_turns.len(), 2);

        assert_eq!(loaded_turns[0].uuid, "uuid-u1");
        assert_eq!(loaded_turns[0].role, TurnRole::User);
        assert_eq!(loaded_turns[0].cwd.as_deref(), Some("/home/user"));

        assert_eq!(loaded_turns[1].uuid, "uuid-a1");
        assert_eq!(loaded_turns[1].role, TurnRole::Assistant);
        assert_eq!(loaded_turns[1].parent_uuid.as_deref(), Some("uuid-u1"));

        if let TurnMeta::Assistant(ref meta) = loaded_turns[1].meta {
            assert_eq!(meta.model.as_deref(), Some("claude-opus-4-6"));
            assert_eq!(meta.stop_reason, Some(StopReason::EndTurn));
        } else {
            panic!("expected AssistantMeta");
        }

        assert_eq!(loaded_turns[1].extra["permissionMode"], "auto");
    }

    #[test]
    fn list_traces_returns_most_recent_first() {
        let db = make_db();

        let mut t1 = sample_trace(agent(1));
        t1.id = TraceId::from_u128(1);
        t1.started_at = "2026-03-01T10:00:00Z".to_string();

        let mut t2 = sample_trace(agent(1));
        t2.id = TraceId::from_u128(2);
        t2.session_id = "sess-2".to_string();
        t2.started_at = "2026-03-02T10:00:00Z".to_string();

        db.save_trace(&t1, &[]).unwrap();
        db.save_trace(&t2, &[]).unwrap();

        let traces = db.list_traces(10, 0).unwrap();
        assert_eq!(traces.len(), 2);
        assert_eq!(traces[0].id, t2.id);
        assert_eq!(traces[1].id, t1.id);
    }

    #[test]
    fn list_traces_respects_limit_and_offset() {
        let db = make_db();

        for i in 0..5 {
            let mut t = sample_trace(agent(1));
            t.id = TraceId::from_u128(i);
            t.session_id = format!("sess-{}", i);
            t.started_at = format!("2026-03-0{}T10:00:00Z", i + 1);
            db.save_trace(&t, &[]).unwrap();
        }

        let page = db.list_traces(2, 1).unwrap();
        assert_eq!(page.len(), 2);
    }

    #[test]
    fn list_agent_traces_filters_by_agent() {
        let db = make_db();

        let mut t1 = sample_trace(agent(1));
        t1.id = TraceId::from_u128(1);

        let mut t2 = sample_trace(agent(2));
        t2.id = TraceId::from_u128(2);
        t2.session_id = "sess-2".to_string();

        db.save_trace(&t1, &[]).unwrap();
        db.save_trace(&t2, &[]).unwrap();

        let agent1_traces = db.list_agent_traces(agent(1)).unwrap();
        assert_eq!(agent1_traces.len(), 1);
        assert_eq!(agent1_traces[0].agent_id, agent(1));
    }

    #[test]
    fn save_trace_overwrites_existing() {
        let db = make_db();

        let mut trace = sample_trace(agent(1));
        db.save_trace(&trace, &[]).unwrap();

        trace.turn_count = 10;
        trace.status = TraceStatus::Aborted;
        db.save_trace(&trace, &[]).unwrap();

        let loaded = db.load_trace(trace.id).unwrap().unwrap();
        assert_eq!(loaded.turn_count, 10);
        assert_eq!(loaded.status, TraceStatus::Aborted);
    }
}
