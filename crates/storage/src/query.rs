use std::path::Path;

use crate::db::Database;

/// Quote a SQLite identifier with double quotes, escaping embedded double quotes.
/// Used for table names from sqlite_master — trusted but quoted defensively.
fn quote_identifier(name: &str) -> String {
    format!("\"{}\"", name.replace('"', "\"\""))
}

#[derive(Debug, Clone)]
pub struct TableInfo {
    pub name: String,
    pub row_count: u64,
}

#[derive(Debug, Clone)]
pub struct DbMetadata {
    pub file_size_bytes: u64,
    pub tables: Vec<TableInfo>,
    pub total_rows: u64,
}

#[derive(Debug, Clone)]
pub struct QueryResult {
    pub columns: Vec<String>,
    pub rows: Vec<Vec<String>>,
    pub total_rows: u64,
}

impl Database {
    /// Get database metadata: file size, table list with row counts.
    pub fn metadata(&self, db_path: &Path) -> anyhow::Result<DbMetadata> {
        let file_size_bytes = std::fs::metadata(db_path).map(|m| m.len()).unwrap_or(0);

        let mut stmt = self.conn.prepare(
            "SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%' ORDER BY name",
        )?;

        let table_names: Vec<String> = stmt
            .query_map([], |row| row.get(0))?
            .filter_map(|r| r.ok())
            .collect();

        let mut tables = Vec::new();
        let mut total_rows: u64 = 0;

        for name in table_names {
            let count_sql = format!("SELECT COUNT(*) FROM {}", quote_identifier(&name));
            let row_count: u64 = self.conn.query_row(&count_sql, [], |row| row.get(0))?;
            total_rows += row_count;
            tables.push(TableInfo { name, row_count });
        }

        Ok(DbMetadata {
            file_size_bytes,
            tables,
            total_rows,
        })
    }

