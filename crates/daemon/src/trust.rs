use std::collections::HashSet;
use std::sync::{PoisonError, RwLock};

use thiserror::Error;

use crate::storage::{BlockedUser, DatabaseHandle, NewUser, StorageError, User};

#[derive(Debug, Error)]
pub enum TrustError {
    #[error(transparent)]
    Storage(#[from] StorageError),

    #[error("trusted user was not found")]
    NotFound,

    #[error("user is blocked")]
    Blocked,
}

pub struct TrustStore {
    database: DatabaseHandle,
    fingerprints: RwLock<HashSet<String>>,
    blocked: RwLock<HashSet<String>>,
}

impl TrustStore {
    pub async fn load(database: DatabaseHandle) -> Result<Self, TrustError> {
        let fingerprints = database
            .list_users()
            .await?
            .into_iter()
            .map(|user| user.fingerprint)
            .collect();
        let blocked = database
            .list_blocked_users()
            .await?
            .into_iter()
            .map(|user| user.fingerprint)
            .collect();
        Ok(Self {
            database,
            fingerprints: RwLock::new(fingerprints),
            blocked: RwLock::new(blocked),
        })
    }

    pub async fn commit(&self, user: NewUser) -> Result<User, TrustError> {
        self.commit_inner(user, true).await
    }

    pub async fn commit_auto(&self, user: NewUser) -> Result<User, TrustError> {
        self.commit_inner(user, false).await
    }

    async fn commit_inner(&self, user: NewUser, clear_block: bool) -> Result<User, TrustError> {
        let fingerprint = user.fingerprint.clone();
        if !clear_block && self.is_blocked(&fingerprint) {
            return Err(TrustError::Blocked);
        }
        let user = self.database.insert_user(user).await?;
        self.fingerprints
            .write()
            .unwrap_or_else(PoisonError::into_inner)
            .insert(user.fingerprint.clone());
        if clear_block {
            self.unblock(&fingerprint).await?;
        }
        Ok(user)
    }

    pub async fn forget(&self, user_id: &str) -> Result<User, TrustError> {
        let user = self
            .database
            .get_user(user_id)
            .await?
            .ok_or(TrustError::NotFound)?;
        self.block(&user).await?;
        self.database.delete_user(user_id).await?;
        self.fingerprints
            .write()
            .unwrap_or_else(PoisonError::into_inner)
            .remove(&user.fingerprint);
        Ok(user)
    }

    pub async fn block(&self, user: &User) -> Result<(), TrustError> {
        self.database
            .block_user(BlockedUser {
                fingerprint: user.fingerprint.clone(),
                node_id: Some(user.id.clone()),
                display_name: user.display_name.clone(),
                blocked_at: now_unix(),
            })
            .await?;
        self.blocked
            .write()
            .unwrap_or_else(PoisonError::into_inner)
            .insert(user.fingerprint.clone());
        Ok(())
    }

    pub async fn unblock(&self, fingerprint: &str) -> Result<bool, TrustError> {
        let removed = self.database.unblock_user(fingerprint).await?;
        self.blocked
            .write()
            .unwrap_or_else(PoisonError::into_inner)
            .remove(fingerprint);
        Ok(removed)
    }

    pub async fn blocked_users(&self) -> Result<Vec<BlockedUser>, TrustError> {
        self.database.list_blocked_users().await.map_err(Into::into)
    }

    pub fn is_blocked(&self, fingerprint: &str) -> bool {
        self.blocked
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .contains(fingerprint)
    }

    pub async fn list(&self) -> Result<Vec<User>, TrustError> {
        self.database.list_users().await.map_err(Into::into)
    }

    pub async fn user(&self, user_id: &str) -> Result<Option<User>, TrustError> {
        self.database.get_user(user_id).await.map_err(Into::into)
    }

    pub async fn user_by_fingerprint(&self, fingerprint: &str) -> Result<Option<User>, TrustError> {
        self.database
            .get_user_by_fingerprint(fingerprint)
            .await
            .map_err(Into::into)
    }

    pub fn is_trusted(&self, fingerprint: &str) -> bool {
        self.fingerprints
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .contains(fingerprint)
    }
}

fn now_unix() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |duration| {
            i64::try_from(duration.as_secs()).unwrap_or(i64::MAX)
        })
}

impl ticonnect_net::transport::TrustLookup for TrustStore {
    fn is_trusted(&self, fingerprint: &str) -> bool {
        Self::is_trusted(self, fingerprint)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::UserPolicy;

    #[tokio::test]
    async fn loads_commits_forgets_and_reports_trust() {
        let dir = std::env::temp_dir().join(format!(
            "ticonnect-trust-{}-{}",
            std::process::id(),
            unique_id()
        ));
        let database = DatabaseHandle::spawn(dir.join("ticonnect.db"))
            .await
            .expect("database");
        let store = TrustStore::load(database).await.expect("trust store");
        assert!(!store.is_trusted("fingerprint-1"));

        store
            .commit(NewUser {
                id: "user-1".to_owned(),
                fingerprint: "fingerprint-1".to_owned(),
                public_key: None,
                display_name: Some("User".to_owned()),
                policy: UserPolicy::Accept,
                trusted_at: 1,
                last_seen: None,
            })
            .await
            .expect("commit");
        assert!(store.is_trusted("fingerprint-1"));
        assert_eq!(store.list().await.expect("list").len(), 1);

        store.forget("user-1").await.expect("forget");
        assert!(!store.is_trusted("fingerprint-1"));
        assert!(store.is_blocked("fingerprint-1"));
        assert!(matches!(
            store.forget("user-1").await,
            Err(TrustError::NotFound)
        ));
        assert!(store.unblock("fingerprint-1").await.expect("unblock"));
        assert!(!store.is_blocked("fingerprint-1"));

        let _ = std::fs::remove_dir_all(dir);
    }

    fn unique_id() -> u64 {
        use std::sync::atomic::{AtomicU64, Ordering};
        static NEXT: AtomicU64 = AtomicU64::new(0);
        NEXT.fetch_add(1, Ordering::Relaxed)
    }
}
