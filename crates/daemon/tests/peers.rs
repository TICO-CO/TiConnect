use std::fs;
use std::path::PathBuf;

use ticonnect_daemon::storage::{DatabaseHandle, NewPeer, PeerPolicy, StorageError};

#[tokio::test]
async fn peer_crud_and_policy_updates() {
    let dir = temp_dir();
    let database = DatabaseHandle::spawn(dir.join("ticonnect.db"))
        .await
        .expect("database");

    let inserted = database
        .insert_peer(peer("peer-1", "fingerprint-1"))
        .await
        .expect("insert");
    assert_eq!(inserted.policy, PeerPolicy::Accept);

    assert_eq!(
        database
            .get_peer("peer-1")
            .await
            .expect("get")
            .expect("peer")
            .display_name
            .as_deref(),
        Some("Peer")
    );
    assert_eq!(
        database
            .get_peer_by_fingerprint("fingerprint-1")
            .await
            .expect("get by fingerprint")
            .expect("peer")
            .id,
        "peer-1"
    );

    assert!(
        database
            .update_peer_policy("peer-1", PeerPolicy::Mute)
            .await
            .expect("policy")
    );
    assert!(
        database
            .update_peer_last_seen("peer-1", 42)
            .await
            .expect("last seen")
    );

    let peers = database.list_peers().await.expect("list");
    assert_eq!(peers.len(), 1);
    assert_eq!(peers[0].policy, PeerPolicy::Mute);
    assert_eq!(peers[0].last_seen, Some(42));

    assert!(database.delete_peer("peer-1").await.expect("delete"));
    assert!(database.list_peers().await.expect("list").is_empty());

    drop(database);
    let _ = fs::remove_dir_all(dir);
}

#[tokio::test]
async fn duplicate_fingerprint_is_a_typed_conflict() {
    let dir = temp_dir();
    let database = DatabaseHandle::spawn(dir.join("ticonnect.db"))
        .await
        .expect("database");

    database
        .insert_peer(peer("peer-1", "fingerprint-1"))
        .await
        .expect("first insert");
    let error = database
        .insert_peer(peer("peer-2", "fingerprint-1"))
        .await
        .expect_err("duplicate");

    assert!(matches!(error, StorageError::DuplicateFingerprint(_)));

    drop(database);
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn invalid_policy_is_rejected() {
    let error = "block".parse::<PeerPolicy>().expect_err("invalid policy");
    assert!(matches!(error, StorageError::InvalidPolicy(_)));
}

fn peer(id: &str, fingerprint: &str) -> NewPeer {
    NewPeer {
        id: id.to_owned(),
        fingerprint: fingerprint.to_owned(),
        display_name: Some("Peer".to_owned()),
        policy: PeerPolicy::Accept,
        trusted_at: 1,
        last_seen: None,
    }
}

fn temp_dir() -> PathBuf {
    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT: AtomicU64 = AtomicU64::new(0);
    let id = NEXT.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!("ticonnect-peers-{}-{id}", std::process::id()))
}
