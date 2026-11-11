use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use ticonnect_daemon::identity::{Identity, PublicIdentity};
use ticonnect_daemon::pairing_service::PairingService;
use ticonnect_daemon::storage::{DatabaseHandle, NewPeer, PeerPolicy};
use ticonnect_daemon::trust::TrustStore;
use ticonnect_net::discovery::{DiscoveryConfig, LanDiscovery};
use ticonnect_net::transport::{DialTarget, Transport, TransportConfig};

#[tokio::test]
async fn forgetting_a_peer_removes_trust_and_closes_the_session() {
    let first = node("first").await;
    let second = node("second").await;

    commit(&first.trust, &second.public).await;
    commit(&second.trust, &first.public).await;

    let second_addr = second.transport.local_addr().expect("second address");
    first
        .transport
        .connect(
            DialTarget::new(second_addr, second.public.fingerprint())
                .with_node_id(second.public.node_id()),
        )
        .await
        .expect("connect");
    wait_until(|| first.transport.session_count() == 1).await;

    let pairing = PairingService::new(
        first.public.clone(),
        first.trust.clone(),
        first.transport.clone(),
        first.discovery.clone(),
    );
    pairing
        .forget_peer(&second.public.node_id().to_string())
        .await
        .expect("forget peer");

    wait_until(|| first.transport.session_count() == 0).await;
    wait_until(|| second.transport.session_count() == 0).await;

    first.transport.shutdown().await;
    second.transport.shutdown().await;
    let _ = std::fs::remove_dir_all(first.dir);
    let _ = std::fs::remove_dir_all(second.dir);
}

struct Node {
    dir: PathBuf,
    public: PublicIdentity,
    trust: Arc<TrustStore>,
    transport: Arc<Transport>,
    discovery: Arc<LanDiscovery>,
}

async fn node(name: &str) -> Node {
    let dir = std::env::temp_dir().join(format!("ticonnect-pairing-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("create dir");
    let identity = Identity::load_or_create(&dir).expect("identity");
    let public = identity.public();
    let database = DatabaseHandle::spawn(dir.join("ticonnect.db"))
        .await
        .expect("database");
    let trust = Arc::new(TrustStore::load(database).await.expect("trust"));
    let discovery = Arc::new(LanDiscovery::new(DiscoveryConfig::new(
        name,
        public.node_id().to_string(),
        public.fingerprint().to_hex(),
    )));
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

    Node {
        dir,
        public,
        trust,
        transport,
        discovery,
    }
}

async fn commit(trust: &TrustStore, peer: &PublicIdentity) {
    trust
        .commit(NewPeer {
            id: peer.node_id().to_string(),
            fingerprint: peer.fingerprint().to_hex(),
            display_name: None,
            policy: PeerPolicy::Accept,
            trusted_at: 1,
            last_seen: None,
        })
        .await
        .expect("commit");
}

async fn wait_until(mut condition: impl FnMut() -> bool) {
    for _ in 0..200 {
        if condition() {
            return;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    panic!("condition was not met");
}
