use rusqlite::{Connection, OptionalExtension, params};

use super::StorageError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransferPartial {
    pub hash: String,
    pub peer_id: String,
    pub size: u64,
    pub bytes_received: u64,
    pub temp_path: String,
    pub ranges: String,
    pub updated_at: i64,
}

pub fn upsert(connection: &Connection, partial: &TransferPartial) -> Result<(), StorageError> {
    connection.execute(
        "INSERT INTO transfer_partials
             (hash, peer_id, size, bytes_received, temp_path, ranges, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
         ON CONFLICT(hash) DO UPDATE SET peer_id = excluded.peer_id, size = excluded.size,
             bytes_received = excluded.bytes_received, temp_path = excluded.temp_path,
             ranges = excluded.ranges, updated_at = excluded.updated_at",
        params![
            partial.hash,
            partial.peer_id,
            i64::try_from(partial.size)
                .map_err(|_| StorageError::InvalidData("attachment size overflow".to_owned()))?,
            i64::try_from(partial.bytes_received).map_err(|_| {
                StorageError::InvalidData("attachment progress overflow".to_owned())
            })?,
            partial.temp_path,
            partial.ranges,
            partial.updated_at,
        ],
    )?;
    Ok(())
}

pub fn get(connection: &Connection, hash: &str) -> Result<Option<TransferPartial>, StorageError> {
    connection
        .query_row(
            "SELECT hash, peer_id, size, bytes_received, temp_path, ranges, updated_at
             FROM transfer_partials WHERE hash = ?1",
            params![hash],
            row_to_partial,
        )
        .optional()
        .map_err(Into::into)
}

pub fn list(connection: &Connection) -> Result<Vec<TransferPartial>, StorageError> {
    let mut statement = connection.prepare(
        "SELECT hash, peer_id, size, bytes_received, temp_path, ranges, updated_at
         FROM transfer_partials ORDER BY updated_at",
    )?;
    let rows = statement.query_map([], row_to_partial)?;
    let mut values = Vec::new();
    for row in rows {
        values.push(row?);
    }
    Ok(values)
}

pub fn remove(connection: &Connection, hash: &str) -> Result<bool, StorageError> {
    Ok(connection.execute(
        "DELETE FROM transfer_partials WHERE hash = ?1",
        params![hash],
    )? > 0)
}

fn row_to_partial(row: &rusqlite::Row<'_>) -> rusqlite::Result<TransferPartial> {
    let size: i64 = row.get(2)?;
    let bytes_received: i64 = row.get(3)?;
    Ok(TransferPartial {
        hash: row.get(0)?,
        peer_id: row.get(1)?,
        size: u64::try_from(size).map_err(to_sql_error)?,
        bytes_received: u64::try_from(bytes_received).map_err(to_sql_error)?,
        temp_path: row.get(4)?,
        ranges: row.get(5)?,
        updated_at: row.get(6)?,
    })
}

fn to_sql_error(error: impl std::error::Error + Send + Sync + 'static) -> rusqlite::Error {
    rusqlite::Error::ToSqlConversionFailure(Box::new(error))
}
