use std::str::FromStr;

use rusqlite::{Connection, ErrorCode, OptionalExtension, params};

use super::StorageError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UserPolicy {
    Accept,
    Mute,
    Ignore,
}

impl UserPolicy {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Accept => "accept",
            Self::Mute => "mute",
            Self::Ignore => "ignore",
        }
    }
}

impl FromStr for UserPolicy {
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
pub struct User {
    pub id: String,
    pub fingerprint: String,
    pub public_key: Option<[u8; 32]>,
    pub display_name: Option<String>,
    pub policy: UserPolicy,
    pub trusted_at: i64,
    pub last_seen: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewUser {
    pub id: String,
    pub fingerprint: String,
    pub public_key: Option<[u8; 32]>,
    pub display_name: Option<String>,
    pub policy: UserPolicy,
    pub trusted_at: i64,
    pub last_seen: Option<i64>,
}

pub fn insert(connection: &Connection, user: &NewUser) -> Result<User, StorageError> {
    let result = connection.execute(
        "INSERT INTO users (id, fingerprint, public_key, display_name, policy, trusted_at, last_seen)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![
            user.id,
            user.fingerprint,
            user.public_key.as_ref().map(|key| key.as_slice()),
            user.display_name,
            user.policy.as_str(),
            user.trusted_at,
            user.last_seen,
        ],
    );

    match result {
        Ok(_) => Ok(User {
            id: user.id.clone(),
            fingerprint: user.fingerprint.clone(),
            public_key: user.public_key,
            display_name: user.display_name.clone(),
            policy: user.policy,
            trusted_at: user.trusted_at,
            last_seen: user.last_seen,
        }),
        Err(error @ rusqlite::Error::SqliteFailure(code, _))
            if code.code == ErrorCode::ConstraintViolation =>
        {
            let exists: bool = connection
                .query_row(
                    "SELECT EXISTS(SELECT 1 FROM users WHERE fingerprint = ?1)",
                    params![user.fingerprint],
                    |row| row.get(0),
                )
                .unwrap_or(false);
            if exists {
                Err(StorageError::DuplicateFingerprint(user.fingerprint.clone()))
            } else {
                Err(error.into())
            }
        }
        Err(error) => Err(error.into()),
    }
}

pub fn get_by_id(connection: &Connection, id: &str) -> Result<Option<User>, StorageError> {
    connection
        .query_row(
            "SELECT id, fingerprint, public_key, display_name, policy, trusted_at, last_seen
             FROM users WHERE id = ?1",
            params![id],
            row_to_user,
        )
        .optional()
        .map_err(Into::into)
}

pub fn get_by_fingerprint(
    connection: &Connection,
    fingerprint: &str,
) -> Result<Option<User>, StorageError> {
    connection
        .query_row(
            "SELECT id, fingerprint, public_key, display_name, policy, trusted_at, last_seen
             FROM users WHERE fingerprint = ?1",
            params![fingerprint],
            row_to_user,
        )
        .optional()
        .map_err(Into::into)
}

pub fn list(connection: &Connection) -> Result<Vec<User>, StorageError> {
    let mut statement = connection.prepare(
        "SELECT id, fingerprint, public_key, display_name, policy, trusted_at, last_seen
         FROM users ORDER BY trusted_at, id",
    )?;
    let rows = statement.query_map([], row_to_user)?;
    let mut users = Vec::new();
    for row in rows {
        users.push(row?);
    }
    Ok(users)
}

pub fn update_policy(
    connection: &Connection,
    id: &str,
    policy: UserPolicy,
) -> Result<bool, StorageError> {
    let updated = connection.execute(
        "UPDATE users SET policy = ?2 WHERE id = ?1",
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
        "UPDATE users SET last_seen = ?2 WHERE id = ?1",
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
            WHERE c.user_id = ?1
        )",
        params![id],
    )?;
    let deleted = transaction.execute("DELETE FROM users WHERE id = ?1", params![id])?;
    transaction.commit()?;
    Ok(deleted > 0)
}

pub fn update_public_key(
    connection: &Connection,
    id: &str,
    public_key: &[u8; 32],
) -> Result<bool, StorageError> {
    let updated = connection.execute(
        "UPDATE users SET public_key = ?2 WHERE id = ?1 AND public_key IS NULL",
        params![id, public_key.as_slice()],
    )?;
    Ok(updated > 0)
}

pub fn count(connection: &Connection) -> Result<u32, StorageError> {
    let count: i64 = connection.query_row("SELECT COUNT(*) FROM users", [], |row| row.get(0))?;
    u32::try_from(count).map_err(|error| StorageError::Integrity(error.to_string()))
}

pub fn outbox_depth(connection: &Connection) -> Result<u32, StorageError> {
    let count: i64 = connection.query_row("SELECT COUNT(*) FROM outbox", [], |row| row.get(0))?;
    u32::try_from(count).map_err(|error| StorageError::Integrity(error.to_string()))
}

fn row_to_user(row: &rusqlite::Row<'_>) -> rusqlite::Result<User> {
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
    Ok(User {
        id: row.get(0)?,
        fingerprint: row.get(1)?,
        public_key,
        display_name: row.get(3)?,
        policy,
        trusted_at: row.get(5)?,
        last_seen: row.get(6)?,
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlockedUser {
    pub fingerprint: String,
    pub node_id: Option<String>,
    pub display_name: Option<String>,
    pub blocked_at: i64,
}

pub fn block(connection: &Connection, user: &BlockedUser) -> Result<(), StorageError> {
    connection.execute(
        "INSERT INTO blocked_users (fingerprint, node_id, display_name, blocked_at)
         VALUES (?1, ?2, ?3, ?4)
         ON CONFLICT(fingerprint) DO UPDATE SET
            node_id = excluded.node_id,
            display_name = excluded.display_name,
            blocked_at = excluded.blocked_at",
        params![
            user.fingerprint,
            user.node_id,
            user.display_name,
            user.blocked_at
        ],
    )?;
    Ok(())
}

pub fn unblock(connection: &Connection, fingerprint: &str) -> Result<bool, StorageError> {
    let deleted = connection.execute(
        "DELETE FROM blocked_users WHERE fingerprint = ?1",
        params![fingerprint],
    )?;
    Ok(deleted > 0)
}

pub fn list_blocked(connection: &Connection) -> Result<Vec<BlockedUser>, StorageError> {
    let mut statement = connection.prepare(
        "SELECT fingerprint, node_id, display_name, blocked_at
         FROM blocked_users ORDER BY blocked_at DESC, fingerprint",
    )?;
    let rows = statement.query_map([], row_to_blocked)?;
    let mut users = Vec::new();
    for row in rows {
        users.push(row?);
    }
    Ok(users)
}

pub fn is_blocked(connection: &Connection, fingerprint: &str) -> Result<bool, StorageError> {
    let exists: bool = connection.query_row(
        "SELECT EXISTS(SELECT 1 FROM blocked_users WHERE fingerprint = ?1)",
        params![fingerprint],
        |row| row.get(0),
    )?;
    Ok(exists)
}

fn row_to_blocked(row: &rusqlite::Row<'_>) -> rusqlite::Result<BlockedUser> {
    Ok(BlockedUser {
        fingerprint: row.get(0)?,
        node_id: row.get(1)?,
        display_name: row.get(2)?,
        blocked_at: row.get(3)?,
    })
}
