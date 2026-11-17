use rusqlite::{Connection, OptionalExtension, params};

use super::StorageError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Conversation {
    pub id: String,
    pub user_id: String,
    pub created_at: i64,
}

pub fn ensure(
    connection: &Connection,
    id: &str,
    user_id: &str,
    created_at: i64,
) -> Result<Conversation, StorageError> {
    connection.execute(
        "INSERT OR IGNORE INTO conversations (id, user_id, created_at) VALUES (?1, ?2, ?3)",
        params![id, user_id, created_at],
    )?;
    by_user(connection, user_id)?
        .ok_or_else(|| StorageError::Integrity("conversation missing".to_owned()))
}

pub fn by_user(
    connection: &Connection,
    user_id: &str,
) -> Result<Option<Conversation>, StorageError> {
    connection
        .query_row(
            "SELECT id, user_id, created_at FROM conversations WHERE user_id = ?1",
            params![user_id],
            |row| {
                Ok(Conversation {
                    id: row.get(0)?,
                    user_id: row.get(1)?,
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
            "SELECT id, user_id, created_at FROM conversations WHERE id = ?1",
            params![id],
            |row| {
                Ok(Conversation {
                    id: row.get(0)?,
                    user_id: row.get(1)?,
                    created_at: row.get(2)?,
                })
            },
        )
        .optional()
        .map_err(Into::into)
}
