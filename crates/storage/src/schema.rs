use rusqlite::Connection;

const CURRENT_VERSION: i64 = 1;

const SCHEMA_V1: &str = "
CREATE TABLE IF NOT EXISTS schema_version (
    version INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS traces (
    id TEXT PRIMARY KEY,
    agent_id TEXT NOT NULL,
    session_id TEXT NOT NULL,
    status TEXT NOT NULL,
    started_at TEXT NOT NULL,
    completed_at TEXT,
    turn_count INTEGER NOT NULL,
    input_tokens INTEGER NOT NULL,
    output_tokens INTEGER NOT NULL,
    cache_creation_input_tokens INTEGER,
    cache_read_input_tokens INTEGER,
    provider TEXT NOT NULL,
    model TEXT
);

CREATE TABLE IF NOT EXISTS turns (
    id INTEGER PRIMARY KEY,
    trace_id TEXT NOT NULL REFERENCES traces(id),
    agent_id TEXT NOT NULL,
    number INTEGER NOT NULL,
    status TEXT NOT NULL,
    uuid TEXT NOT NULL,
    parent_uuid TEXT,
    session_id TEXT,
    timestamp TEXT NOT NULL,
    message_type TEXT NOT NULL,
    cwd TEXT,
    git_branch TEXT,
    role TEXT NOT NULL,
    content TEXT NOT NULL,
    meta TEXT NOT NULL,
    extra TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_turns_trace_id ON turns(trace_id);
CREATE INDEX IF NOT EXISTS idx_traces_agent_id ON traces(agent_id);
";

pub fn init(conn: &Connection) -> rusqlite::Result<()> {
    let version = get_version(conn);

    if version < CURRENT_VERSION {
        conn.execute_batch(SCHEMA_V1)?;
        set_version(conn, CURRENT_VERSION)?;
    }

    Ok(())
}

fn get_version(conn: &Connection) -> i64 {
    // Table may not exist yet
    let table_exists: bool = conn
        .query_row(
            "SELECT COUNT(*) > 0 FROM sqlite_master WHERE type='table' AND name='schema_version'",
            [],
            |row| row.get(0),
        )
        .unwrap_or(false);

    if !table_exists {
        return 0;
    }

    conn.query_row("SELECT version FROM schema_version LIMIT 1", [], |row| {
        row.get(0)
    })
    .unwrap_or(0)
}

fn set_version(conn: &Connection, version: i64) -> rusqlite::Result<()> {
    conn.execute("DELETE FROM schema_version", [])?;
    conn.execute("INSERT INTO schema_version (version) VALUES (?1)", [version])?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn init_creates_tables() {
        let conn = Connection::open_in_memory().unwrap();
        init(&conn).unwrap();

        let tables: Vec<String> = conn
            .prepare("SELECT name FROM sqlite_master WHERE type='table' ORDER BY name")
            .unwrap()
            .query_map([], |row| row.get(0))
            .unwrap()
            .filter_map(|r| r.ok())
            .collect();

        assert!(tables.contains(&"traces".to_string()));
        assert!(tables.contains(&"turns".to_string()));
        assert!(tables.contains(&"schema_version".to_string()));
    }

    #[test]
    fn init_sets_version() {
        let conn = Connection::open_in_memory().unwrap();
        init(&conn).unwrap();
        assert_eq!(get_version(&conn), CURRENT_VERSION);
    }

    #[test]
    fn init_is_idempotent() {
        let conn = Connection::open_in_memory().unwrap();
        init(&conn).unwrap();
        init(&conn).unwrap();
        assert_eq!(get_version(&conn), CURRENT_VERSION);
    }

    #[test]
    fn get_version_returns_zero_for_empty_db() {
        let conn = Connection::open_in_memory().unwrap();
        assert_eq!(get_version(&conn), 0);
    }
}
