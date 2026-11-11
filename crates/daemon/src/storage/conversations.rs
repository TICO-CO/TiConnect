use rusqlite::{Connection, OptionalExtension, params};

use super::StorageError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Conversation {
    pub id: String,
    pub peer_id: String,
    pub created_at: i64,
}

pub fn ensure(
    connection: &Connection,
    id: &str,
    peer_id: &str,
    created_at: i64,
) -> Result<Conversation, StorageError> {
    connection.execute(
        "INSERT OR IGNORE INTO conversations (id, peer_id, created_at) VALUES (?1, ?2, ?3)",
        params![id, peer_id, created_at],
    )?;
    by_peer(connection, peer_id)?
        .ok_or_else(|| StorageError::Integrity("conversation missing".to_owned()))
}

pub fn by_peer(
    connection: &Connection,
    peer_id: &str,
) -> Result<Option<Conversation>, StorageError> {
    connection
        .query_row(
            "SELECT id, peer_id, created_at FROM conversations WHERE peer_id = ?1",
            params![peer_id],
            |row| {
                Ok(Conversation {
                    id: row.get(0)?,
                    peer_id: row.get(1)?,
                    created_at: row.get(2)?,
                })
            },
        )
        .optional()
        .map_err(Into::into)
}

pub fn by_id(connection: &Connection, id: &str) -> Result<Option<Conversation>, StorageError> {
    connection
        .query_row(
            "SELECT id, peer_id, created_at FROM conversations WHERE id = ?1",
            params![id],
            |row| {
                Ok(Conversation {
                    id: row.get(0)?,
                    peer_id: row.get(1)?,
                    created_at: row.get(2)?,
                })
            },
        )
        .optional()
        .map_err(Into::into)
}
