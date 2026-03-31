use rusqlite::Connection;
use std::path::Path;

use crate::schema;

/// General-purpose project database.
///
/// Owns the SQLite connection and provides domain-specific operations
/// organized by module (traces, etc.). New domains add methods here.
pub struct Database {
    pub(crate) conn: Connection,
}

impl Database {
    /// Open (or create) the database at the given path and run migrations.
    pub fn open(path: &Path) -> anyhow::Result<Self> {
        let conn = Connection::open(path)?;
        schema::init(&conn)?;
        Ok(Self { conn })
    }

    /// Open an in-memory database. Useful for tests.
    pub fn open_in_memory() -> anyhow::Result<Self> {
        let conn = Connection::open_in_memory()?;
        schema::init(&conn)?;
        Ok(Self { conn })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn open_in_memory_succeeds() {
        let db = Database::open_in_memory().unwrap();
        // Verify tables exist
        let count: i64 = db
            .conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert!(count >= 3); // schema_version, traces, turns
    }
}
