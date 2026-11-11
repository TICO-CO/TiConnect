use rusqlite::{Connection, params};

use super::StorageError;

pub const OUTBOX_BATCH: u32 = 128;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutboxEntry {
    pub message_id: String,
    pub attempts: u32,
    pub next_try: i64,
    pub expires_at: i64,
}

pub fn enqueue(
    connection: &Connection,
    message_id: &str,
    next_try: i64,
    expires_at: i64,
) -> Result<(), StorageError> {
    connection.execute(
        "INSERT OR REPLACE INTO outbox (message_id, attempts, next_try, expires_at)
         VALUES (?1, 0, ?2, ?3)",
        params![message_id, next_try, expires_at],
    )?;
    Ok(())
}

pub fn due_for_peer(
    connection: &Connection,
    peer_id: &str,
    now: i64,
) -> Result<Vec<OutboxEntry>, StorageError> {
    let mut statement = connection.prepare(
        "SELECT o.message_id, o.attempts, o.next_try, o.expires_at
         FROM outbox o
         JOIN messages m ON m.id = o.message_id
         JOIN conversations c ON c.id = m.conversation_id
         WHERE c.peer_id = ?1 AND o.next_try <= ?2 AND o.expires_at > ?2
         ORDER BY m.seq
         LIMIT ?3",
    )?;
    let rows = statement.query_map(params![peer_id, now, i64::from(OUTBOX_BATCH)], |row| {
        let attempts: i64 = row.get(1)?;
        Ok(OutboxEntry {
            message_id: row.get(0)?,
            attempts: u32::try_from(attempts).unwrap_or_default(),
            next_try: row.get(2)?,
            expires_at: row.get(3)?,
        })
    })?;
    let mut entries = Vec::new();
    for row in rows {
        entries.push(row?);
    }
    Ok(entries)
}

pub fn defer(
    connection: &Connection,
    message_id: &str,
    next_try: i64,
) -> Result<bool, StorageError> {
    let updated = connection.execute(
        "UPDATE outbox SET attempts = attempts + 1, next_try = ?2 WHERE message_id = ?1",
        params![message_id, next_try],
    )?;
    Ok(updated > 0)
}

pub fn remove(connection: &Connection, message_id: &str) -> Result<bool, StorageError> {
    let deleted = connection.execute(
        "DELETE FROM outbox WHERE message_id = ?1",
        params![message_id],
    )?;
    Ok(deleted > 0)
}

pub fn expire(connection: &Connection, now: i64) -> Result<Vec<String>, StorageError> {
    let transaction = connection.unchecked_transaction()?;
    let mut statement =
        transaction.prepare("SELECT message_id FROM outbox WHERE expires_at <= ?1 LIMIT ?2")?;
    let rows = statement.query_map(params![now, i64::from(OUTBOX_BATCH)], |row| {
        row.get::<_, String>(0)
    })?;
    let mut expired = Vec::new();
    for row in rows {
        expired.push(row?);
    }
    drop(statement);

    for message_id in &expired {
        transaction.execute(
            "DELETE FROM outbox WHERE message_id = ?1",
            params![message_id],
        )?;
        super::messages::set_state(
            &transaction,
            message_id,
            super::messages::MessageState::Expired,
        )?;
    }
    transaction.commit()?;
    Ok(expired)
}
