//! Integration tests for auriga-storage public API

use auriga_storage::{Database, DbMetadata, QueryResult, TableInfo};
use std::path::Path;

#[test]
fn database_open_in_memory() {
    let db = Database::open_in_memory().unwrap();
    // Should succeed without error
    assert!(true);
    drop(db);
}

#[test]
fn database_creates_schema() {
    let db = Database::open_in_memory().unwrap();
    // Metadata should work even with dummy path for in-memory db
    let meta = db.metadata(Path::new("/dummy")).unwrap();
    // Should have at least the core tables
    assert!(meta.tables.len() >= 2);
}

#[test]
fn database_metadata_returns_table_info() {
    let db = Database::open_in_memory().unwrap();
    let meta = db.metadata(Path::new("/dummy")).unwrap();

    // Check metadata structure
    assert!(meta.tables.iter().any(|t| t.name == "traces"));
    assert!(meta.tables.iter().any(|t| t.name == "turns"));
}

#[test]
fn database_query_table_traces() {
    let db = Database::open_in_memory().unwrap();

    // Query empty traces table
    let result = db.query_table("traces", 10, 0).unwrap();
    assert!(!result.columns.is_empty());
    assert!(result.columns.contains(&"id".to_string()));
    assert_eq!(result.rows.len(), 0); // empty
    assert_eq!(result.total_rows, 0);
}

#[test]
fn database_query_table_nonexistent_fails() {
    let db = Database::open_in_memory().unwrap();
    let result = db.query_table("nonexistent_table_xyz", 10, 0);
    assert!(result.is_err());
}

#[test]
fn database_query_table_pagination() {
    let db = Database::open_in_memory().unwrap();

    // Pagination with empty table should work
    let page1 = db.query_table("traces", 5, 0).unwrap();
    let page2 = db.query_table("traces", 5, 5).unwrap();

    assert_eq!(page1.total_rows, 0);
    assert_eq!(page2.total_rows, 0);
}

// -- Type structure tests --

#[test]
fn table_info_structure() {
    let info = TableInfo {
        name: "test".to_string(),
        row_count: 100,
    };

    assert_eq!(info.name, "test");
    assert_eq!(info.row_count, 100);
}

#[test]
fn db_metadata_structure() {
    let meta = DbMetadata {
        file_size_bytes: 4096,
        tables: vec![TableInfo {
            name: "traces".to_string(),
            row_count: 10,
        }],
        total_rows: 10,
    };

    assert_eq!(meta.file_size_bytes, 4096);
    assert_eq!(meta.tables.len(), 1);
    assert_eq!(meta.total_rows, 10);
}

#[test]
fn query_result_structure() {
    let result = QueryResult {
        columns: vec!["id".to_string(), "name".to_string()],
        rows: vec![
            vec!["1".to_string(), "Alice".to_string()],
            vec!["2".to_string(), "Bob".to_string()],
        ],
        total_rows: 2,
    };

    assert_eq!(result.columns.len(), 2);
    assert_eq!(result.rows.len(), 2);
    assert_eq!(result.total_rows, 2);
}
