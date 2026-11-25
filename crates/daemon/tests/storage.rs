use std::fs;
use std::path::PathBuf;

use ticonnect_core::id::NodeId;
use ticonnect_core::message::{MessageKind, MessageRecord};
use ticonnect_daemon::storage::{DatabaseHandle, MessageState, NewUser, UserPolicy};

#[tokio::test]
async fn database_opens_creates_private_files_and_reports_integrity() {
    let dir = temp_dir();
    let path = dir.join("ticonnect.db");
    let database = DatabaseHandle::spawn(&path).await.expect("database");

    assert_eq!(database.integrity_check().await.expect("integrity"), "ok");
    assert_eq!(database.count_users().await.expect("users"), 0);
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
        .insert_user(user("user-1", "fingerprint-1"))
        .await
        .expect("insert");

    let reader = rusqlite::Connection::open(&path).expect("reader");
    let count: i64 = reader
        .query_row("SELECT COUNT(*) FROM users", [], |row| row.get(0))
        .expect("count");
    assert_eq!(count, 1);

    drop(database);
    let _ = fs::remove_dir_all(dir);
}

#[tokio::test]
async fn conversation_summaries_track_unread_until_marked_read() {
    let dir = temp_dir();
    let path = dir.join("ticonnect.db");
    let database = DatabaseHandle::spawn(&path).await.expect("database");
    let user_id = NodeId::generate().to_string();
    database
        .insert_user(user(&user_id, "fingerprint-1"))
        .await
        .expect("insert");
    database
        .ensure_conversation("conversation-1", &user_id, 1)
        .await
        .expect("conversation");
    let record = MessageRecord::new(
        "conversation-1",
        user_id.clone(),
        1,
        100,
        MessageKind::Text,
        Some("hello".to_owned()),
    );
    database
        .insert_inbound(record.clone(), 100, vec![0_u8; 64], &user_id)
        .await
        .expect("message");

    let summaries = database
        .list_conversation_summaries(&NodeId::generate().to_string())
        .await
        .expect("summaries");
    assert_eq!(summaries.len(), 1);
    assert_eq!(summaries[0].unread_count, 1);
    assert_eq!(summaries[0].last_body.as_deref(), Some("hello"));

    let updated = database
        .mark_messages_read("conversation-1", &[record.message_id.to_string()], 110)
        .await
        .expect("mark read");
    assert_eq!(updated, vec![record.message_id.to_string()]);
    let summaries = database
        .list_conversation_summaries(&NodeId::generate().to_string())
        .await
        .expect("summaries");
    assert_eq!(summaries[0].unread_count, 0);

    let second = MessageRecord::new(
        "conversation-1",
        user_id.clone(),
        2,
        200,
        MessageKind::Text,
        Some("again".to_owned()),
    );
    database
        .insert_inbound(second.clone(), 200, vec![0_u8; 64], &user_id)
        .await
        .expect("second message");
    let updated = database
        .mark_messages_read(
            "conversation-1",
            &[record.message_id.to_string(), second.message_id.to_string()],
            210,
        )
        .await
        .expect("mark mixed read");
    assert_eq!(updated, vec![second.message_id.to_string()]);

    drop(database);
    let _ = fs::remove_dir_all(dir);
}

#[tokio::test]
async fn mark_conversation_read_marks_only_unread_inbound() {
    let dir = temp_dir();
    let path = dir.join("ticonnect.db");
    let database = DatabaseHandle::spawn(&path).await.expect("database");
    let local_id = NodeId::generate().to_string();
    let user_id = NodeId::generate().to_string();
    database
        .insert_user(user(&user_id, "fingerprint-1"))
        .await
        .expect("insert");
    database
        .ensure_conversation("conversation-1", &user_id, 1)
        .await
        .expect("conversation");
    let first = MessageRecord::new(
        "conversation-1",
        user_id.clone(),
        1,
        100,
        MessageKind::Text,
        Some("first".to_owned()),
    );
    let second = MessageRecord::new(
        "conversation-1",
        user_id.clone(),
        2,
        200,
        MessageKind::Text,
        Some("second".to_owned()),
    );
    database
        .insert_inbound(first.clone(), 100, vec![0_u8; 64], &user_id)
        .await
        .expect("first message");
    database
        .insert_inbound(second.clone(), 200, vec![0_u8; 64], &user_id)
        .await
        .expect("second message");
    let outgoing = MessageRecord::new(
        "conversation-1",
        local_id.clone(),
        1,
        150,
        MessageKind::Text,
        Some("outgoing".to_owned()),
    );
    database
        .insert_message(
            outgoing.clone(),
            150,
            vec![0_u8; 64],
            Some(MessageState::Sent),
        )
        .await
        .expect("outgoing message");

    let mut updated = database
        .mark_conversation_read("conversation-1", &local_id, 300)
        .await
        .expect("mark conversation read");
    updated.sort();
    let mut expected = vec![first.message_id.to_string(), second.message_id.to_string()];
    expected.sort();
    assert_eq!(updated, expected);
    assert!(!updated.contains(&outgoing.message_id.to_string()));
    let summaries = database
        .list_conversation_summaries(&local_id)
        .await
        .expect("summaries");
    assert_eq!(summaries[0].unread_count, 0);

    let updated = database
        .mark_conversation_read("conversation-1", &local_id, 310)
        .await
        .expect("mark conversation read again");
    assert!(updated.is_empty());

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
