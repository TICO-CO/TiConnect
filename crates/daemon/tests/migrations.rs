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
fn peer_state_migration_preserves_rows_and_removes_peer_fk() {
    let path = temp_db();
    let mut connection = Connection::open(&path).expect("open db");
    connection
        .execute_batch(
            "CREATE TABLE schema_version (version INTEGER PRIMARY KEY, applied_at INTEGER NOT NULL);
             INSERT INTO schema_version VALUES (1, 0), (2, 0);",
        )
        .expect("schema version");
    connection
        .execute_batch(include_str!("../src/storage/migrations/0001_init.sql"))
        .expect("v1 schema");
    connection
        .execute_batch(include_str!("../src/storage/migrations/0002_messaging.sql"))
        .expect("v2 schema");
    connection
        .execute_batch(
            "INSERT INTO peers
                 (id, fingerprint, public_key, display_name, policy, trusted_at, last_seen)
             VALUES ('peer-1', 'fingerprint-1', NULL, NULL, 'accept', 1, NULL);
             INSERT INTO peer_state (peer_id, last_sent_seq, last_recv_seq)
             VALUES ('peer-1', 7, 3);",
        )
        .expect("seed data");

    migrations::run(&mut connection).expect("migration");
    assert_eq!(schema_version(&connection), migrations::latest_version());

    let state: (i64, i64) = connection
        .query_row(
            "SELECT last_sent_seq, last_recv_seq FROM peer_state WHERE peer_id = 'peer-1'",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .expect("peer state");
    assert_eq!(state, (7, 3));

    let foreign_keys: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM pragma_foreign_key_list('peer_state')",
            [],
            |row| row.get(0),
        )
        .expect("foreign keys");
    assert_eq!(foreign_keys, 0);

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
