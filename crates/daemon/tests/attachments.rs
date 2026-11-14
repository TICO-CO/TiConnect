use std::path::PathBuf;

use ticonnect_daemon::storage::{AttachmentRecord, DatabaseHandle, TransferPartial};

#[tokio::test]
async fn attachment_and_partial_metadata_round_trip() {
    let path = temp_db();
    std::fs::create_dir_all(path.parent().expect("parent")).expect("dir");
    let database = DatabaseHandle::spawn(&path).await.expect("database");
    let attachment = AttachmentRecord {
        hash: "a".repeat(64),
        size: 42,
        path: "/tmp/a".to_owned(),
        mime: "image/png".to_owned(),
        name: "photo.png".to_owned(),
        created_at: 7,
    };
    database
        .upsert_attachment(attachment.clone())
        .await
        .expect("upsert attachment");
    assert_eq!(
        database
            .get_attachment(&attachment.hash)
            .await
            .expect("get"),
        Some(attachment.clone())
    );
    assert_eq!(database.list_attachments().await.expect("list").len(), 1);

    let partial = TransferPartial {
        hash: attachment.hash.clone(),
        peer_id: "peer".to_owned(),
        size: 42,
        bytes_received: 10,
        temp_path: "/tmp/a.part".to_owned(),
        ranges: "0:10".to_owned(),
        updated_at: 8,
    };
    database
        .upsert_transfer_partial(partial.clone())
        .await
        .expect("upsert partial");
    assert_eq!(
        database
            .get_transfer_partial(&partial.hash)
            .await
            .expect("get partial"),
        Some(partial)
    );
    assert!(
        database
            .remove_transfer_partial(&attachment.hash)
            .await
            .expect("remove")
    );
    assert!(
        database
            .get_transfer_partial(&attachment.hash)
            .await
            .expect("get removed")
            .is_none()
    );

    drop(database);
    if let Some(dir) = path.parent() {
        let _ = std::fs::remove_dir_all(dir);
    }
}

fn temp_db() -> PathBuf {
    use std::sync::atomic::{AtomicU64, Ordering};
    static NEXT: AtomicU64 = AtomicU64::new(0);
    std::env::temp_dir()
        .join(format!(
            "ticonnect-attachments-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ))
        .join("ticonnect.db")
}

#[tokio::test]
async fn two_nodes_transfer_and_verify_attachment() {
    use std::sync::Arc;

    use ticonnect_core::attachment::AttachmentHash;
    use ticonnect_daemon::attachments::AttachmentService;
    use ticonnect_daemon::identity::Identity;
    use ticonnect_daemon::storage::NewPeer;
    use ticonnect_daemon::trust::TrustStore;
    use ticonnect_net::transfer::transfer_frame_from_wire;
    use ticonnect_net::transport::{DialTarget, Transport, TransportConfig};
    use ticonnect_settings::Settings;

    struct Node {
        identity: ticonnect_daemon::identity::PublicIdentity,
        trust: Arc<TrustStore>,
        transport: Arc<Transport>,
        service: AttachmentService,
        dir: PathBuf,
    }

    async fn node(name: &str) -> Node {
        let dir = std::env::temp_dir().join(format!(
            "ticonnect-node-{}-{name}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).expect("dir");
        let settings = Settings::load_from(dir.join("config.toml")).expect("settings");
        let identity = Identity::load_or_create(&dir).expect("identity");
        let public = identity.public();
        let database = DatabaseHandle::spawn(dir.join("ticonnect.db"))
            .await
            .expect("db");
        let trust = Arc::new(TrustStore::load(database.clone()).await.expect("trust"));
        let transport = Arc::new(
            Transport::bind(TransportConfig {
                identity: identity.key_pair().clone(),
                node_id: public.node_id(),
                fingerprint: public.fingerprint(),
                bind_addr: std::net::SocketAddr::from(([127, 0, 0, 1], 0)),
                trust: trust.clone(),
            })
            .await
            .expect("transport"),
        );
        let service = AttachmentService::new(
            database,
            dir.join("attachments"),
            transport.clone(),
            settings,
        )
        .expect("service");
        Node {
            identity: public,
            trust,
            transport,
            service,
            dir,
        }
    }

    let sender = node("sender").await;
    let receiver = node("receiver").await;
    for (owner, peer) in [(&sender, &receiver), (&receiver, &sender)] {
        owner
            .trust
            .commit(NewPeer {
                id: peer.identity.node_id().to_string(),
                fingerprint: peer.identity.fingerprint().to_hex(),
                public_key: None,
                display_name: Some("Peer".to_owned()),
                policy: ticonnect_daemon::storage::PeerPolicy::Accept,
                trusted_at: 0,
                last_seen: None,
            })
            .await
            .expect("trust peer");
    }
    let address = sender.transport.local_addr().expect("addr");
    receiver
        .transport
        .connect(DialTarget::new(address, sender.identity.fingerprint()))
        .await
        .expect("connect");
    let sender_id = sender.identity.node_id();
    let receiver_id = receiver.identity.node_id();

    let source = sender.dir.join("payload.bin");
    let data = b"resumable attachment payload";
    std::fs::write(&source, data).expect("write");
    let attachment = sender
        .service
        .prepare_file(&source, Some("application/octet-stream"))
        .await
        .expect("prepare");
    assert_eq!(attachment.hash, AttachmentHash::of(data));
    sender
        .service
        .record_local(&attachment)
        .await
        .expect("record");

    let sender_transport = sender.transport.clone();
    let sender_service = sender.service.clone();
    let serve = tokio::spawn(async move {
        let session = loop {
            if let Some(session) = sender_transport.session_by_node_id(receiver_id) {
                break session;
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        };
        let (send, mut recv) = session.accept_bi().await.expect("accept");
        let frame = ticonnect_net::transport::session::read_frame(&mut recv)
            .await
            .expect("request");
        let request = transfer_frame_from_wire(&frame).expect("transfer frame");
        sender_service
            .serve_attachment(session.peer().clone(), send, recv, request)
            .await
            .expect("serve");
    });

    receiver
        .service
        .receive_attachment(&sender_id.to_string(), attachment.clone())
        .await
        .expect("receive");
    serve.await.expect("serve task");

    let hash = attachment.hash;
    assert!(receiver.service.store().contains(hash));
    assert!(
        receiver
            .service
            .get(&hash.to_hex())
            .await
            .expect("metadata")
            .is_some()
    );
    assert_eq!(
        std::fs::read(receiver.service.store().path_for(hash)).expect("content"),
        data
    );

    let _ = std::fs::remove_dir_all(sender.dir);
    let _ = std::fs::remove_dir_all(receiver.dir);
}
