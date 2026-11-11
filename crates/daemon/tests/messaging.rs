use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::Arc;
use std::time::Duration;

use ticonnect_core::id::NodeId;
use ticonnect_core::message::conversation_id;
use ticonnect_daemon::identity::Identity;
use ticonnect_daemon::messaging::MessagingService;
use ticonnect_daemon::storage::{DatabaseHandle, MessageState, NewPeer, PeerPolicy};
use ticonnect_daemon::trust::TrustStore;
use ticonnect_net::transport::{DialTarget, Transport, TransportConfig};
use ticonnect_settings::Settings;

struct Node {
    dir: std::path::PathBuf,
    node_id: NodeId,
    transport: Arc<Transport>,
    trust: Arc<TrustStore>,
    messaging: MessagingService,
}

#[tokio::test]
async fn two_nodes_exchange_messages_and_receipts() {
    let first = node("first").await;
    let second = node("second").await;

    let first_identity = Identity::load_or_create(&first.dir).expect("first identity");
    let second_identity = Identity::load_or_create(&second.dir).expect("second identity");

    first
        .trust
        .commit(NewPeer {
            id: second.node_id.to_string(),
            fingerprint: second.transport.fingerprint().to_hex(),
            public_key: Some(second_identity.key_pair().verifying_key().to_bytes()),
            display_name: Some("Second".to_owned()),
            policy: PeerPolicy::Accept,
            trusted_at: 1,
            last_seen: None,
        })
        .await
        .expect("trust second");
    second
        .trust
        .commit(NewPeer {
            id: first.node_id.to_string(),
            fingerprint: first.transport.fingerprint().to_hex(),
            public_key: Some(first_identity.key_pair().verifying_key().to_bytes()),
            display_name: Some("First".to_owned()),
            policy: PeerPolicy::Accept,
            trusted_at: 1,
            last_seen: None,
        })
        .await
        .expect("trust first");

    let target = DialTarget::new(
        second.transport.local_addr().expect("addr"),
        second.transport.fingerprint(),
    )
    .with_node_id(second.node_id);
    let session = first
        .transport
        .connect_with_retry(target)
        .await
        .expect("connect");

    first
        .messaging
        .on_peer_connected(session.peer().clone())
        .await;
    let remote = second
        .transport
        .sessions()
        .into_iter()
        .find(|session| session.peer().node_id == first.node_id)
        .expect("second session")
        .peer()
        .clone();
    second.messaging.on_peer_connected(remote).await;

    wait_for(Duration::from_secs(10), || async {
        first.messaging.sync_ready(&second.node_id.to_string())
            && second.messaging.sync_ready(&first.node_id.to_string())
    })
    .await
    .expect("sync ready");

    let conversation = conversation_id(first.node_id, second.node_id);
    let message_id = first
        .messaging
        .send_message(&second.node_id.to_string(), "hello")
        .await
        .expect("send");

    wait_for(Duration::from_secs(10), || async {
        second
            .messaging
            .list_messages(&conversation, 10, 0)
            .await
            .map(|messages| !messages.is_empty())
            .unwrap_or(false)
    })
    .await
    .expect("message received");

    tokio::time::sleep(Duration::from_millis(600)).await;
    second.messaging.flush_receipts().await;
    wait_for(Duration::from_secs(10), || async {
        first
            .messaging
            .message_state(&message_id.to_string())
            .await
            .map(|state| state == MessageState::Delivered)
            .unwrap_or(false)
    })
    .await
    .expect("delivered receipt");

    second
        .messaging
        .mark_read(&conversation, &[message_id.to_string()])
        .await
        .expect("mark read");
    tokio::time::sleep(Duration::from_millis(600)).await;
    second.messaging.flush_receipts().await;
    wait_for(Duration::from_secs(10), || async {
        first
            .messaging
            .message_state(&message_id.to_string())
            .await
            .map(|state| state == MessageState::Read)
            .unwrap_or(false)
    })
    .await
    .expect("read receipt");

    second
        .messaging
        .set_peer_policy(&first.node_id.to_string(), PeerPolicy::Ignore)
        .await
        .expect("ignore first");
    first
        .messaging
        .send_message(&second.node_id.to_string(), "ignored")
        .await
        .expect("send ignored");
    tokio::time::sleep(Duration::from_millis(500)).await;
    assert_eq!(
        second
            .messaging
            .list_messages(&conversation, 10, 0)
            .await
            .expect("history")
            .len(),
        1
    );

    first.transport.shutdown().await;
    second.transport.shutdown().await;
    let _ = std::fs::remove_dir_all(&first.dir);
    let _ = std::fs::remove_dir_all(&second.dir);
}

async fn node(label: &str) -> Node {
    let dir =
        std::env::temp_dir().join(format!("ticonnect-two-node-{}-{label}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("dir");
    let settings = Settings::load_from(dir.join("config.toml")).expect("settings");
    let identity = Identity::load_or_create(&dir).expect("identity");
    let public = identity.public();
    let database = DatabaseHandle::spawn(dir.join("ticonnect.db"))
        .await
        .expect("database");
    let trust = Arc::new(TrustStore::load(database.clone()).await.expect("trust"));
    let transport = Arc::new(
        Transport::bind(TransportConfig {
            identity: identity.key_pair().clone(),
            node_id: public.node_id(),
            fingerprint: public.fingerprint(),
            bind_addr: SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 0),
            trust: trust.clone(),
        })
        .await
        .expect("transport"),
    );
    let messaging = MessagingService::new(
        identity.key_pair().clone(),
        public.node_id(),
        database.clone(),
        trust.clone(),
        transport.clone(),
        settings,
    );
    Node {
        dir,
        node_id: public.node_id(),
        transport,
        trust,
        messaging,
    }
}

async fn wait_for<F, Fut>(timeout: Duration, mut condition: F) -> Result<(), ()>
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = bool>,
{
    let deadline = tokio::time::Instant::now() + timeout;
    loop {
        if condition().await {
            return Ok(());
        }
        if tokio::time::Instant::now() >= deadline {
            return Err(());
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}
