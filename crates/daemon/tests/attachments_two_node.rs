mod support;

use std::time::Duration;

use support::{TestNode, connect_pair, wait_for};
use ticonnect_core::attachment::AttachmentHash;
use ticonnect_core::message::{MessageKind, conversation_id};

#[tokio::test]
async fn sends_image_audio_and_file_between_two_nodes() {
    let first = TestNode::start("att-send-first").await;
    let second = TestNode::start("att-send-second").await;
    connect_pair(&first, &second).await;

    let conversation = conversation_id(first.node_id(), second.node_id());
    let cases = [
        (
            "photo.png",
            "image/png",
            b"\x89PNG\r\n\x1a\nintegration".as_slice(),
            MessageKind::Image,
        ),
        (
            "clip.ogg",
            "audio/ogg",
            b"OggS-integration-audio".as_slice(),
            MessageKind::Audio,
        ),
        (
            "report.pdf",
            "application/pdf",
            b"%PDF-1.7 integration".as_slice(),
            MessageKind::File,
        ),
    ];

    for (index, (name, mime, bytes, kind)) in cases.iter().enumerate() {
        let path = first.dir.join(name);
        std::fs::write(&path, bytes).expect("write attachment");
        let attachment = first
            .attachments
            .prepare_file(&path, Some(mime))
            .await
            .expect("prepare attachment");
        first
            .attachments
            .record_local(&attachment)
            .await
            .expect("record local");
        let message_id = first
            .messaging
            .send_attachment(&second.node_id().to_string(), &attachment)
            .await
            .expect("send attachment");

        wait_for(Duration::from_secs(10), || async {
            second
                .attachments
                .get(&attachment.hash.to_hex())
                .await
                .ok()
                .flatten()
                .map(|record| record.hash)
        })
        .await
        .expect("attachment stored");

        let stored = second
            .attachments
            .get(&attachment.hash.to_hex())
            .await
            .expect("metadata")
            .expect("attachment row");
        assert_eq!(stored.name, *name);
        assert_eq!(stored.mime, *mime);
        assert_eq!(stored.size, bytes.len() as u64);
        assert_eq!(
            std::fs::read(second.attachments.store().path_for(attachment.hash)).expect("content"),
            *bytes
        );

        wait_for(Duration::from_secs(10), || async {
            second
                .messaging
                .list_messages(&conversation, 20, 0)
                .await
                .ok()
                .and_then(|messages| {
                    messages.into_iter().find(|message| {
                        message.record.message_id.to_string() == message_id.to_string()
                    })
                })
                .map(|message| message.record.kind)
        })
        .await
        .expect("message stored");

        let messages = second
            .messaging
            .list_messages(&conversation, 20, 0)
            .await
            .expect("history");
        let message = messages
            .into_iter()
            .find(|message| message.record.message_id.to_string() == message_id.to_string())
            .expect("attachment message");
        assert_eq!(message.record.kind, *kind);
        assert_eq!(message.record.attachment_name.as_deref(), Some(*name));
        assert_eq!(
            message.record.attachment_hash.as_deref(),
            Some(attachment.hash.to_hex().as_str())
        );
        assert_eq!(
            message.record.attachment_size,
            Some(bytes.len() as u64),
            "case {index}"
        );
    }

    first.shutdown().await;
    second.shutdown().await;
}

