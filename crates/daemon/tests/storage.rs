use std::fs;
use std::path::PathBuf;

use ticonnect_daemon::storage::{DatabaseHandle, NewPeer, PeerPolicy};

#[tokio::test]
async fn database_opens_creates_private_files_and_reports_integrity() {
    let dir = temp_dir();
    let path = dir.join("ticonnect.db");
    let database = DatabaseHandle::spawn(&path).await.expect("database");

    assert_eq!(database.integrity_check().await.expect("integrity"), "ok");
    assert_eq!(database.count_peers().await.expect("peers"), 0);
    assert_eq!(database.outbox_depth().await.expect("outbox"), 0);
    assert_eq!(mode(&dir), 0o700);
    assert_eq!(mode(&path), 0o600);

    drop(database);
    let _ = fs::remove_dir_all(dir);
}

#[tokio::test]
async fn wal_mode_allows_second_connection_to_read() {
    let dir = temp_dir();
    let path = dir.join("ticonnect.db");
    let database = DatabaseHandle::spawn(&path).await.expect("database");
    database
        .insert_peer(peer("peer-1", "fingerprint-1"))
        .await
        .expect("insert");

    let reader = rusqlite::Connection::open(&path).expect("reader");
    let count: i64 = reader
        .query_row("SELECT COUNT(*) FROM peers", [], |row| row.get(0))
        .expect("count");
    assert_eq!(count, 1);

    drop(database);
    let _ = fs::remove_dir_all(dir);
}

fn peer(id: &str, fingerprint: &str) -> NewPeer {
    NewPeer {
        id: id.to_owned(),
        fingerprint: fingerprint.to_owned(),
        public_key: None,
        display_name: Some("Peer".to_owned()),
        policy: PeerPolicy::Accept,
        trusted_at: 1,
        last_seen: None,
    }
}

fn mode(path: &std::path::Path) -> u32 {
    use std::os::unix::fs::PermissionsExt;

    fs::metadata(path).expect("metadata").permissions().mode() & 0o777
}

fn temp_dir() -> PathBuf {
    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT: AtomicU64 = AtomicU64::new(0);
    let id = NEXT.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!("ticonnect-storage-{}-{id}", std::process::id()))
}
