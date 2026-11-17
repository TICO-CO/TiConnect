use std::fs;
use std::path::PathBuf;

use ticonnect_core::id::NodeId;
use ticonnect_core::message::{MessageRecord, conversation_id};
use ticonnect_daemon::storage::{DatabaseHandle, MessageState, NewUser, StorageError, UserPolicy};

#[tokio::test]
async fn user_crud_and_policy_updates() {
    let dir = temp_dir();
    let database = DatabaseHandle::spawn(dir.join("ticonnect.db"))
        .await
        .expect("database");

    let inserted = database
        .insert_user(user("user-1", "fingerprint-1"))
        .await
        .expect("insert");
    assert_eq!(inserted.policy, UserPolicy::Accept);

    assert_eq!(
        database
            .get_user("user-1")
            .await
            .expect("get")
            .expect("user")
            .display_name
            .as_deref(),
        Some("User")
    );
    assert_eq!(
        database
            .get_user_by_fingerprint("fingerprint-1")
            .await
            .expect("get by fingerprint")
            .expect("user")
            .id,
        "user-1"
    );

    assert!(
        database
            .update_user_policy("user-1", UserPolicy::Mute)
            .await
            .expect("policy")
    );
    assert!(
        database
            .update_user_last_seen("user-1", 42)
            .await
            .expect("last seen")
    );

    let users = database.list_users().await.expect("list");
    assert_eq!(users.len(), 1);
    assert_eq!(users[0].policy, UserPolicy::Mute);
    assert_eq!(users[0].last_seen, Some(42));

    assert!(database.delete_user("user-1").await.expect("delete"));
    assert!(database.list_users().await.expect("list").is_empty());

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
        .insert_user(user("user-1", "fingerprint-1"))
        .await
        .expect("first insert");
    let error = database
        .insert_user(user("user-2", "fingerprint-1"))
        .await
        .expect_err("duplicate");

    assert!(matches!(error, StorageError::DuplicateFingerprint(_)));

    drop(database);
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn invalid_policy_is_rejected() {
    let error = "block".parse::<UserPolicy>().expect_err("invalid policy");
    assert!(matches!(error, StorageError::InvalidPolicy(_)));
}

#[tokio::test]
async fn deleting_a_user_preserves_sequence_state_and_clears_outbox() {
    let dir = temp_dir();
    let database = DatabaseHandle::spawn(dir.join("ticonnect.db"))
        .await
        .expect("database");
    let local = NodeId::generate();
    let user_node = NodeId::generate();
    let user_id = user_node.to_string();
    let conversation = conversation_id(local, user_node);

    database
        .insert_user(user(&user_id, "fingerprint-1"))
        .await
        .expect("user");
    database
        .ensure_conversation(&conversation, &user_id, 1)
        .await
        .expect("conversation");
    let seq = database.allocate_seq(&user_id).await.expect("seq");
    assert_eq!(seq, 1);

    let record = MessageRecord::text(&conversation, local.to_string(), seq, 1, "pending");
    database
        .insert_message(
            record.clone(),
            1,
            vec![0_u8; 64],
            Some(MessageState::Queued),
        )
        .await
        .expect("message");
    database
        .enqueue_outbox(&record.message_id.to_string(), 0, i64::MAX)
        .await
        .expect("outbox");
    assert_eq!(database.outbox_depth().await.expect("depth"), 1);

    assert!(database.delete_user(&user_id).await.expect("delete"));
    assert!(database.list_users().await.expect("users").is_empty());
    assert_eq!(database.outbox_depth().await.expect("depth"), 0);
    assert_eq!(
        database.user_state(&user_id).await.expect("user state"),
        ticonnect_daemon::storage::UserState {
            last_sent_seq: 1,
            last_recv_seq: 0,
        }
    );

    database
        .insert_user(user(&user_id, "fingerprint-1"))
        .await
        .expect("re-pair");
    database
        .ensure_conversation(&conversation, &user_id, 2)
        .await
        .expect("conversation");
    assert_eq!(database.allocate_seq(&user_id).await.expect("next seq"), 2);

    drop(database);
    let _ = fs::remove_dir_all(dir);
}

fn user(id: &str, fingerprint: &str) -> NewUser {
    NewUser {
        id: id.to_owned(),
        fingerprint: fingerprint.to_owned(),
        public_key: None,
        display_name: Some("User".to_owned()),
        policy: UserPolicy::Accept,
        trusted_at: 1,
        last_seen: None,
    }
}

fn temp_dir() -> PathBuf {
    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT: AtomicU64 = AtomicU64::new(0);
    let id = NEXT.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!("ticonnect-users-{}-{id}", std::process::id()))
}
