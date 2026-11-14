mod support;

use std::time::Duration;

use support::{TestNode, connect_pair, wait_for};
use ticonnect_core::attachment::AttachmentHash;
use ticonnect_daemon::storage::TransferPartial;

#[tokio::test]
async fn resumes_partial_attachment_without_receiving_the_prefix_again() {
    let first = TestNode::start("att-resume-first").await;
    let second = TestNode::start("att-resume-second").await;
    connect_pair(&first, &second).await;

    let bytes: Vec<u8> = (0..256 * 1024).map(|index| (index % 251) as u8).collect();
    let hash = AttachmentHash::of(&bytes);
    let path = first.dir.join("resume.bin");
    std::fs::write(&path, &bytes).expect("write");
    let attachment = first
        .attachments
        .prepare_file(&path, Some("application/octet-stream"))
        .await
        .expect("prepare");
    assert_eq!(attachment.hash, hash);
    first
        .attachments
        .store()
        .verify(hash)
        .expect("sender store");

    let prefix = 64 * 1024usize;
    let partial = second.attachments.store().partial_path(hash);
    if let Some(parent) = partial.parent() {
        std::fs::create_dir_all(parent).expect("partial dir");
    }
    std::fs::write(&partial, &bytes[..prefix]).expect("write partial");
    second
        .attachments
        .database()
        .upsert_transfer_partial(TransferPartial {
            hash: hash.to_hex(),
            peer_id: first.node_id().to_string(),
            size: bytes.len() as u64,
            bytes_received: prefix as u64,
            temp_path: partial.display().to_string(),
            ranges: format!("0:{prefix}"),
            updated_at: 1,
        })
        .await
        .expect("partial row");

    second
        .attachments
        .receive_attachment(&first.node_id().to_string(), attachment)
        .await
        .expect("resume transfer");
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
    .expect("attachment metadata");
    assert_eq!(
        std::fs::read(second.attachments.store().path_for(hash)).expect("content"),
        bytes
    );
    assert!(
        second
            .attachments
            .database()
            .get_transfer_partial(&hash.to_hex())
            .await
            .expect("partial lookup")
            .is_none(),
        "partial state was not cleared"
    );

    first.shutdown().await;
    second.shutdown().await;
}
