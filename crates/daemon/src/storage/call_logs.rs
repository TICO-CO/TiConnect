use rusqlite::{Connection, params};

use super::StorageError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CallRecord {
    pub id: String,
    pub user_id: String,
    pub direction: String,
    pub started_at: i64,
    pub connected_at: Option<i64>,
    pub ended_at: i64,
    pub duration_secs: u64,
    pub reason: String,
}

pub fn insert(connection: &Connection, record: &CallRecord) -> Result<(), StorageError> {
    let connected_at = record.connected_at.unwrap_or_default();
    let duration_secs = i64::try_from(record.duration_secs)
        .map_err(|_| StorageError::InvalidData("call duration is out of range".to_owned()))?;
    connection.execute(
        "INSERT OR REPLACE INTO call_logs
            (id, user_id, direction, started_at, connected_at, ended_at, duration_secs, reason)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        params![
            record.id,
            record.user_id,
            record.direction,
            record.started_at,
            connected_at,
            record.ended_at,
            duration_secs,
            record.reason,
        ],
    )?;
    Ok(())
}

pub fn list(
    connection: &Connection,
    user_id: Option<&str>,
    limit: u32,
) -> Result<Vec<CallRecord>, StorageError> {
    let mut statement = connection.prepare(
        "SELECT id, user_id, direction, started_at, connected_at, ended_at, duration_secs, reason
         FROM call_logs
         WHERE (?1 IS NULL OR user_id = ?1)
         ORDER BY ended_at DESC, id DESC
         LIMIT ?2",
    )?;
    let rows = statement.query_map(params![user_id, limit], |row| {
        let connected_at: i64 = row.get(4)?;
        let duration_secs: i64 = row.get(6)?;
        Ok((
            CallRecord {
                id: row.get(0)?,
                user_id: row.get(1)?,
                direction: row.get(2)?,
                started_at: row.get(3)?,
                connected_at: (connected_at > 0).then_some(connected_at),
                ended_at: row.get(5)?,
                duration_secs: 0,
                reason: row.get(7)?,
            },
            duration_secs,
        ))
    })?;
    rows.map(|row| {
        let (mut record, duration_secs) = row?;
        record.duration_secs = u64::try_from(duration_secs)
            .map_err(|_| StorageError::InvalidData("call duration is out of range".to_owned()))?;
        Ok(record)
    })
    .collect()
}

pub fn clear(connection: &Connection, user_id: Option<&str>) -> Result<u32, StorageError> {
    let removed = match user_id {
        Some(user_id) => {
            connection.execute("DELETE FROM call_logs WHERE user_id = ?1", params![user_id])?
        }
        None => connection.execute("DELETE FROM call_logs", [])?,
    };
    u32::try_from(removed)
        .map_err(|_| StorageError::InvalidData("call log count overflow".to_owned()))
}
