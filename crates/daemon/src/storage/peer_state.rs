use rusqlite::{Connection, OptionalExtension, params};

use super::StorageError;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PeerState {
    pub last_sent_seq: u64,
    pub last_recv_seq: u64,
}

pub fn get(connection: &Connection, peer_id: &str) -> Result<PeerState, StorageError> {
    let state = connection
        .query_row(
            "SELECT last_sent_seq, last_recv_seq FROM peer_state WHERE peer_id = ?1",
            params![peer_id],
            |row| {
                let sent: i64 = row.get(0)?;
                let recv: i64 = row.get(1)?;
                Ok(PeerState {
                    last_sent_seq: u64::try_from(sent).unwrap_or_default(),
                    last_recv_seq: u64::try_from(recv).unwrap_or_default(),
                })
            },
        )
        .optional()?;
    Ok(state.unwrap_or_default())
}

pub fn ensure(connection: &Connection, peer_id: &str) -> Result<(), StorageError> {
    connection.execute(
        "INSERT OR IGNORE INTO peer_state (peer_id, last_sent_seq, last_recv_seq)
         VALUES (?1, 0, 0)",
        params![peer_id],
    )?;
    Ok(())
}

pub fn allocate_seq(connection: &Connection, peer_id: &str) -> Result<u64, StorageError> {
    let transaction = connection.unchecked_transaction()?;
    ensure(&transaction, peer_id)?;
    let current: i64 = transaction.query_row(
        "SELECT last_sent_seq FROM peer_state WHERE peer_id = ?1",
        params![peer_id],
        |row| row.get(0),
    )?;
    let next = current
        .checked_add(1)
        .ok_or_else(|| StorageError::InvalidData("sequence overflow".to_owned()))?;
    transaction.execute(
        "UPDATE peer_state SET last_sent_seq = ?2 WHERE peer_id = ?1",
        params![peer_id, next],
    )?;
    transaction.commit()?;
    u64::try_from(next).map_err(|_| StorageError::InvalidData("sequence overflow".to_owned()))
}

pub fn advance_recv(connection: &Connection, peer_id: &str, seq: u64) -> Result<(), StorageError> {
    ensure(connection, peer_id)?;
    connection.execute(
        "UPDATE peer_state
         SET last_recv_seq = MAX(last_recv_seq, ?2)
         WHERE peer_id = ?1",
        params![
            peer_id,
            i64::try_from(seq)
                .map_err(|_| StorageError::InvalidData("sequence overflow".to_owned()))?
        ],
    )?;
    Ok(())
}
