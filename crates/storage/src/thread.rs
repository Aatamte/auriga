use std::path::PathBuf;
use std::sync::mpsc;
use std::thread;

use orchestrator_classifier::ClassificationResult;
use orchestrator_core::{Trace, TraceId, Turn};
use orchestrator_ml::SavedModel;

use crate::db::Database;

/// Commands processed by the storage background thread.
/// New domain operations are added as variants here.
pub enum StorageCommand {
    SaveTrace {
        trace: Box<Trace>,
        turns: Vec<Turn>,
    },
    SaveClassification {
        result: ClassificationResult,
    },
    SaveModel {
        model: SavedModel,
    },
    SaveTrainingLabel {
        trace_id: TraceId,
        classifier_name: String,
        label: String,
    },
    Shutdown,
}

/// Handle to the storage background thread. Non-blocking sends.
pub struct StorageHandle {
    tx: mpsc::Sender<StorageCommand>,
    join: Option<thread::JoinHandle<()>>,
}

impl StorageHandle {
    /// Queue a trace + turns for persistence. Non-blocking.
    pub fn save_trace(&self, trace: Trace, turns: Vec<Turn>) {
        let _ = self.tx.send(StorageCommand::SaveTrace {
            trace: Box::new(trace),
            turns,
        });
    }

    /// Queue a classification result for persistence. Non-blocking.
    pub fn save_classification(&self, result: ClassificationResult) {
        let _ = self.tx.send(StorageCommand::SaveClassification { result });
    }

    /// Queue a trained model for persistence. Non-blocking.
    pub fn save_model(&self, model: SavedModel) {
        let _ = self.tx.send(StorageCommand::SaveModel { model });
    }

    /// Queue a training label for persistence. Non-blocking.
    pub fn save_training_label(&self, trace_id: TraceId, classifier_name: String, label: String) {
        let _ = self.tx.send(StorageCommand::SaveTrainingLabel {
            trace_id,
            classifier_name,
            label,
        });
    }

    /// Signal shutdown and wait for the background thread to finish.
    pub fn shutdown(&mut self) {
        let _ = self.tx.send(StorageCommand::Shutdown);
        if let Some(handle) = self.join.take() {
            let _ = handle.join();
        }
    }
}

impl Drop for StorageHandle {
    fn drop(&mut self) {
        self.shutdown();
    }
}

/// Start the storage background thread. Opens the database and processes
/// commands until shutdown. Returns a handle for non-blocking sends.
pub fn start_storage_thread(db_path: PathBuf) -> anyhow::Result<StorageHandle> {
    let db = Database::open(&db_path)?;
    let (tx, rx) = mpsc::channel();

    let join = thread::Builder::new()
        .name("storage".to_string())
        .spawn(move || {
            while let Ok(cmd) = rx.recv() {
                match cmd {
                    StorageCommand::SaveTrace { trace, turns } => {
                        let turn_refs: Vec<&Turn> = turns.iter().collect();
                        if let Err(e) = db.save_trace(&trace, &turn_refs) {
                            eprintln!("storage error: {}", e);
                        }
                    }
                    StorageCommand::SaveClassification { result } => {
                        if let Err(e) = db.save_classification(&result) {
                            eprintln!("storage error: {}", e);
                        }
                    }
                    StorageCommand::SaveModel { model } => {
                        if let Err(e) = db.save_model(&model) {
                            eprintln!("storage error: {}", e);
                        }
                    }
                    StorageCommand::SaveTrainingLabel {
                        trace_id,
                        classifier_name,
                        label,
                    } => {
                        if let Err(e) = db.save_training_label(trace_id, &classifier_name, &label) {
                            eprintln!("storage error: {}", e);
                        }
                    }
                    StorageCommand::Shutdown => break,
                }
            }
        })?;

    Ok(StorageHandle {
        tx,
        join: Some(join),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use orchestrator_core::{AgentId, TokenUsage, TraceId, TraceStatus};
    use rusqlite::Connection;

    fn sample_trace() -> Trace {
        Trace {
            id: TraceId::from_u128(1),
            agent_id: AgentId::from_u128(1),
            session_id: "sess-1".to_string(),
            status: TraceStatus::Complete,
            started_at: "2026-03-01T10:00:00Z".to_string(),
            completed_at: Some("2026-03-01T10:05:00Z".to_string()),
            turn_count: 0,
            token_usage: TokenUsage {
                input_tokens: 0,
                output_tokens: 0,
                cache_creation_input_tokens: None,
                cache_read_input_tokens: None,
            },
            provider: "claude".to_string(),
            model: None,
        }
    }

    #[test]
    fn handle_saves_and_shuts_down() {
        let db_path = std::env::temp_dir().join("orch_test_save.db");
        let _ = std::fs::remove_file(&db_path);

        let mut handle = start_storage_thread(db_path.clone()).unwrap();
        handle.save_trace(sample_trace(), vec![]);
        handle.shutdown();

        let conn = Connection::open(&db_path).unwrap();
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM traces", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn drop_triggers_shutdown() {
        let db_path = std::env::temp_dir().join("orch_test_drop.db");
        let _ = std::fs::remove_file(&db_path);

        {
            let handle = start_storage_thread(db_path.clone()).unwrap();
            handle.save_trace(sample_trace(), vec![]);
        }

        let conn = Connection::open(&db_path).unwrap();
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM traces", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 1);
    }
}
