use std::fs;
use std::path::PathBuf;

use ticonnect_core::id::NodeId;
use ticonnect_core::identity::IdentityKeyPair;
use ticonnect_core::message::{MessageRecord, conversation_id};
use ticonnect_daemon::storage::{
    DatabaseHandle, InsertOutcome, MessageState, NewPeer, PeerPolicy, StorageError,
};

#[tokio::test]
async fn message_insert_history_and_state_are_transactional() {
    let dir = temp_dir();
    let database = DatabaseHandle::spawn(dir.join("ticonnect.db"))
        .await
        .expect("database");
    let local = NodeId::generate();
    let peer = NodeId::generate();
    let identity = IdentityKeyPair::from_secret_bytes([81_u8; 32]);
    database
        .insert_peer(NewPeer {
            id: peer.to_string(),
            fingerprint: "fingerprint".to_owned(),
            public_key: Some(identity.verifying_key().to_bytes()),
            display_name: None,
            policy: PeerPolicy::Accept,
            trusted_at: 1,
            last_seen: None,
        })
        .await
        .expect("peer");

    let conversation = conversation_id(local, peer);
    database
        .ensure_conversation(&conversation, &peer.to_string(), 1)
        .await
        .expect("conversation");

    let seq = database.allocate_seq(&peer.to_string()).await.expect("seq");
    assert_eq!(seq, 1);
    assert_eq!(
        database.allocate_seq(&peer.to_string()).await.expect("seq"),
        2
    );

    let record = MessageRecord::text(
        conversation.clone(),
        local.to_string(),
        1,
        1_700_000_000_000,
        "hello",
    );
    let envelope = record.signed(&identity).expect("sign");
    assert_eq!(
        database
            .insert_message(
                record.clone(),
                1_700_000_000_100,
                envelope.signature().to_vec(),
                Some(MessageState::Queued),
            )
            .await
            .expect("insert"),
        InsertOutcome::Inserted
    );
    assert_eq!(
        database
            .insert_message(
                record.clone(),
                1_700_000_000_100,
                envelope.signature().to_vec(),
                Some(MessageState::Queued),
            )
            .await
            .expect("duplicate"),
        InsertOutcome::Duplicate
    );

    let history = database
        .list_history(&conversation, 10, 0)
        .await
        .expect("history");
    assert_eq!(history.len(), 1);
    assert_eq!(history[0].record.body.as_deref(), Some("hello"));

    assert!(
        database
            .set_message_state(&record.message_id.to_string(), MessageState::Sent)
            .await
            .expect("sent")
    );
    assert!(
        !database
            .set_message_state(&record.message_id.to_string(), MessageState::Queued)
            .await
            .expect("backwards")
    );
    assert_eq!(
        database
            .message_state(&record.message_id.to_string())
            .await
            .expect("state"),
        Some(MessageState::Sent)
    );

    drop(database);
    let _ = fs::remove_dir_all(dir);
}

#[tokio::test]
async fn inbound_insert_advances_peer_state_and_outbox_expires() {
    let dir = temp_dir();
    let database = DatabaseHandle::spawn(dir.join("ticonnect.db"))
        .await
        .expect("database");
    let local = NodeId::generate();
    let peer = NodeId::generate();
    let identity = IdentityKeyPair::from_secret_bytes([82_u8; 32]);
    database
        .insert_peer(NewPeer {
            id: peer.to_string(),
            fingerprint: "fingerprint".to_owned(),
            public_key: Some(identity.verifying_key().to_bytes()),
            display_name: None,
            policy: PeerPolicy::Accept,
            trusted_at: 1,
            last_seen: None,
        })
        .await
        .expect("peer");
    let conversation = conversation_id(local, peer);
    database
        .ensure_conversation(&conversation, &peer.to_string(), 1)
        .await
        .expect("conversation");

    let record = MessageRecord::text(&conversation, peer.to_string(), 5, 10, "incoming");
    let envelope = record.signed(&identity).expect("sign");
    database
        .insert_inbound(
            record.clone(),
            record.sent_at,
            envelope.signature().to_vec(),
            &peer.to_string(),
        )
        .await
        .expect("inbound");
    assert_eq!(
        database
            .peer_state(&peer.to_string())
            .await
            .expect("peer state")
            .last_recv_seq,
        5
    );

    database
        .enqueue_outbox(&record.message_id.to_string(), 0, 1)
        .await
        .expect("outbox");
    assert_eq!(database.outbox_depth().await.expect("depth"), 1);
    assert!(
        database
            .due_outbox(&peer.to_string(), 1)
            .await
            .expect("due")
            .is_empty()
    );
    let expired = database.expire_outbox(10).await.expect("expire");
    assert_eq!(expired, vec![record.message_id.to_string()]);
    assert_eq!(database.outbox_depth().await.expect("depth"), 0);

    drop(database);
    let _ = fs::remove_dir_all(dir);
}

#[tokio::test]
async fn unknown_state_is_a_typed_error() {
    assert!(matches!(
        "bogus".parse::<MessageState>(),
        Err(StorageError::InvalidMessageState(_))
    ));
}

fn temp_dir() -> PathBuf {
    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT: AtomicU64 = AtomicU64::new(0);
    let id = NEXT.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!("ticonnect-messages-{}-{id}", std::process::id()))
}
