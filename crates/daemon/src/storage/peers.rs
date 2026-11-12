use std::str::FromStr;

use rusqlite::{Connection, ErrorCode, OptionalExtension, params};

use super::StorageError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PeerPolicy {
    Accept,
    Mute,
    Ignore,
}

impl PeerPolicy {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Accept => "accept",
            Self::Mute => "mute",
            Self::Ignore => "ignore",
        }
    }
}

impl FromStr for PeerPolicy {
    type Err = StorageError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "accept" => Ok(Self::Accept),
            "mute" => Ok(Self::Mute),
            "ignore" => Ok(Self::Ignore),
            other => Err(StorageError::InvalidPolicy(other.to_owned())),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Peer {
    pub id: String,
    pub fingerprint: String,
    pub public_key: Option<[u8; 32]>,
    pub display_name: Option<String>,
    pub policy: PeerPolicy,
    pub trusted_at: i64,
    pub last_seen: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewPeer {
    pub id: String,
    pub fingerprint: String,
    pub public_key: Option<[u8; 32]>,
    pub display_name: Option<String>,
    pub policy: PeerPolicy,
    pub trusted_at: i64,
    pub last_seen: Option<i64>,
}

pub fn insert(connection: &Connection, peer: &NewPeer) -> Result<Peer, StorageError> {
    let result = connection.execute(
        "INSERT INTO peers (id, fingerprint, public_key, display_name, policy, trusted_at, last_seen)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![
            peer.id,
            peer.fingerprint,
            peer.public_key.as_ref().map(|key| key.as_slice()),
            peer.display_name,
            peer.policy.as_str(),
            peer.trusted_at,
            peer.last_seen,
        ],
    );

    match result {
        Ok(_) => Ok(Peer {
            id: peer.id.clone(),
            fingerprint: peer.fingerprint.clone(),
            public_key: peer.public_key,
            display_name: peer.display_name.clone(),
            policy: peer.policy,
            trusted_at: peer.trusted_at,
            last_seen: peer.last_seen,
        }),
        Err(error @ rusqlite::Error::SqliteFailure(code, _))
            if code.code == ErrorCode::ConstraintViolation =>
        {
            let exists: bool = connection
                .query_row(
                    "SELECT EXISTS(SELECT 1 FROM peers WHERE fingerprint = ?1)",
                    params![peer.fingerprint],
                    |row| row.get(0),
                )
                .unwrap_or(false);
            if exists {
                Err(StorageError::DuplicateFingerprint(peer.fingerprint.clone()))
            } else {
                Err(error.into())
            }
        }
        Err(error) => Err(error.into()),
    }
}

pub fn get_by_id(connection: &Connection, id: &str) -> Result<Option<Peer>, StorageError> {
    connection
        .query_row(
            "SELECT id, fingerprint, public_key, display_name, policy, trusted_at, last_seen
             FROM peers WHERE id = ?1",
            params![id],
            row_to_peer,
        )
        .optional()
        .map_err(Into::into)
}

pub fn get_by_fingerprint(
    connection: &Connection,
    fingerprint: &str,
) -> Result<Option<Peer>, StorageError> {
    connection
        .query_row(
            "SELECT id, fingerprint, public_key, display_name, policy, trusted_at, last_seen
             FROM peers WHERE fingerprint = ?1",
            params![fingerprint],
            row_to_peer,
        )
        .optional()
        .map_err(Into::into)
}

pub fn list(connection: &Connection) -> Result<Vec<Peer>, StorageError> {
    let mut statement = connection.prepare(
        "SELECT id, fingerprint, public_key, display_name, policy, trusted_at, last_seen
         FROM peers ORDER BY trusted_at, id",
    )?;
    let rows = statement.query_map([], row_to_peer)?;
    let mut peers = Vec::new();
    for row in rows {
        peers.push(row?);
    }
    Ok(peers)
}

pub fn update_policy(
    connection: &Connection,
    id: &str,
    policy: PeerPolicy,
) -> Result<bool, StorageError> {
    let updated = connection.execute(
        "UPDATE peers SET policy = ?2 WHERE id = ?1",
        params![id, policy.as_str()],
    )?;
    Ok(updated > 0)
}

pub fn update_last_seen(
    connection: &Connection,
    id: &str,
    last_seen: i64,
) -> Result<bool, StorageError> {
    let updated = connection.execute(
        "UPDATE peers SET last_seen = ?2 WHERE id = ?1",
        params![id, last_seen],
    )?;
    Ok(updated > 0)
}

pub fn delete(connection: &Connection, id: &str) -> Result<bool, StorageError> {
    let transaction = connection.unchecked_transaction()?;
    transaction.execute(
        "DELETE FROM outbox WHERE message_id IN (
            SELECT m.id
            FROM messages m
            JOIN conversations c ON c.id = m.conversation_id
            WHERE c.peer_id = ?1
        )",
        params![id],
    )?;
    let deleted = transaction.execute("DELETE FROM peers WHERE id = ?1", params![id])?;
    transaction.commit()?;
    Ok(deleted > 0)
}

pub fn update_public_key(
    connection: &Connection,
    id: &str,
    public_key: &[u8; 32],
) -> Result<bool, StorageError> {
    let updated = connection.execute(
        "UPDATE peers SET public_key = ?2 WHERE id = ?1 AND public_key IS NULL",
        params![id, public_key.as_slice()],
    )?;
    Ok(updated > 0)
}

pub fn count(connection: &Connection) -> Result<u32, StorageError> {
    let count: i64 = connection.query_row("SELECT COUNT(*) FROM peers", [], |row| row.get(0))?;
    u32::try_from(count).map_err(|error| StorageError::Integrity(error.to_string()))
}

pub fn outbox_depth(connection: &Connection) -> Result<u32, StorageError> {
    let count: i64 = connection.query_row("SELECT COUNT(*) FROM outbox", [], |row| row.get(0))?;
    u32::try_from(count).map_err(|error| StorageError::Integrity(error.to_string()))
}

fn row_to_peer(row: &rusqlite::Row<'_>) -> rusqlite::Result<Peer> {
    let policy: String = row.get(4)?;
    let policy = policy
        .parse()
        .map_err(|error| rusqlite::Error::ToSqlConversionFailure(Box::new(error)))?;
    let public_key: Option<Vec<u8>> = row.get(2)?;
    let public_key = public_key
        .map(|key| {
            key.try_into().map_err(|_| {
                rusqlite::Error::InvalidColumnType(
                    2,
                    "public_key".to_owned(),
                    rusqlite::types::Type::Blob,
                )
            })
        })
        .transpose()?;
    Ok(Peer {
        id: row.get(0)?,
        fingerprint: row.get(1)?,
        public_key,
        display_name: row.get(3)?,
        policy,
        trusted_at: row.get(5)?,
        last_seen: row.get(6)?,
    })
}
