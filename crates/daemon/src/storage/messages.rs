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
    peer_id: &str,
) -> Result<InsertOutcome, StorageError> {
    let transaction = connection.unchecked_transaction()?;
    let outcome = insert(&transaction, record, received_at, signature, None)?;
    if outcome == InsertOutcome::Inserted {
        super::peer_state::advance_recv(&transaction, peer_id, record.seq)?;
    }
    transaction.commit()?;
    Ok(outcome)
}

pub fn get(connection: &Connection, id: &str) -> Result<Option<StoredMessage>, StorageError> {
    connection
        .query_row(
            "SELECT id, conversation_id, sender_id, seq, sent_at, received_at, kind, target_id,
                    body, attachment_hash, attachment_name, attachment_size, attachment_mime,
                    signature, state
             FROM messages WHERE id = ?1",
            params![id],
            row_to_message,
        )
        .optional()
        .map_err(Into::into)
}

pub fn count(connection: &Connection, conversation_id: &str) -> Result<u32, StorageError> {
    let count: i64 = connection.query_row(
        "SELECT COUNT(*) FROM messages WHERE conversation_id = ?1 AND kind != 'ack'",
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
                signature, state
         FROM messages
         WHERE conversation_id = ?1 AND kind != 'ack'
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
                signature, state
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
    })
}

fn to_sql_error(error: impl std::error::Error + Send + Sync + 'static) -> rusqlite::Error {
    rusqlite::Error::ToSqlConversionFailure(Box::new(error))
}
