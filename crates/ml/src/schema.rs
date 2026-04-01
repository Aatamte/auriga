/// SQL schema for ML tables (training labels + trained models).
///
/// Defined here but applied by the storage layer during database initialization.
pub const ML_SCHEMA_SQL: &str = "\
CREATE TABLE IF NOT EXISTS training_labels (
    trace_id TEXT NOT NULL,
    classifier_name TEXT NOT NULL,
    label TEXT NOT NULL,
    PRIMARY KEY (trace_id, classifier_name)
);

CREATE TABLE IF NOT EXISTS models (
    id TEXT PRIMARY KEY,
    classifier_name TEXT NOT NULL,
    version INTEGER NOT NULL,
    created_at TEXT NOT NULL,
    feature_names TEXT NOT NULL,
    model_data TEXT NOT NULL,
    accuracy REAL,
    UNIQUE(classifier_name, version)
);

CREATE INDEX IF NOT EXISTS idx_models_classifier_name
    ON models(classifier_name);
CREATE INDEX IF NOT EXISTS idx_training_labels_classifier_name
    ON training_labels(classifier_name);
";
