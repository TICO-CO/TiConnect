use std::collections::HashSet;
use std::sync::{PoisonError, RwLock};

use thiserror::Error;

use crate::storage::{DatabaseHandle, NewPeer, Peer, StorageError};

#[derive(Debug, Error)]
pub enum TrustError {
    #[error(transparent)]
    Storage(#[from] StorageError),

    #[error("trusted peer was not found")]
    NotFound,
}

pub struct TrustStore {
    database: DatabaseHandle,
    fingerprints: RwLock<HashSet<String>>,
}

impl TrustStore {
    pub async fn load(database: DatabaseHandle) -> Result<Self, TrustError> {
        let fingerprints = database
            .list_peers()
            .await?
            .into_iter()
            .map(|peer| peer.fingerprint)
            .collect();
        Ok(Self {
            database,
            fingerprints: RwLock::new(fingerprints),
        })
    }

    pub async fn commit(&self, peer: NewPeer) -> Result<Peer, TrustError> {
        let peer = self.database.insert_peer(peer).await?;
        self.fingerprints
            .write()
            .unwrap_or_else(PoisonError::into_inner)
            .insert(peer.fingerprint.clone());
        Ok(peer)
    }

    pub async fn forget(&self, peer_id: &str) -> Result<Peer, TrustError> {
        let peer = self
            .database
            .get_peer(peer_id)
            .await?
            .ok_or(TrustError::NotFound)?;
        self.database.delete_peer(peer_id).await?;
        self.fingerprints
            .write()
            .unwrap_or_else(PoisonError::into_inner)
            .remove(&peer.fingerprint);
        Ok(peer)
    }

    pub async fn list(&self) -> Result<Vec<Peer>, TrustError> {
        self.database.list_peers().await.map_err(Into::into)
    }

    pub async fn peer(&self, peer_id: &str) -> Result<Option<Peer>, TrustError> {
        self.database.get_peer(peer_id).await.map_err(Into::into)
    }

    pub async fn peer_by_fingerprint(&self, fingerprint: &str) -> Result<Option<Peer>, TrustError> {
        self.database
            .get_peer_by_fingerprint(fingerprint)
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

impl ticonnect_net::transport::TrustLookup for TrustStore {
    fn is_trusted(&self, fingerprint: &str) -> bool {
        Self::is_trusted(self, fingerprint)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::PeerPolicy;

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
            .commit(NewPeer {
                id: "peer-1".to_owned(),
                fingerprint: "fingerprint-1".to_owned(),
                display_name: Some("Peer".to_owned()),
                policy: PeerPolicy::Accept,
                trusted_at: 1,
                last_seen: None,
            })
            .await
            .expect("commit");
        assert!(store.is_trusted("fingerprint-1"));
        assert_eq!(store.list().await.expect("list").len(), 1);

        store.forget("peer-1").await.expect("forget");
        assert!(!store.is_trusted("fingerprint-1"));
        assert!(matches!(
            store.forget("peer-1").await,
            Err(TrustError::NotFound)
        ));

        let _ = std::fs::remove_dir_all(dir);
    }

    fn unique_id() -> u64 {
        use std::sync::atomic::{AtomicU64, Ordering};
        static NEXT: AtomicU64 = AtomicU64::new(0);
        NEXT.fetch_add(1, Ordering::Relaxed)
    }
}
