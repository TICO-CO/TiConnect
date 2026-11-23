use std::collections::HashSet;
use std::str::FromStr;

use rusqlite::{Connection, OptionalExtension, params};
use ticonnect_core::id::MessageId;
use ticonnect_core::message::{MessageKind, MessageRecord};
use ticonnect_core::sign::SignedEnvelope;

use super::StorageError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MessageState {
    Queued,
    Sent,
    Delivered,
    Read,
    Failed,
    Expired,
}

impl MessageState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Queued => "queued",
            Self::Sent => "sent",
            Self::Delivered => "delivered",
            Self::Read => "read",
            Self::Failed => "failed",
            Self::Expired => "expired",
        }
    }
}

impl FromStr for MessageState {
    type Err = StorageError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "queued" => Ok(Self::Queued),
            "sent" => Ok(Self::Sent),
            "delivered" => Ok(Self::Delivered),
            "read" => Ok(Self::Read),
            "failed" => Ok(Self::Failed),
            "expired" => Ok(Self::Expired),
            other => Err(StorageError::InvalidMessageState(other.to_owned())),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredMessage {
    pub record: MessageRecord,
    pub received_at: i64,
    pub signature: Vec<u8>,
    pub state: Option<MessageState>,
    pub edited_at: Option<i64>,
    pub deleted_at: Option<i64>,
}

impl StoredMessage {
    pub fn signed_bytes(&self) -> Result<Vec<u8>, StorageError> {
        let payload = self
            .record
            .canonical_bytes()
            .map_err(|error| StorageError::InvalidData(error.to_string()))?;
        let signature: [u8; 64] = self.signature.as_slice().try_into().map_err(|_| {
            StorageError::InvalidData("message signature has invalid length".to_owned())
        })?;
        SignedEnvelope::from_parts(&payload, signature)
            .map(|envelope| envelope.encode())
            .map_err(|error| StorageError::InvalidData(error.to_string()))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InsertOutcome {
    Inserted,
    Duplicate,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConversationSummary {
    pub conversation_id: String,
    pub user_id: String,
    pub last_message_id: Option<String>,
    pub last_sender_id: Option<String>,
    pub last_body: Option<String>,
    pub last_deleted: bool,
    pub last_sent_at: Option<i64>,
    pub unread_count: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClearHistoryOutcome {
    pub messages: u32,
    pub attachments: Vec<String>,
}

pub fn clear_history(
    connection: &Connection,
    user_id: Option<&str>,
) -> Result<ClearHistoryOutcome, StorageError> {
    let transaction = connection.unchecked_transaction()?;
    let filter = if user_id.is_some() {
        "conversation_id IN (SELECT id FROM conversations WHERE user_id = ?1)"
    } else {
        "1 = 1"
    };

    let mut scoped = transaction.prepare(&format!(
        "SELECT id, attachment_hash FROM messages WHERE {filter}"
    ))?;
    let rows = if let Some(user_id) = user_id {
        scoped
            .query_map(params![user_id], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, Option<String>>(1)?))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?
    } else {
        scoped
            .query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, Option<String>>(1)?))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?
    };
    drop(scoped);
    let count = u32::try_from(rows.len())
        .map_err(|_| StorageError::InvalidData("message count overflow".to_owned()))?;
    let mut scoped_hashes: HashSet<String> = rows
        .into_iter()
        .filter_map(|(_, hash)| hash)
        .filter(|hash| !hash.is_empty())
        .collect();
    if user_id.is_none() {
        let mut attachments = transaction.prepare("SELECT hash FROM attachments")?;
        scoped_hashes.extend(
            attachments
                .query_map([], |row| row.get::<_, String>(0))?
                .collect::<rusqlite::Result<Vec<_>>>()?,
        );
    }

    let delete_outbox =
        format!("DELETE FROM outbox WHERE message_id IN (SELECT id FROM messages WHERE {filter})");
    let delete_messages = format!("DELETE FROM messages WHERE {filter}");
    if let Some(user_id) = user_id {
        transaction.execute(&delete_outbox, params![user_id])?;
        transaction.execute(&delete_messages, params![user_id])?;
    } else {
        transaction.execute(&delete_outbox, [])?;
        transaction.execute(&delete_messages, [])?;
    }

    let mut remaining_statement = transaction.prepare(
        "SELECT DISTINCT attachment_hash FROM messages WHERE attachment_hash IS NOT NULL",
    )?;
    let remaining: HashSet<String> = remaining_statement
        .query_map([], |row| row.get::<_, String>(0))?
        .collect::<rusqlite::Result<Vec<_>>>()?
        .into_iter()
        .collect();
    drop(remaining_statement);

    if user_id.is_none() {
        transaction.execute("DELETE FROM transfer_partials", [])?;
    }
    let mut attachments = Vec::new();
    for hash in scoped_hashes {
        if remaining.contains(&hash) {
            continue;
        }
        let removed =
            transaction.execute("DELETE FROM attachments WHERE hash = ?1", params![hash])?;
        transaction.execute(
            "DELETE FROM transfer_partials WHERE hash = ?1",
            params![hash],
        )?;
        if removed > 0 {
            attachments.push(hash);
        }
    }
    transaction.commit()?;
    Ok(ClearHistoryOutcome {
        messages: count,
        attachments,
    })
}

pub fn storage_usage(connection: &Connection) -> Result<StorageStats, StorageError> {
    let message_count: i64 = connection.query_row(
        "SELECT COUNT(*) FROM messages WHERE kind NOT IN ('ack', 'edit', 'delete')",
        [],
        |row| row.get(0),
    )?;
    let (attachment_count, attachment_bytes): (i64, i64) = connection.query_row(
        "SELECT COUNT(*), COALESCE(SUM(size), 0) FROM attachments",
        [],
        |row| Ok((row.get(0)?, row.get(1)?)),
    )?;
    let page_count: i64 = connection.query_row("PRAGMA page_count", [], |row| row.get(0))?;
    let page_size: i64 = connection.query_row("PRAGMA page_size", [], |row| row.get(0))?;
    let database_bytes = page_count.saturating_mul(page_size).max(0);
    Ok(StorageStats {
        message_count: u64::try_from(message_count).unwrap_or_default(),
        attachment_count: u64::try_from(attachment_count).unwrap_or_default(),
        attachment_bytes: u64::try_from(attachment_bytes).unwrap_or_default(),
        database_bytes: u64::try_from(database_bytes).unwrap_or_default(),
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StorageStats {
    pub message_count: u64,
    pub attachment_count: u64,
    pub attachment_bytes: u64,
    pub database_bytes: u64,
}

pub fn conversation_summaries(
    connection: &Connection,
    local_node_id: &str,
) -> Result<Vec<ConversationSummary>, StorageError> {
    let mut conversations = connection.prepare("SELECT id, user_id FROM conversations")?;
    let rows = conversations.query_map([], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
    })?;

    let mut summaries = Vec::new();
    for row in rows {
        let (conversation_id, user_id) = row?;
        let last = connection
            .query_row(
                "SELECT id, sender_id,
                        CASE WHEN deleted_at IS NOT NULL THEN NULL ELSE body END,
                        sent_at,
                        deleted_at IS NOT NULL
                 FROM messages
                 WHERE conversation_id = ?1 AND kind NOT IN ('ack', 'edit', 'delete')
                 ORDER BY CASE WHEN sent_at > received_at THEN received_at ELSE sent_at END DESC,
                          received_at DESC, id DESC
                 LIMIT 1",
                params![conversation_id],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, Option<String>>(2)?,
                        row.get::<_, i64>(3)?,
                        row.get::<_, bool>(4)?,
                    ))
                },
            )
            .optional()?;
        let unread: i64 = connection.query_row(
            "SELECT COUNT(*) FROM messages
             WHERE conversation_id = ?1 AND sender_id != ?2 AND kind NOT IN ('ack', 'edit', 'delete')
               AND read_at IS NULL",
            params![conversation_id, local_node_id],
            |row| row.get(0),
        )?;
        let unread_count = u32::try_from(unread).map_err(|_| {
            StorageError::InvalidData(format!("unread message count out of range: {unread}"))
        })?;
        summaries.push(ConversationSummary {
            conversation_id,
            user_id,
            last_message_id: last.as_ref().map(|item| item.0.clone()),
            last_sender_id: last.as_ref().map(|item| item.1.clone()),
            last_body: last.as_ref().and_then(|item| item.2.clone()),
            last_deleted: last.as_ref().is_some_and(|item| item.4),
            last_sent_at: last.as_ref().map(|item| item.3),
            unread_count,
        });
    }
    Ok(summaries)
}

pub fn mark_read(
    connection: &Connection,
    conversation_id: &str,
    message_ids: &[String],
    read_at: i64,
) -> Result<Vec<String>, StorageError> {
    if message_ids.is_empty() {
        return Ok(Vec::new());
    }
    let placeholders = vec!["?"; message_ids.len()].join(", ");
    let select = format!(
        "SELECT id FROM messages
         WHERE conversation_id = ? AND read_at IS NULL AND id IN ({placeholders})"
    );
    let mut select_values: Vec<Box<dyn rusqlite::ToSql>> =
        Vec::with_capacity(message_ids.len() + 1);
    select_values.push(Box::new(conversation_id.to_owned()));
    for message_id in message_ids {
        select_values.push(Box::new(message_id.clone()));
    }
    let mut statement = connection.prepare(&select)?;
    let unread = statement
        .query_map(
            rusqlite::params_from_iter(select_values.iter().map(|value| value.as_ref())),
            |row| row.get::<_, String>(0),
        )?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    mark_ids_read(connection, conversation_id, &unread, read_at)?;
    Ok(unread)
}

pub fn mark_conversation_read(
    connection: &Connection,
    conversation_id: &str,
    local_sender_id: &str,
    read_at: i64,
) -> Result<Vec<String>, StorageError> {
    let mut statement = connection.prepare(
        "SELECT id FROM messages
         WHERE conversation_id = ?1 AND sender_id != ?2 AND kind NOT IN ('ack', 'edit', 'delete')
           AND read_at IS NULL",
    )?;
    let unread = statement
        .query_map(params![conversation_id, local_sender_id], |row| {
            row.get::<_, String>(0)
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    connection.execute(
        "UPDATE messages SET read_at = ?1
         WHERE conversation_id = ?2 AND sender_id != ?3 AND kind NOT IN ('ack', 'edit', 'delete')
           AND read_at IS NULL",
        params![read_at, conversation_id, local_sender_id],
    )?;
    Ok(unread)
}

fn mark_ids_read(
    connection: &Connection,
    conversation_id: &str,
    message_ids: &[String],
    read_at: i64,
) -> Result<(), StorageError> {
    if message_ids.is_empty() {
        return Ok(());
    }

    let placeholders = vec!["?"; message_ids.len()].join(", ");
    let update = format!(
        "UPDATE messages SET read_at = ?
         WHERE conversation_id = ? AND read_at IS NULL AND id IN ({placeholders})"
    );
    let mut update_values: Vec<Box<dyn rusqlite::ToSql>> =
        Vec::with_capacity(message_ids.len() + 2);
    update_values.push(Box::new(read_at));
    update_values.push(Box::new(conversation_id.to_owned()));
    for message_id in message_ids {
        update_values.push(Box::new(message_id.clone()));
    }
    connection.execute(
        &update,
        rusqlite::params_from_iter(update_values.iter().map(|value| value.as_ref())),
    )?;
    Ok(())
}

pub fn insert(
    connection: &Connection,
    record: &MessageRecord,
    received_at: i64,
    signature: &[u8],
    state: Option<MessageState>,
) -> Result<InsertOutcome, StorageError> {
    record
        .validate()
        .map_err(|error| StorageError::InvalidData(error.to_string()))?;
    let inserted = connection.execute(
        "INSERT OR IGNORE INTO messages
         (id, conversation_id, sender_id, seq, sent_at, received_at, kind, target_id, body,
          attachment_hash, attachment_name, attachment_size, attachment_mime, signature, state)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)",
        params![
            record.message_id.to_string(),
            record.conversation_id,
            record.sender_id,
            i64::try_from(record.seq)
                .map_err(|_| StorageError::InvalidData("sequence overflow".to_owned()))?,
            record.sent_at,
            received_at,
            record.kind.as_str(),
            record.target_id,
            record.body,
            record.attachment_hash,
            record.attachment_name,
            record
                .attachment_size
                .map(i64::try_from)
                .transpose()
                .map_err(|_| {
                    StorageError::InvalidData("attachment size overflow".to_owned())
                })?,
            record.attachment_mime,
            signature,
            state.map(MessageState::as_str),
        ],
    )?;
    Ok(if inserted > 0 {
        InsertOutcome::Inserted
    } else {
        InsertOutcome::Duplicate
    })
}

pub fn insert_inbound(
    connection: &Connection,
    record: &MessageRecord,
    received_at: i64,
    signature: &[u8],
    user_id: &str,
) -> Result<InsertOutcome, StorageError> {
    let transaction = connection.unchecked_transaction()?;
    let outcome = insert(&transaction, record, received_at, signature, None)?;
    if outcome == InsertOutcome::Inserted {
        super::user_state::advance_recv(&transaction, user_id, record.seq)?;
    }
    transaction.commit()?;
    Ok(outcome)
}

pub fn get(connection: &Connection, id: &str) -> Result<Option<StoredMessage>, StorageError> {
    connection
        .query_row(
            "SELECT id, conversation_id, sender_id, seq, sent_at, received_at, kind, target_id,
                    body, attachment_hash, attachment_name, attachment_size, attachment_mime,
                    signature, state, edited_at, deleted_at
             FROM messages WHERE id = ?1",
            params![id],
            row_to_message,
        )
        .optional()
        .map_err(Into::into)
}

pub fn apply_lifecycle(
    connection: &Connection,
    record: &MessageRecord,
    received_at: i64,
    signature: &[u8],
    user_id: &str,
) -> Result<InsertOutcome, StorageError> {
    let transaction = connection.unchecked_transaction()?;
    let outcome = insert(&transaction, record, received_at, signature, None)?;
    if outcome == InsertOutcome::Inserted {
        super::user_state::advance_recv(&transaction, user_id, record.seq)?;
        apply_lifecycle_update(&transaction, record)?;
    }
    transaction.commit()?;
    Ok(outcome)
}

pub fn apply_lifecycle_update(
    connection: &Connection,
    record: &MessageRecord,
) -> Result<(), StorageError> {
    let target = record
        .target_id
        .clone()
        .ok_or_else(|| StorageError::InvalidData("lifecycle record has no target".to_owned()))?;
    match record.kind {
        MessageKind::Edit => {
            connection.execute(
                "UPDATE messages SET body = ?3, edited_at = ?4
                 WHERE id = ?1 AND conversation_id = ?2 AND sender_id = ?5
                   AND kind IN ('text', 'image', 'audio', 'file') AND deleted_at IS NULL",
                params![
                    target,
                    record.conversation_id,
                    record.body,
                    record.sent_at,
                    record.sender_id
                ],
            )?;
        }
        MessageKind::Delete => {
            connection.execute(
                "UPDATE messages SET deleted_at = ?3
                 WHERE id = ?1 AND conversation_id = ?2 AND sender_id = ?4
                   AND kind IN ('text', 'image', 'audio', 'file')",
                params![
                    target,
                    record.conversation_id,
                    record.sent_at,
                    record.sender_id
                ],
            )?;
        }
        _ => {}
    }
    Ok(())
}

pub fn count(connection: &Connection, conversation_id: &str) -> Result<u32, StorageError> {
    let count: i64 = connection.query_row(
        "SELECT COUNT(*) FROM messages WHERE conversation_id = ?1 AND kind NOT IN ('ack', 'edit', 'delete')",
        params![conversation_id],
        |row| row.get(0),
    )?;
    u32::try_from(count).map_err(|_| StorageError::InvalidData("message count overflow".to_owned()))
}

pub fn history(
    connection: &Connection,
    conversation_id: &str,
    limit: u32,
    offset: u32,
) -> Result<Vec<StoredMessage>, StorageError> {
    let mut statement = connection.prepare(
        "SELECT id, conversation_id, sender_id, seq, sent_at, received_at, kind, target_id,
                body, attachment_hash, attachment_name, attachment_size, attachment_mime,
                signature, state, edited_at, deleted_at
         FROM messages
         WHERE conversation_id = ?1 AND kind NOT IN ('ack', 'edit', 'delete')
         ORDER BY CASE WHEN sent_at > received_at THEN received_at ELSE sent_at END,
                  received_at, id
         LIMIT ?2 OFFSET ?3",
    )?;
    let rows = statement.query_map(
        params![conversation_id, i64::from(limit), i64::from(offset)],
        row_to_message,
    )?;
    let mut messages = Vec::new();
    for row in rows {
        messages.push(row?);
    }
    Ok(messages)
}

pub fn records_between(
    connection: &Connection,
    conversation_id: &str,
    sender_id: &str,
    from_seq: u64,
    to_seq: u64,
    limit: u32,
) -> Result<Vec<StoredMessage>, StorageError> {
    let mut statement = connection.prepare(
        "SELECT id, conversation_id, sender_id, seq, sent_at, received_at, kind, target_id,
                body, attachment_hash, attachment_name, attachment_size, attachment_mime,
                signature, state, edited_at, deleted_at
         FROM messages
         WHERE conversation_id = ?1 AND sender_id = ?2 AND seq >= ?3 AND seq <= ?4
           AND (state IS NULL OR state NOT IN ('expired', 'failed'))
         ORDER BY seq
         LIMIT ?5",
    )?;
    let rows = statement.query_map(
        params![
            conversation_id,
            sender_id,
            i64::try_from(from_seq)
                .map_err(|_| StorageError::InvalidData("sequence overflow".to_owned()))?,
            i64::try_from(to_seq)
                .map_err(|_| StorageError::InvalidData("sequence overflow".to_owned()))?,
            i64::from(limit),
        ],
        row_to_message,
    )?;
    let mut messages = Vec::new();
    for row in rows {
        messages.push(row?);
    }
    Ok(messages)
}

pub fn set_state(
    connection: &Connection,
    id: &str,
    state: MessageState,
) -> Result<bool, StorageError> {
    let updated = connection.execute(
        "UPDATE messages SET state = ?2
         WHERE id = ?1 AND state IS NOT NULL
           AND (CASE state
                 WHEN 'queued' THEN 1 WHEN 'sent' THEN 2 WHEN 'delivered' THEN 3
                 WHEN 'read' THEN 4 WHEN 'failed' THEN 99 WHEN 'expired' THEN 99
                 ELSE 0 END)
             < (CASE ?2
                 WHEN 'queued' THEN 1 WHEN 'sent' THEN 2 WHEN 'delivered' THEN 3
                 WHEN 'read' THEN 4 WHEN 'failed' THEN 99 WHEN 'expired' THEN 99
                 ELSE 0 END)",
        params![id, state.as_str()],
    )?;
    Ok(updated > 0)
}

pub fn state(connection: &Connection, id: &str) -> Result<Option<MessageState>, StorageError> {
    let value: Option<Option<String>> = connection
        .query_row(
            "SELECT state FROM messages WHERE id = ?1",
            params![id],
            |row| row.get(0),
        )
        .optional()?;
    value.flatten().map(|value| value.parse()).transpose()
}

fn row_to_message(row: &rusqlite::Row<'_>) -> rusqlite::Result<StoredMessage> {
    let id: String = row.get(0)?;
    let message_id = MessageId::from_str(&id).map_err(to_sql_error)?;
    let kind: String = row.get(6)?;
    let kind = MessageKind::from_str(&kind).map_err(to_sql_error)?;
    let seq: i64 = row.get(3)?;
    let state: Option<String> = row.get(14)?;
    let state = state
        .map(|value| value.parse().map_err(to_sql_error))
        .transpose()?;
    let record = MessageRecord {
        message_id,
        conversation_id: row.get(1)?,
        sender_id: row.get(2)?,
        seq: u64::try_from(seq).map_err(to_sql_error)?,
        sent_at: row.get(4)?,
        kind,
        target_id: row.get(7)?,
        body: row.get(8)?,
        attachment_hash: row.get(9)?,
        attachment_name: row.get(10)?,
        attachment_size: row
            .get::<_, Option<i64>>(11)?
            .map(u64::try_from)
            .transpose()
            .map_err(to_sql_error)?,
        attachment_mime: row.get(12)?,
    };
    Ok(StoredMessage {
        record,
        received_at: row.get(5)?,
        signature: row.get(13)?,
        state,
        edited_at: row.get(15)?,
        deleted_at: row.get(16)?,
    })
}

fn to_sql_error(error: impl std::error::Error + Send + Sync + 'static) -> rusqlite::Error {
    rusqlite::Error::ToSqlConversionFailure(Box::new(error))
}
