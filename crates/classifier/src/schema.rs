/// SQL schema for the classifications table.
///
/// Defined here but intended to be applied by the storage layer
/// during database initialization.
pub const CLASSIFICATIONS_TABLE_SQL: &str = "\
CREATE TABLE IF NOT EXISTS classifications (
    id TEXT PRIMARY KEY,
    trace_id TEXT NOT NULL REFERENCES traces(id),
    classifier_name TEXT NOT NULL,
    timestamp TEXT NOT NULL,
    payload TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_classifications_trace_id
    ON classifications(trace_id);
CREATE INDEX IF NOT EXISTS idx_classifications_classifier_name
    ON classifications(classifier_name);
";
