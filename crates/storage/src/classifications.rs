use orchestrator_classifier::{ClassificationId, ClassificationResult};
use orchestrator_core::TraceId;
use rusqlite::params;
use uuid::Uuid;

use crate::Database;

impl Database {
    pub fn save_classification(&self, result: &ClassificationResult) -> anyhow::Result<()> {
        let payload_json = serde_json::to_string(&result.payload)?;
        self.conn.execute(
            "INSERT OR REPLACE INTO classifications \
             (id, trace_id, classifier_name, timestamp, payload) \
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                result.id.0.to_string(),
                result.trace_id.0.to_string(),
                result.classifier_name,
                result.timestamp,
                payload_json,
            ],
        )?;
        Ok(())
    }

    pub fn list_recent_classifications(
        &self,
        limit: u64,
    ) -> anyhow::Result<Vec<ClassificationResult>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, trace_id, classifier_name, timestamp, payload \
             FROM classifications ORDER BY timestamp DESC LIMIT ?1",
        )?;

        let results = stmt
            .query_map(params![limit], |row| {
                let id_str: String = row.get(0)?;
                let trace_id_str: String = row.get(1)?;
                let classifier_name: String = row.get(2)?;
                let timestamp: String = row.get(3)?;
                let payload_str: String = row.get(4)?;

                Ok((
                    id_str,
                    trace_id_str,
                    classifier_name,
                    timestamp,
                    payload_str,
                ))
            })?
            .filter_map(|r| r.ok())
            .filter_map(|(id_str, trace_id_str, name, ts, payload_str)| {
                let id = Uuid::parse_str(&id_str).ok()?;
                let trace_id = Uuid::parse_str(&trace_id_str).ok()?;
                let payload: serde_json::Value = serde_json::from_str(&payload_str).ok()?;
                Some(ClassificationResult {
                    id: ClassificationId(id),
                    trace_id: TraceId(trace_id),
                    classifier_name: name,
                    timestamp: ts,
                    payload,
                    notification: None,
                })
            })
            .collect();

        Ok(results)
    }

    pub fn load_classifications(
        &self,
        trace_id: TraceId,
    ) -> anyhow::Result<Vec<ClassificationResult>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, trace_id, classifier_name, timestamp, payload \
             FROM classifications WHERE trace_id = ?1 ORDER BY timestamp",
        )?;

        let results = stmt
            .query_map(params![trace_id.0.to_string()], |row| {
                let id_str: String = row.get(0)?;
                let trace_id_str: String = row.get(1)?;
                let classifier_name: String = row.get(2)?;
                let timestamp: String = row.get(3)?;
                let payload_str: String = row.get(4)?;

                Ok((
                    id_str,
                    trace_id_str,
                    classifier_name,
                    timestamp,
                    payload_str,
                ))
            })?
            .filter_map(|r| r.ok())
            .filter_map(|(id_str, trace_id_str, name, ts, payload_str)| {
                let id = Uuid::parse_str(&id_str).ok()?;
                let trace_id = Uuid::parse_str(&trace_id_str).ok()?;
                let payload: serde_json::Value = serde_json::from_str(&payload_str).ok()?;
                Some(ClassificationResult {
                    id: ClassificationId(id),
                    trace_id: TraceId(trace_id),
                    classifier_name: name,
                    timestamp: ts,
                    payload,
                    notification: None,
                })
            })
            .collect();

        Ok(results)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn save_and_load_classification() {
        let db = Database::open_in_memory().unwrap();
        // Need a trace first (FK constraint)
        let trace = orchestrator_core::Trace {
            id: TraceId::from_u128(1),
            agent_id: orchestrator_core::AgentId::from_u128(1),
            session_id: "s1".into(),
            status: orchestrator_core::TraceStatus::Active,
            started_at: "2026-01-01T00:00:00Z".into(),
            completed_at: None,
            turn_count: 0,
            token_usage: orchestrator_core::TokenUsage {
                input_tokens: 0,
                output_tokens: 0,
                cache_creation_input_tokens: None,
                cache_read_input_tokens: None,
            },
            provider: "claude".into(),
            model: None,
        };
        db.save_trace(&trace, &[]).unwrap();

        let result = ClassificationResult {
            id: ClassificationId::from_u128(10),
            trace_id: TraceId::from_u128(1),
            classifier_name: "test-classifier".into(),
            timestamp: "2026-01-01T00:01:00Z".into(),
            payload: serde_json::json!({"label": "looping", "confidence": 0.85}),
            notification: None,
        };
        db.save_classification(&result).unwrap();

        let loaded = db.load_classifications(TraceId::from_u128(1)).unwrap();
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].id, ClassificationId::from_u128(10));
        assert_eq!(loaded[0].classifier_name, "test-classifier");
        assert_eq!(loaded[0].payload["label"], "looping");
        assert_eq!(loaded[0].payload["confidence"], 0.85);
    }

    #[test]
    fn load_classifications_empty() {
        let db = Database::open_in_memory().unwrap();
        let loaded = db.load_classifications(TraceId::from_u128(999)).unwrap();
        assert!(loaded.is_empty());
    }

    #[test]
    fn save_classification_upserts() {
        let db = Database::open_in_memory().unwrap();
        let trace = orchestrator_core::Trace {
            id: TraceId::from_u128(1),
            agent_id: orchestrator_core::AgentId::from_u128(1),
            session_id: "s1".into(),
            status: orchestrator_core::TraceStatus::Active,
            started_at: "2026-01-01T00:00:00Z".into(),
            completed_at: None,
            turn_count: 0,
            token_usage: orchestrator_core::TokenUsage {
                input_tokens: 0,
                output_tokens: 0,
                cache_creation_input_tokens: None,
                cache_read_input_tokens: None,
            },
            provider: "claude".into(),
            model: None,
        };
        db.save_trace(&trace, &[]).unwrap();

        let result = ClassificationResult {
            id: ClassificationId::from_u128(10),
            trace_id: TraceId::from_u128(1),
            classifier_name: "test".into(),
            timestamp: "2026-01-01T00:01:00Z".into(),
            payload: serde_json::json!({"v": 1}),
            notification: None,
        };
        db.save_classification(&result).unwrap();

        // Upsert with different payload
        let result2 = ClassificationResult {
            id: ClassificationId::from_u128(10),
            trace_id: TraceId::from_u128(1),
            classifier_name: "test".into(),
            timestamp: "2026-01-01T00:01:00Z".into(),
            payload: serde_json::json!({"v": 2}),
            notification: None,
        };
        db.save_classification(&result2).unwrap();

        let loaded = db.load_classifications(TraceId::from_u128(1)).unwrap();
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].payload["v"], 2);
    }

    #[test]
    fn list_recent_classifications_ordered_by_timestamp_desc() {
        let db = Database::open_in_memory().unwrap();
        let trace = orchestrator_core::Trace {
            id: TraceId::from_u128(1),
            agent_id: orchestrator_core::AgentId::from_u128(1),
            session_id: "s1".into(),
            status: orchestrator_core::TraceStatus::Active,
            started_at: "2026-01-01T00:00:00Z".into(),
            completed_at: None,
            turn_count: 0,
            token_usage: orchestrator_core::TokenUsage {
                input_tokens: 0,
                output_tokens: 0,
                cache_creation_input_tokens: None,
                cache_read_input_tokens: None,
            },
            provider: "claude".into(),
            model: None,
        };
        db.save_trace(&trace, &[]).unwrap();

        for i in 0..5 {
            let r = ClassificationResult {
                id: ClassificationId::from_u128(i + 10),
                trace_id: TraceId::from_u128(1),
                classifier_name: "test".into(),
                timestamp: format!("2026-01-01T00:0{}:00Z", i),
                payload: serde_json::json!({"i": i}),
                notification: None,
            };
            db.save_classification(&r).unwrap();
        }

        let recent = db.list_recent_classifications(3).unwrap();
        assert_eq!(recent.len(), 3);
        // Most recent first
        assert_eq!(recent[0].timestamp, "2026-01-01T00:04:00Z");
        assert_eq!(recent[2].timestamp, "2026-01-01T00:02:00Z");
    }
}