    /// Query rows from a table with pagination. Returns column names and string values.
    pub fn query_table(&self, table: &str, limit: u64, offset: u64) -> anyhow::Result<QueryResult> {
        // Validate table name exists to prevent SQL injection
        let exists: bool = self.conn.query_row(
            "SELECT COUNT(*) > 0 FROM sqlite_master WHERE type='table' AND name = ?1",
            [table],
            |row| row.get(0),
        )?;

        if !exists {
            anyhow::bail!("table '{}' not found", table);
        }

        let quoted = quote_identifier(table);
        let count_sql = format!("SELECT COUNT(*) FROM {}", quoted);
        let total_rows: u64 = self.conn.query_row(&count_sql, [], |row| row.get(0))?;

        let query_sql = format!("SELECT * FROM {} LIMIT {} OFFSET {}", quoted, limit, offset);
        let mut stmt = self.conn.prepare(&query_sql)?;

        let columns: Vec<String> = stmt.column_names().iter().map(|s| s.to_string()).collect();

        let col_count = columns.len();
        let rows: Vec<Vec<String>> = stmt
            .query_map([], |row| {
                let mut vals = Vec::with_capacity(col_count);
                for i in 0..col_count {
                    let val: String = match row.get_ref(i) {
                        Ok(rusqlite::types::ValueRef::Null) => "NULL".to_string(),
                        Ok(rusqlite::types::ValueRef::Integer(n)) => n.to_string(),
                        Ok(rusqlite::types::ValueRef::Real(f)) => f.to_string(),
                        Ok(rusqlite::types::ValueRef::Text(s)) => {
                            let s = String::from_utf8_lossy(s);
                            if s.chars().count() > 60 {
                                let truncated: String = s.chars().take(57).collect();
                                format!("{}...", truncated)
                            } else {
                                s.to_string()
                            }
                        }
                        Ok(rusqlite::types::ValueRef::Blob(b)) => {
                            format!("<blob {} bytes>", b.len())
                        }
                        Err(_) => "?".to_string(),
                    };
                    vals.push(val);
                }
                Ok(vals)
            })?
            .filter_map(|r| r.ok())
            .collect();

        Ok(QueryResult {
            columns,
            rows,
            total_rows,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::Database;

    fn make_db() -> Database {
        let db = Database::open_in_memory().unwrap();
        // Insert some test data
        db.conn
            .execute_batch(
                "INSERT INTO traces (id, agent_id, session_id, status, started_at, turn_count, input_tokens, output_tokens, provider)
                 VALUES ('t1', 'a1', 's1', 'Complete', '2026-01-01', 2, 100, 50, 'claude');
                 INSERT INTO traces (id, agent_id, session_id, status, started_at, turn_count, input_tokens, output_tokens, provider)
                 VALUES ('t2', 'a1', 's2', 'Active', '2026-01-02', 1, 200, 80, 'claude');",
            )
            .unwrap();
        db
    }

    #[test]
    fn quote_identifier_wraps_in_double_quotes() {
        assert_eq!(quote_identifier("traces"), "\"traces\"");
    }

    #[test]
    fn quote_identifier_escapes_embedded_double_quotes() {
        assert_eq!(quote_identifier("has\"quote"), "\"has\"\"quote\"");
    }

    #[test]
    fn metadata_lists_tables() {
        let db = make_db();
        // In-memory DB has no file, so pass a dummy path
        let meta = db.metadata(Path::new("/nonexistent")).unwrap();
        assert_eq!(meta.file_size_bytes, 0); // file doesn't exist
        assert!(meta.tables.len() >= 3); // schema_version, traces, turns
        assert!(meta.total_rows >= 2); // at least the 2 traces we inserted
    }

    #[test]
    fn metadata_counts_rows() {
        let db = make_db();
        let meta = db.metadata(Path::new("/nonexistent")).unwrap();
        let traces_table = meta.tables.iter().find(|t| t.name == "traces").unwrap();
        assert_eq!(traces_table.row_count, 2);
    }

    #[test]
    fn query_table_returns_columns_and_rows() {
        let db = make_db();
        let result = db.query_table("traces", 10, 0).unwrap();
        assert!(result.columns.contains(&"id".to_string()));
        assert!(result.columns.contains(&"status".to_string()));
        assert_eq!(result.rows.len(), 2);
        assert_eq!(result.total_rows, 2);
    }

    #[test]
    fn query_table_pagination() {
        let db = make_db();
        let page1 = db.query_table("traces", 1, 0).unwrap();
        assert_eq!(page1.rows.len(), 1);
        assert_eq!(page1.total_rows, 2);

        let page2 = db.query_table("traces", 1, 1).unwrap();
        assert_eq!(page2.rows.len(), 1);
    }

    #[test]
    fn query_table_offset_past_end() {
        let db = make_db();
        let result = db.query_table("traces", 10, 100).unwrap();
        assert!(result.rows.is_empty());
        assert_eq!(result.total_rows, 2);
    }

    #[test]
    fn query_table_nonexistent_fails() {
        let db = make_db();
        assert!(db.query_table("nonexistent", 10, 0).is_err());
    }

    #[test]
    fn query_table_handles_null_values() {
        let db = make_db();
        let result = db.query_table("traces", 10, 0).unwrap();
        // model column is nullable, should show "NULL"
        let model_idx = result.columns.iter().position(|c| c == "model").unwrap();
        assert_eq!(result.rows[0][model_idx], "NULL");
    }

    #[test]
    fn query_table_truncates_long_text() {
        let db = Database::open_in_memory().unwrap();
        db.conn
            .execute_batch(&format!(
                "CREATE TABLE test_long (data TEXT);
                 INSERT INTO test_long VALUES ('{}');",
                "x".repeat(100)
            ))
            .unwrap();
        let result = db.query_table("test_long", 10, 0).unwrap();
        assert!(result.rows[0][0].len() <= 60);
        assert!(result.rows[0][0].ends_with("..."));
    }
}
