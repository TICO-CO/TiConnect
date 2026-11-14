use rusqlite::{Connection, OptionalExtension, params};

use super::StorageError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AttachmentRecord {
    pub hash: String,
    pub size: u64,
    pub path: String,
    pub mime: String,
    pub name: String,
    pub created_at: i64,
}

pub fn upsert(connection: &Connection, attachment: &AttachmentRecord) -> Result<(), StorageError> {
    connection.execute(
        "INSERT INTO attachments (hash, size, path, mime, name, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)
         ON CONFLICT(hash) DO UPDATE SET size = excluded.size, path = excluded.path,
             mime = excluded.mime, name = excluded.name, created_at = excluded.created_at",
        params![
            attachment.hash,
            i64::try_from(attachment.size)
                .map_err(|_| StorageError::InvalidData("attachment size overflow".to_owned()))?,
            attachment.path,
            attachment.mime,
            attachment.name,
            attachment.created_at,
        ],
    )?;
    Ok(())
}

pub fn get(connection: &Connection, hash: &str) -> Result<Option<AttachmentRecord>, StorageError> {
    connection
        .query_row(
            "SELECT hash, size, path, mime, COALESCE(name, ''), created_at
             FROM attachments WHERE hash = ?1",
            params![hash],
            |row| {
                let size: i64 = row.get(1)?;
                Ok(AttachmentRecord {
                    hash: row.get(0)?,
                    size: u64::try_from(size).map_err(to_sql_error)?,
                    path: row.get(2)?,
                    mime: row.get::<_, Option<String>>(3)?.unwrap_or_default(),
                    name: row.get(4)?,
                    created_at: row.get(5)?,
                })
            },
        )
        .optional()
        .map_err(Into::into)
}

pub fn list(connection: &Connection) -> Result<Vec<AttachmentRecord>, StorageError> {
    let mut statement = connection.prepare(
        "SELECT hash, size, path, mime, COALESCE(name, ''), created_at
         FROM attachments ORDER BY created_at, hash",
    )?;
    let rows = statement.query_map([], |row| {
        let size: i64 = row.get(1)?;
        Ok(AttachmentRecord {
            hash: row.get(0)?,
            size: u64::try_from(size).map_err(to_sql_error)?,
            path: row.get(2)?,
            mime: row.get::<_, Option<String>>(3)?.unwrap_or_default(),
            name: row.get(4)?,
            created_at: row.get(5)?,
        })
    })?;
    let mut values = Vec::new();
    for row in rows {
        values.push(row?);
    }
    Ok(values)
}

fn to_sql_error(error: impl std::error::Error + Send + Sync + 'static) -> rusqlite::Error {
    rusqlite::Error::ToSqlConversionFailure(Box::new(error))
}
