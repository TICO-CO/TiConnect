use std::path::PathBuf;

use rusqlite::Connection;
use ticonnect_daemon::storage::migrations;

#[test]
fn migrations_apply_from_empty_and_are_idempotent() {
    let path = temp_db();
    let mut connection = Connection::open(&path).expect("open db");

    migrations::run(&mut connection).expect("first migration");
    assert_eq!(schema_version(&connection), migrations::latest_version());
    assert_eq!(table_count(&connection), 7);
    let settings_table: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = 'settings'",
            [],
            |row| row.get(0),
        )
        .expect("settings table count");
    assert_eq!(settings_table, 0);

    migrations::run(&mut connection).expect("second migration");
    assert_eq!(schema_version(&connection), migrations::latest_version());
    assert_eq!(table_count(&connection), 7);

    let _ = std::fs::remove_file(path);
}

#[test]
fn required_indexes_exist() {
    let path = temp_db();
    let mut connection = Connection::open(&path).expect("open db");
    migrations::run(&mut connection).expect("migration");

    let count: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master
             WHERE type = 'index' AND name IN (?, ?)",
            rusqlite::params![
                "messages_conversation_sent_at",
                "messages_conversation_sender_seq"
            ],
            |row| row.get(0),
        )
        .expect("index count");
    assert_eq!(count, 2);

    let _ = std::fs::remove_file(path);
}

#[test]
fn newer_schema_is_rejected() {
    let path = temp_db();
    let mut connection = Connection::open(&path).expect("open db");
    connection
        .execute_batch("CREATE TABLE schema_version (version INTEGER PRIMARY KEY, applied_at INTEGER NOT NULL); INSERT INTO schema_version VALUES (99, 0);")
        .expect("seed schema");

    let error = migrations::run(&mut connection).expect_err("newer schema");
    assert!(matches!(
        error,
        ticonnect_daemon::storage::StorageError::SchemaTooNew { .. }
    ));

    let _ = std::fs::remove_file(path);
}

fn schema_version(connection: &Connection) -> i64 {
    connection
        .query_row(
            "SELECT COALESCE(MAX(version), 0) FROM schema_version",
            [],
            |row| row.get(0),
        )
        .expect("schema version")
}

fn table_count(connection: &Connection) -> i64 {
    connection
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name NOT LIKE 'sqlite_%'",
            [],
            |row| row.get(0),
        )
        .expect("table count")
}

fn temp_db() -> PathBuf {
    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT: AtomicU64 = AtomicU64::new(0);
    let id = NEXT.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!(
        "ticonnect-migrations-{}-{id}.db",
        std::process::id()
    ))
}
