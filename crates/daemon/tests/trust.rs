use std::fs;
use std::path::PathBuf;

use ticonnect_daemon::storage::{DatabaseHandle, NewPeer, PeerPolicy};
use ticonnect_daemon::trust::{TrustError, TrustStore};
use ticonnect_net::transport::TrustLookup;

#[tokio::test]
async fn trust_store_loads_commits_forgets_and_reports_trust() {
    let dir = temp_dir();
    let database = DatabaseHandle::spawn(dir.join("ticonnect.db"))
        .await
        .expect("database");
    let store = TrustStore::load(database).await.expect("trust store");

    assert!(!TrustLookup::is_trusted(&store, "fingerprint-1"));
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

    assert!(TrustLookup::is_trusted(&store, "fingerprint-1"));
    assert_eq!(store.list().await.expect("list").len(), 1);

    store.forget("peer-1").await.expect("forget");
    assert!(!TrustLookup::is_trusted(&store, "fingerprint-1"));
    assert!(matches!(
        store.forget("peer-1").await,
        Err(TrustError::NotFound)
    ));

    let _ = fs::remove_dir_all(dir);
}

#[tokio::test]
async fn trust_store_reloads_existing_peers() {
    let dir = temp_dir();
    let database = DatabaseHandle::spawn(dir.join("ticonnect.db"))
        .await
        .expect("database");
    database
        .insert_peer(NewPeer {
            id: "peer-1".to_owned(),
            fingerprint: "fingerprint-1".to_owned(),
            display_name: None,
            policy: PeerPolicy::Accept,
            trusted_at: 1,
            last_seen: None,
        })
        .await
        .expect("insert");

    let store = TrustStore::load(database).await.expect("trust store");
    assert!(TrustLookup::is_trusted(&store, "fingerprint-1"));

    let _ = fs::remove_dir_all(dir);
}

fn temp_dir() -> PathBuf {
    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT: AtomicU64 = AtomicU64::new(0);
    let id = NEXT.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!("ticonnect-trust-{}-{id}", std::process::id()))
}