#[tokio::test]
async fn duplicate_content_skips_second_transfer_and_ignore_blocks_new_attachments() {
    let first = TestNode::start("att-dedupe-first").await;
    let second = TestNode::start("att-dedupe-second").await;
    connect_pair(&first, &second).await;
    let peer_first = first.node_id().to_string();
    let peer_second = second.node_id().to_string();
    let conversation = conversation_id(first.node_id(), second.node_id());

    let path = first.dir.join("duplicate.bin");
    let bytes = vec![7_u8; 192 * 1024];
    std::fs::write(&path, &bytes).expect("write");
    let attachment = first
        .attachments
        .prepare_file(&path, Some("application/octet-stream"))
        .await
        .expect("prepare");
    let hash = attachment.hash;
    first
        .attachments
        .record_local(&attachment)
        .await
        .expect("record");
    first
        .messaging
        .send_attachment(&peer_second, &attachment)
        .await
        .expect("send first");
    wait_for(Duration::from_secs(10), || async {
        second
            .attachments
            .get(&hash.to_hex())
            .await
            .ok()
            .flatten()
            .map(|_| ())
    })
    .await
    .expect("first transfer");

    let files_before = count_files(second.dir.join("attachments"));
    let mut events = second.attachments.subscribe();
    while events.try_recv().is_ok() {}
    first
        .messaging
        .send_attachment(&peer_second, &attachment)
        .await
        .expect("send duplicate");
    wait_for(Duration::from_secs(10), || async {
        second
            .messaging
            .list_messages(&conversation, 20, 0)
            .await
            .ok()
            .map(|messages| messages.len() == 2)
    })
    .await
    .expect("duplicate message arrives");
    tokio::time::sleep(Duration::from_millis(500)).await;
    assert_eq!(count_files(second.dir.join("attachments")), files_before);
    let mut saw_progress = false;
    while let Ok(event) = events.try_recv() {
        if let ticonnect_daemon::attachments::TransferEvent::Progress(snapshot) = event
            && snapshot.hash == hash.to_hex()
        {
            saw_progress = true;
        }
    }
    assert!(!saw_progress, "store-first path started a transfer");

    second
        .messaging
        .set_peer_policy(&peer_first, ticonnect_daemon::storage::PeerPolicy::Ignore)
        .await
        .expect("ignore peer");
    let ignored_path = first.dir.join("ignored.bin");
    std::fs::write(&ignored_path, b"ignored attachment").expect("write ignored");
    let ignored = first
        .attachments
        .prepare_file(&ignored_path, Some("application/octet-stream"))
        .await
        .expect("prepare ignored");
    first
        .messaging
        .send_attachment(&peer_second, &ignored)
        .await
        .expect("send ignored");
    tokio::time::sleep(Duration::from_millis(800)).await;
    assert!(
        second
            .attachments
            .get(&ignored.hash.to_hex())
            .await
            .expect("ignored lookup")
            .is_none(),
        "ignored attachment was stored"
    );

    second
        .messaging
        .set_peer_policy(&peer_first, ticonnect_daemon::storage::PeerPolicy::Accept)
        .await
        .expect("restore policy");
    first.shutdown().await;
    second.shutdown().await;
}

#[tokio::test]
async fn store_first_path_skips_transfer_for_known_content() {
    let first = TestNode::start("att-store-first").await;
    let second = TestNode::start("att-store-second").await;
    connect_pair(&first, &second).await;

    let bytes = b"already-present-content";
    let hash = AttachmentHash::of(bytes);
    let path = second
        .attachments
        .store()
        .put_bytes(hash, bytes)
        .expect("prestore");
    second
        .attachments
        .record_local(
            &ticonnect_core::attachment::AttachmentRef::new(
                hash,
                "known.bin",
                bytes.len() as u64,
                "application/octet-stream",
            )
            .expect("ref"),
        )
        .await
        .expect("record");
    assert!(path.is_file());

    let source = first.dir.join("known.bin");
    std::fs::write(&source, bytes).expect("write");
    let attachment = first
        .attachments
        .prepare_file(&source, Some("application/octet-stream"))
        .await
        .expect("prepare");
    let mut events = second.attachments.subscribe();
    first
        .messaging
        .send_attachment(&second.node_id().to_string(), &attachment)
        .await
        .expect("send");
    let event = tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            if let Ok(event) = events.recv().await {
                match event {
                    ticonnect_daemon::attachments::TransferEvent::Done { hash } => return hash,
                    ticonnect_daemon::attachments::TransferEvent::Progress(snapshot) => {
                        panic!("unexpected transfer progress: {snapshot:?}");
                    }
                    ticonnect_daemon::attachments::TransferEvent::Failed { reason, .. } => {
                        panic!("unexpected transfer failure: {reason}");
                    }
                }
            }
        }
    })
    .await
    .expect("done event");
    assert_eq!(event, hash.to_hex());

    first.shutdown().await;
    second.shutdown().await;
}

fn count_files(dir: std::path::PathBuf) -> usize {
    fn walk(dir: &std::path::Path, count: &mut usize) {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                walk(&path, count);
            } else {
                *count += 1;
            }
        }
    }
    let mut count = 0;
    walk(&dir, &mut count);
    count
}
