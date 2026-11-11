use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::{Connection, params};

use super::StorageError;

const LATEST_VERSION: i64 = 2;
const MIGRATIONS: &[(i64, &str)] = &[
    (1, include_str!("migrations/0001_init.sql")),
    (2, include_str!("migrations/0002_messaging.sql")),
];

pub fn run(connection: &mut Connection) -> Result<(), StorageError> {
    connection.execute_batch(
        "CREATE TABLE IF NOT EXISTS schema_version (
            version INTEGER PRIMARY KEY,
            applied_at INTEGER NOT NULL
        );",
    )?;

    let current: i64 = connection.query_row(
        "SELECT COALESCE(MAX(version), 0) FROM schema_version",
        [],
        |row| row.get(0),
    )?;

    if current > LATEST_VERSION {
        return Err(StorageError::SchemaTooNew {
            current,
            latest: LATEST_VERSION,
        });
    }

    for (version, sql) in MIGRATIONS {
        if *version <= current {
            continue;
        }

        let transaction = connection.transaction()?;
        transaction.execute_batch(sql)?;
        transaction.execute(
            "INSERT INTO schema_version (version, applied_at) VALUES (?1, ?2)",
            params![version, now_ms()],
        )?;
        transaction.commit()?;
    }

    Ok(())
}

pub fn latest_version() -> i64 {
    LATEST_VERSION
}

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_millis() as i64)
}
