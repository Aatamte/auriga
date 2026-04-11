use auriga_core::TraceId;
use auriga_ml::SavedModel;
use rusqlite::params;
use uuid::Uuid;

use crate::Database;

impl Database {
    pub fn save_training_label(
        &self,
        trace_id: TraceId,
        classifier_name: &str,
        position: u32,
        label: &str,
    ) -> anyhow::Result<()> {
        self.conn.execute(
            "INSERT OR REPLACE INTO training_labels \
             (trace_id, classifier_name, position, label) VALUES (?1, ?2, ?3, ?4)",
            params![trace_id.0.to_string(), classifier_name, position, label],
        )?;
        Ok(())
    }

    pub fn load_training_labels(
        &self,
        classifier_name: &str,
    ) -> anyhow::Result<Vec<(TraceId, u32, String)>> {
        let mut stmt = self.conn.prepare(
            "SELECT trace_id, position, label FROM training_labels WHERE classifier_name = ?1",
        )?;

        let results = stmt
            .query_map(params![classifier_name], |row| {
                let trace_id_str: String = row.get(0)?;
                let position: u32 = row.get(1)?;
                let label: String = row.get(2)?;
                Ok((trace_id_str, position, label))
            })?
            .filter_map(|r| r.ok())
            .filter_map(|(id_str, position, label)| {
                let uuid = Uuid::parse_str(&id_str).ok()?;
                Some((TraceId(uuid), position, label))
            })
            .collect();

        Ok(results)
    }

