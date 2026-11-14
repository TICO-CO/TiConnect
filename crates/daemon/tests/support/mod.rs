#![allow(dead_code)]

use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use ticonnect_core::id::NodeId;
use ticonnect_daemon::attachments::AttachmentService;
use ticonnect_daemon::calls::CallService;
use ticonnect_daemon::identity::{Identity, PublicIdentity};
use ticonnect_daemon::messaging::MessagingService;
use ticonnect_daemon::remote::{NoopRemoteHost, RemoteService};
use ticonnect_daemon::storage::{DatabaseHandle, NewPeer, PeerPolicy};
use ticonnect_daemon::trust::TrustStore;
use ticonnect_media::ringtone::NullRingtone;
use ticonnect_net::transport::{DialTarget, Transport, TransportConfig};
use ticonnect_settings::Settings;
use tokio::task::JoinHandle;

pub struct TestNode {
    pub dir: PathBuf,
    pub identity: PublicIdentity,
    pub public_key: [u8; 32],
    pub trust: Arc<TrustStore>,
    pub transport: Arc<Transport>,
    pub database: DatabaseHandle,
    pub messaging: MessagingService,
    pub attachments: AttachmentService,
    pub calls: CallService,
    pub remote: RemoteService,
    workers: Vec<JoinHandle<()>>,
}

impl TestNode {
    pub async fn start(label: &str) -> Self {
        Self::start_with_clock(label, Arc::new(ticonnect_daemon::messaging::now_unix_ms)).await
    }

    pub async fn start_with_clock(label: &str, clock: Arc<dyn Fn() -> i64 + Send + Sync>) -> Self {
        let dir = std::env::temp_dir().join(format!(
            "ticonnect-it-{}-{label}-{}-{}",
            std::process::id(),
            unique(),
            unique()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("test dir");

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
            settings.clone(),
        );
        let attachments = AttachmentService::new(
            database.clone(),
            dir.join("attachments"),
            transport.clone(),
            settings.clone(),
        )
        .expect("attachments");
        let calls = CallService::new(
            public.node_id(),
            database.clone(),
            trust.clone(),
            transport.clone(),
            settings.clone(),
            Arc::new(NullRingtone::default()),
        );
        calls.set_media_enabled(false);
        let remote = RemoteService::with_clock(
            trust.clone(),
            transport.clone(),
            settings,
            Arc::new(NoopRemoteHost),
            clock,
        );

        let (transfer_tx, transfer_rx) = tokio::sync::mpsc::unbounded_channel();
        messaging.set_transfer_sender(transfer_tx);
        let (call_tx, call_rx) = tokio::sync::mpsc::unbounded_channel();
        let (audio_tx, audio_rx) = tokio::sync::mpsc::unbounded_channel();
        messaging.set_call_channels(call_tx, audio_tx);
        let (remote_tx, remote_rx) = tokio::sync::mpsc::unbounded_channel();
        messaging.set_remote_sender(remote_tx);

        let workers = vec![
            ticonnect_daemon::attachments::spawn(
                attachments.clone(),
                messaging.subscribe(),
                transfer_rx,
            ),
            ticonnect_daemon::calls::spawn(calls.clone(), call_rx, audio_rx),
            ticonnect_daemon::remote::spawn(remote.clone(), remote_rx),
        ];

        Self {
            dir,
            identity: public,
            public_key: identity.key_pair().verifying_key().to_bytes(),
            trust,
            transport,
            database,
            messaging,
            attachments,
            calls,
            remote,
            workers,
        }
    }

    pub fn node_id(&self) -> NodeId {
        self.identity.node_id()
    }

    pub async fn shutdown(self) {
        self.transport.shutdown().await;
        for worker in self.workers {
            worker.abort();
        }
        let _ = tokio::fs::remove_dir_all(&self.dir).await;
    }
}

pub async fn trust_pair(first: &TestNode, second: &TestNode) {
    first
        .trust
        .commit(NewPeer {
            id: second.node_id().to_string(),
            fingerprint: second.identity.fingerprint().to_hex(),
            public_key: Some(second.public_key),
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
            id: first.node_id().to_string(),
            fingerprint: first.identity.fingerprint().to_hex(),
            public_key: Some(first.public_key),
            display_name: Some("First".to_owned()),
            policy: PeerPolicy::Accept,
            trusted_at: 1,
            last_seen: None,
        })
        .await
        .expect("trust first");
}

pub async fn connect_pair(first: &TestNode, second: &TestNode) {
    trust_pair(first, second).await;
    let target = DialTarget::new(
        second.transport.local_addr().expect("second addr"),
        second.identity.fingerprint(),
    )
    .with_node_id(second.node_id());
    let first_session = first
        .transport
        .connect_with_retry(target)
        .await
        .expect("connect second");
    first
        .messaging
        .on_peer_connected(first_session.peer().clone())
        .await;
    let second_peer = wait_for(Duration::from_secs(5), || async {
        second
            .transport
            .session_by_node_id(first.node_id())
            .map(|session| session.peer().clone())
    })
    .await
    .expect("second session");
    second.messaging.on_peer_connected(second_peer).await;
    wait_true(Duration::from_secs(10), || async {
        first.messaging.sync_ready(&second.node_id().to_string())
            && second.messaging.sync_ready(&first.node_id().to_string())
    })
    .await
    .expect("sync ready");
}

pub async fn wait_for<T, F, Fut>(timeout: Duration, mut condition: F) -> Result<T, ()>
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = Option<T>>,
{
    let deadline = tokio::time::Instant::now() + timeout;
    loop {
        if let Some(value) = condition().await {
            return Ok(value);
        }
        if tokio::time::Instant::now() >= deadline {
            return Err(());
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
}

pub async fn wait_true<F, Fut>(timeout: Duration, mut condition: F) -> Result<(), ()>
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
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
}

pub fn unique() -> u64 {
    use std::sync::atomic::{AtomicU64, Ordering};
    static NEXT: AtomicU64 = AtomicU64::new(0);
    NEXT.fetch_add(1, Ordering::Relaxed)
}
