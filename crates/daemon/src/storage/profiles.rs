use rusqlite::{Connection, OptionalExtension, params};

use super::StorageError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PeerProfile {
    pub fingerprint: String,
    pub node_id: Option<String>,
    pub display_name: String,
    pub avatar_hash: Option<String>,
    pub avatar_mime: Option<String>,
    pub avatar: Vec<u8>,
    pub updated_at: i64,
}

pub fn upsert(connection: &Connection, profile: &PeerProfile) -> Result<(), StorageError> {
    connection.execute(
        "INSERT INTO peer_profiles
         (fingerprint, node_id, display_name, avatar_hash, avatar_mime, avatar, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
         ON CONFLICT(fingerprint) DO UPDATE SET
           node_id = excluded.node_id,
           display_name = excluded.display_name,
           avatar_hash = excluded.avatar_hash,
           avatar_mime = excluded.avatar_mime,
           avatar = excluded.avatar,
           updated_at = excluded.updated_at",
        params![
            profile.fingerprint,
            profile.node_id,
            profile.display_name,
            profile.avatar_hash,
            profile.avatar_mime,
            profile.avatar,
            profile.updated_at,
        ],
    )?;
    Ok(())
}

pub fn get(
    connection: &Connection,
    fingerprint: &str,
) -> Result<Option<PeerProfile>, StorageError> {
    connection
        .query_row(
            "SELECT fingerprint, node_id, display_name, avatar_hash, avatar_mime, avatar,
                    updated_at
             FROM peer_profiles WHERE fingerprint = ?1",
            params![fingerprint],
            |row| {
                Ok(PeerProfile {
                    fingerprint: row.get(0)?,
                    node_id: row.get(1)?,
                    display_name: row.get(2)?,
                    avatar_hash: row.get(3)?,
                    avatar_mime: row.get(4)?,
                    avatar: row.get(5)?,
                    updated_at: row.get(6)?,
                })
            },
        )
        .optional()
        .map_err(Into::into)
}
