use std::fs;
use std::path::PathBuf;

use ticonnect_core::id::NodeId;
use ticonnect_core::identity::IdentityKeyPair;
use ticonnect_core::message::{MessageRecord, conversation_id};
use ticonnect_daemon::storage::{
    DatabaseHandle, InsertOutcome, MessageState, NewUser, StorageError, UserPolicy,
};

#[tokio::test]
async fn message_insert_history_and_state_are_transactional() {
    let dir = temp_dir();
    let database = DatabaseHandle::spawn(dir.join("ticonnect.db"))
        .await
        .expect("database");
    let local = NodeId::generate();
    let user = NodeId::generate();
    let identity = IdentityKeyPair::from_secret_bytes([81_u8; 32]);
    database
        .insert_user(NewUser {
            id: user.to_string(),
            fingerprint: "fingerprint".to_owned(),
            public_key: Some(identity.verifying_key().to_bytes()),
            display_name: None,
            policy: UserPolicy::Accept,
            trusted_at: 1,
            last_seen: None,
        })
        .await
        .expect("user");

    let conversation = conversation_id(local, user);
    database
        .ensure_conversation(&conversation, &user.to_string(), 1)
        .await
        .expect("conversation");

    let seq = database.allocate_seq(&user.to_string()).await.expect("seq");
    assert_eq!(seq, 1);
    assert_eq!(
        database.allocate_seq(&user.to_string()).await.expect("seq"),
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
    assert_eq!(
        database.count_history(&conversation).await.expect("count"),
        1
    );
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
async fn inbound_insert_advances_user_state_and_outbox_expires() {
    let dir = temp_dir();
    let database = DatabaseHandle::spawn(dir.join("ticonnect.db"))
        .await
        .expect("database");
    let local = NodeId::generate();
    let user = NodeId::generate();
    let identity = IdentityKeyPair::from_secret_bytes([82_u8; 32]);
    database
        .insert_user(NewUser {
            id: user.to_string(),
            fingerprint: "fingerprint".to_owned(),
            public_key: Some(identity.verifying_key().to_bytes()),
            display_name: None,
            policy: UserPolicy::Accept,
            trusted_at: 1,
            last_seen: None,
        })
        .await
        .expect("user");
    let conversation = conversation_id(local, user);
    database
        .ensure_conversation(&conversation, &user.to_string(), 1)
        .await
        .expect("conversation");

    let record = MessageRecord::text(&conversation, user.to_string(), 5, 10, "incoming");
    let envelope = record.signed(&identity).expect("sign");
    database
        .insert_inbound(
            record.clone(),
            record.sent_at,
            envelope.signature().to_vec(),
            &user.to_string(),
        )
        .await
        .expect("inbound");
    assert_eq!(
        database
            .user_state(&user.to_string())
            .await
            .expect("user state")
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
            .due_outbox(&user.to_string(), 1)
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
async fn records_between_filters_by_sender() {
    let dir = temp_dir();
    let database = DatabaseHandle::spawn(dir.join("ticonnect.db"))
        .await
        .expect("database");
    let local = NodeId::generate();
    let user = NodeId::generate();
    let local_identity = IdentityKeyPair::from_secret_bytes([83_u8; 32]);
    let user_identity = IdentityKeyPair::from_secret_bytes([84_u8; 32]);
    database
        .insert_user(NewUser {
            id: user.to_string(),
            fingerprint: "user-fingerprint".to_owned(),
            public_key: Some(user_identity.verifying_key().to_bytes()),
            display_name: None,
            policy: UserPolicy::Accept,
            trusted_at: 1,
            last_seen: None,
        })
        .await
        .expect("user");
    let conversation = conversation_id(local, user);
    database
        .ensure_conversation(&conversation, &user.to_string(), 1)
        .await
        .expect("conversation");

    let outgoing = MessageRecord::text(
        conversation.clone(),
        local.to_string(),
        1,
        1_700_000_000_000,
        "outgoing",
    );
    let outgoing_envelope = outgoing.signed(&local_identity).expect("sign outgoing");
    database
        .insert_message(
            outgoing,
            1_700_000_000_100,
            outgoing_envelope.signature().to_vec(),
            Some(MessageState::Queued),
        )
        .await
        .expect("insert outgoing");

    for (seq, state, body) in [
        (2, MessageState::Expired, "expired"),
        (3, MessageState::Failed, "failed"),
    ] {
        let terminal = MessageRecord::text(
            conversation.clone(),
            local.to_string(),
            seq,
            1_700_000_000_000 + i64::try_from(seq).expect("seq fits"),
            body,
        );
        database
            .insert_message(terminal, 1_700_000_000_100, vec![0_u8; 64], Some(state))
            .await
            .expect("insert terminal message");
    }

    let incoming = MessageRecord::text(
        conversation.clone(),
        user.to_string(),
        1,
        1_700_000_000_000,
        "incoming",
    );
    let incoming_envelope = incoming.signed(&user_identity).expect("sign incoming");
    database
        .insert_inbound(
            incoming,
            1_700_000_000_100,
            incoming_envelope.signature().to_vec(),
            &user.to_string(),
        )
        .await
        .expect("insert incoming");

    let local_records = database
        .records_between(&conversation, &local.to_string(), 1, 3, 10)
        .await
        .expect("local records");
    assert_eq!(local_records.len(), 1);
    assert_eq!(local_records[0].record.body.as_deref(), Some("outgoing"));

    let user_records = database
        .records_between(&conversation, &user.to_string(), 1, 1, 10)
        .await
        .expect("user records");
    assert_eq!(user_records.len(), 1);
    assert_eq!(user_records[0].record.body.as_deref(), Some("incoming"));

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

#[tokio::test]
async fn clearing_history_scopes_messages_and_unreferenced_attachments() {
    use ticonnect_core::attachment::{AttachmentHash, AttachmentRef};
    use ticonnect_core::message::MessageKind;
    use ticonnect_daemon::storage::{AttachmentRecord, TransferPartial};

    let dir = temp_dir();
    let database = DatabaseHandle::spawn(dir.join("ticonnect.db"))
        .await
        .expect("database");
    let local = NodeId::generate();
    let first = NodeId::generate();
    let second = NodeId::generate();
    let identity = IdentityKeyPair::from_secret_bytes([83_u8; 32]);
    for (id, fingerprint) in [
        (first.to_string(), "first".to_owned()),
        (second.to_string(), "second".to_owned()),
    ] {
        database
            .insert_user(NewUser {
                id,
                fingerprint,
                public_key: Some(identity.verifying_key().to_bytes()),
                display_name: None,
                policy: UserPolicy::Accept,
                trusted_at: 1,
                last_seen: None,
            })
            .await
            .expect("user");
    }

    let first_conversation = conversation_id(local, first);
    let second_conversation = conversation_id(local, second);
    database
        .ensure_conversation(&first_conversation, &first.to_string(), 1)
        .await
        .expect("first conversation");
    database
        .ensure_conversation(&second_conversation, &second.to_string(), 1)
        .await
        .expect("second conversation");

    let first_record = MessageRecord::text(
        first_conversation.clone(),
        local.to_string(),
        1,
        1_700_000_000_000,
        "first",
    );
    let envelope = first_record.signed(&identity).expect("sign");
    database
        .insert_message(
            first_record.clone(),
            1_700_000_000_000,
            envelope.signature().to_vec(),
            Some(MessageState::Queued),
        )
        .await
        .expect("first message");
    database
        .enqueue_outbox(&first_record.message_id.to_string(), 0, i64::MAX)
        .await
        .expect("first outbox");

    let hash = AttachmentHash::of(b"attachment");
    let attachment = AttachmentRef::new(hash, "clip.webm", 10, "audio/webm").expect("attachment");
    database
        .upsert_attachment(AttachmentRecord {
            hash: hash.to_hex(),
            size: 10,
            path: "/tmp/clip.webm".to_owned(),
            mime: "audio/webm".to_owned(),
            name: "clip.webm".to_owned(),
            created_at: 1,
        })
        .await
        .expect("attachment");
    let attachment_record = MessageRecord::new(
        first_conversation.clone(),
        local.to_string(),
        2,
        1_700_000_000_001,
        MessageKind::Audio,
        None,
    )
    .with_attachment(&attachment)
    .expect("attachment message");
    let envelope = attachment_record.signed(&identity).expect("sign");
    database
        .insert_message(
            attachment_record.clone(),
            1_700_000_000_001,
            envelope.signature().to_vec(),
            Some(MessageState::Queued),
        )
        .await
        .expect("attachment message");
    database
        .enqueue_outbox(&attachment_record.message_id.to_string(), 0, i64::MAX)
        .await
        .expect("attachment outbox");
    database
        .upsert_transfer_partial(TransferPartial {
            hash: hash.to_hex(),
            user_id: first.to_string(),
            size: 10,
            bytes_received: 4,
            temp_path: "/tmp/clip.webm.part".to_owned(),
            ranges: "0:4".to_owned(),
            updated_at: 1,
        })
        .await
        .expect("transfer partial");

    let second_record = MessageRecord::text(
        second_conversation.clone(),
        local.to_string(),
        1,
        1_700_000_000_002,
        "second",
    );
    let envelope = second_record.signed(&identity).expect("sign");
    database
        .insert_message(
            second_record,
            1_700_000_000_002,
            envelope.signature().to_vec(),
            Some(MessageState::Queued),
        )
        .await
        .expect("second message");

    let outcome = database
        .clear_history(Some(&first.to_string()))
        .await
        .expect("clear first");
    assert_eq!(outcome.messages, 2);
    assert_eq!(outcome.attachments, vec![hash.to_hex()]);
    assert_eq!(
        database
            .count_history(&first_conversation)
            .await
            .expect("first count"),
        0
    );
    assert_eq!(
        database
            .count_history(&second_conversation)
            .await
            .expect("second count"),
        1
    );
    assert!(
        database
            .get_attachment(&hash.to_hex())
            .await
            .expect("attachment")
            .is_none()
    );
    assert!(
        database
            .get_transfer_partial(&hash.to_hex())
            .await
            .expect("partial")
            .is_none()
    );
    assert_eq!(database.outbox_depth().await.expect("outbox depth"), 0);

    let usage = database.storage_usage().await.expect("usage");
    assert_eq!(usage.message_count, 1);
    assert_eq!(usage.attachment_count, 0);
    assert!(usage.database_bytes > 0);

    let orphan_hash = AttachmentHash::of(b"orphan-attachment").to_hex();
    database
        .upsert_attachment(AttachmentRecord {
            hash: orphan_hash.clone(),
            size: 3,
            path: "/tmp/orphan.bin".to_owned(),
            mime: "application/octet-stream".to_owned(),
            name: "orphan.bin".to_owned(),
            created_at: 2,
        })
        .await
        .expect("orphan attachment");

    let outcome = database.clear_history(None).await.expect("clear all");
    assert_eq!(outcome.messages, 1);
    assert_eq!(outcome.attachments, vec![orphan_hash.clone()]);
    assert!(
        database
            .get_attachment(&orphan_hash)
            .await
            .expect("orphan")
            .is_none()
    );
    assert_eq!(
        database
            .storage_usage()
            .await
            .expect("usage")
            .attachment_count,
        0
    );

    drop(database);
    let _ = fs::remove_dir_all(dir);
}