    pub fn save_model(&self, model: &SavedModel) -> anyhow::Result<()> {
        let feature_names_json = serde_json::to_string(&model.feature_names)?;
        self.conn.execute(
            "INSERT OR REPLACE INTO models \
             (id, classifier_name, version, created_at, feature_names, model_data, accuracy) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                model.id,
                model.classifier_name,
                model.version,
                model.created_at,
                feature_names_json,
                model.model_data,
                model.accuracy,
            ],
        )?;
        Ok(())
    }

    pub fn load_latest_model(&self, classifier_name: &str) -> anyhow::Result<Option<SavedModel>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, classifier_name, version, created_at, feature_names, model_data, accuracy \
             FROM models WHERE classifier_name = ?1 ORDER BY version DESC LIMIT 1",
        )?;

        let mut rows = stmt.query_map(params![classifier_name], |row| {
            let id: String = row.get(0)?;
            let classifier_name: String = row.get(1)?;
            let version: i64 = row.get(2)?;
            let created_at: String = row.get(3)?;
            let feature_names_json: String = row.get(4)?;
            let model_data: String = row.get(5)?;
            let accuracy: Option<f64> = row.get(6)?;
            Ok((
                id,
                classifier_name,
                version,
                created_at,
                feature_names_json,
                model_data,
                accuracy,
            ))
        })?;

        let Some(Ok((id, name, version, created_at, fn_json, model_data, accuracy))) = rows.next()
        else {
            return Ok(None);
        };

        let feature_names: Vec<String> = serde_json::from_str(&fn_json)?;

        Ok(Some(SavedModel {
            id,
            classifier_name: name,
            version,
            created_at,
            feature_names,
            model_data,
            accuracy,
        }))
    }

    pub fn list_models(&self, classifier_name: &str) -> anyhow::Result<Vec<SavedModel>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, classifier_name, version, created_at, feature_names, model_data, accuracy \
             FROM models WHERE classifier_name = ?1 ORDER BY version DESC",
        )?;

        let results = stmt
            .query_map(params![classifier_name], |row| {
                let id: String = row.get(0)?;
                let classifier_name: String = row.get(1)?;
                let version: i64 = row.get(2)?;
                let created_at: String = row.get(3)?;
                let feature_names_json: String = row.get(4)?;
                let model_data: String = row.get(5)?;
                let accuracy: Option<f64> = row.get(6)?;
                Ok((
                    id,
                    classifier_name,
                    version,
                    created_at,
                    feature_names_json,
                    model_data,
                    accuracy,
                ))
            })?
            .filter_map(|r| r.ok())
            .filter_map(
                |(id, name, version, created_at, fn_json, model_data, accuracy)| {
                    let feature_names: Vec<String> = serde_json::from_str(&fn_json).ok()?;
                    Some(SavedModel {
                        id,
                        classifier_name: name,
                        version,
                        created_at,
                        feature_names,
                        model_data,
                        accuracy,
                    })
                },
            )
            .collect();

        Ok(results)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn save_and_load_training_label() {
        let db = Database::open_in_memory().unwrap();
        let trace_id = TraceId::from_u128(1);

        // Need a trace first (FK might not be enforced, but let's be safe)
        let trace = auriga_core::Trace {
            id: trace_id,
            agent_id: auriga_core::AgentId::from_u128(1),
            session_id: "s1".into(),
            status: auriga_core::TraceStatus::Complete,
            started_at: "2026-01-01T00:00:00Z".into(),
            completed_at: None,
            turn_count: 0,
            token_usage: auriga_core::TokenUsage {
                input_tokens: 0,
                output_tokens: 0,
                cache_creation_input_tokens: None,
                cache_read_input_tokens: None,
            },
            provider: "claude".into(),
            model: None,
        };
        db.save_trace(&trace, &[]).unwrap();

        db.save_training_label(trace_id, "test-clf", 5, "success")
            .unwrap();

        let labels = db.load_training_labels("test-clf").unwrap();
        assert_eq!(labels.len(), 1);
        assert_eq!(labels[0].0, trace_id);
        assert_eq!(labels[0].1, 5);
        assert_eq!(labels[0].2, "success");
    }

    #[test]
    fn save_and_load_model() {
        let db = Database::open_in_memory().unwrap();

        let model = SavedModel {
            id: "m1".into(),
            classifier_name: "test-clf".into(),
            version: 1,
            created_at: "2026-01-01T00:00:00Z".into(),
            feature_names: vec!["turn_count".into(), "tokens".into()],
            model_data: "{\"tree\": \"data\"}".into(),
            accuracy: Some(0.95),
        };
        db.save_model(&model).unwrap();

        let loaded = db.load_latest_model("test-clf").unwrap();
        assert!(loaded.is_some());
        let loaded = loaded.unwrap();
        assert_eq!(loaded.id, "m1");
        assert_eq!(loaded.version, 1);
        assert_eq!(loaded.accuracy, Some(0.95));
        assert_eq!(loaded.feature_names, vec!["turn_count", "tokens"]);
    }

    #[test]
    fn load_latest_model_returns_highest_version() {
        let db = Database::open_in_memory().unwrap();

        for v in 1..=3 {
            let model = SavedModel {
                id: format!("m{}", v),
                classifier_name: "test".into(),
                version: v,
                created_at: "2026-01-01T00:00:00Z".into(),
                feature_names: vec![],
                model_data: "{}".into(),
                accuracy: None,
            };
            db.save_model(&model).unwrap();
        }

        let latest = db.load_latest_model("test").unwrap().unwrap();
        assert_eq!(latest.version, 3);
    }

    #[test]
    fn load_latest_model_none_for_unknown() {
        let db = Database::open_in_memory().unwrap();
        assert!(db.load_latest_model("nope").unwrap().is_none());
    }

    #[test]
    fn list_models_returns_all_versions() {
        let db = Database::open_in_memory().unwrap();

        for v in 1..=3 {
            let model = SavedModel {
                id: format!("m{}", v),
                classifier_name: "test".into(),
                version: v,
                created_at: "2026-01-01T00:00:00Z".into(),
                feature_names: vec![],
                model_data: "{}".into(),
                accuracy: None,
            };
            db.save_model(&model).unwrap();
        }

        let models = db.list_models("test").unwrap();
        assert_eq!(models.len(), 3);
        assert_eq!(models[0].version, 3); // desc order
    }

    #[test]
    fn load_training_labels_empty_for_unknown_classifier() {
        let db = Database::open_in_memory().unwrap();
        let labels = db.load_training_labels("nonexistent").unwrap();
        assert!(labels.is_empty());
    }

    #[test]
    fn save_training_label_upserts_on_same_key() {
        let db = Database::open_in_memory().unwrap();
        let trace_id = TraceId::from_u128(1);

        let trace = auriga_core::Trace {
            id: trace_id,
            agent_id: auriga_core::AgentId::from_u128(1),
            session_id: "s1".into(),
            status: auriga_core::TraceStatus::Complete,
            started_at: "2026-01-01T00:00:00Z".into(),
            completed_at: None,
            turn_count: 0,
            token_usage: auriga_core::TokenUsage {
                input_tokens: 0,
                output_tokens: 0,
                cache_creation_input_tokens: None,
                cache_read_input_tokens: None,
            },
            provider: "claude".into(),
            model: None,
        };
        db.save_trace(&trace, &[]).unwrap();

        db.save_training_label(trace_id, "clf", 1, "good").unwrap();
        db.save_training_label(trace_id, "clf", 1, "bad").unwrap();

        let labels = db.load_training_labels("clf").unwrap();
        assert_eq!(labels.len(), 1);
        assert_eq!(labels[0].2, "bad");
    }

    #[test]
    fn save_model_with_no_accuracy() {
        let db = Database::open_in_memory().unwrap();

        let model = SavedModel {
            id: "m-none".into(),
            classifier_name: "test".into(),
            version: 1,
            created_at: "2026-01-01T00:00:00Z".into(),
            feature_names: vec!["f1".into()],
            model_data: "{}".into(),
            accuracy: None,
        };
        db.save_model(&model).unwrap();

        let loaded = db.load_latest_model("test").unwrap().unwrap();
        assert!(loaded.accuracy.is_none());
        assert_eq!(loaded.feature_names, vec!["f1"]);
    }

    #[test]
    fn list_models_empty_for_unknown_classifier() {
        let db = Database::open_in_memory().unwrap();
        let models = db.list_models("nonexistent").unwrap();
        assert!(models.is_empty());
    }
}
