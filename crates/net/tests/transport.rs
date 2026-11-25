use std::collections::HashMap;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::Arc;
use std::time::Duration;

use ticonnect_core::fingerprint::Fingerprint;
use ticonnect_core::id::NodeId;
use ticonnect_core::identity::IdentityKeyPair;
use ticonnect_net::transport::{
    DialTarget, Session, Transport, TransportConfig, TransportEvent, TrustLookup,
};

struct StaticTrust(HashMap<String, NodeId>);

impl TrustLookup for StaticTrust {
    fn is_trusted(&self, fingerprint: &str) -> bool {
        self.0.contains_key(fingerprint)
    }

    fn node_id_for_fingerprint(&self, fingerprint: &str) -> Option<NodeId> {
        self.0.get(fingerprint).copied()
    }
}

struct Node {
    transport: Transport,
    node_id: NodeId,
    fingerprint: Fingerprint,
    address: SocketAddr,
}

#[tokio::test]
async fn two_nodes_connect_over_loopback_and_exchange_handshakes() {
    let (first, second) = trusted_pair().await;
    first
        .transport
        .connect(target(&second))
        .await
        .expect("connect");

    wait_until(|| first.transport.session_count() == 1 && second.transport.session_count() == 1)
        .await;
    let sessions = first.transport.sessions();
    assert_eq!(sessions[0].user().node_id, second.node_id);
    assert_eq!(sessions[0].user().fingerprint, second.fingerprint);
    assert_eq!(first.transport.metrics().handshakes, 1);

    first.transport.shutdown().await;
    second.transport.shutdown().await;
}

#[tokio::test]
async fn untrusted_user_is_rejected_before_any_session() {
    let (first, _second) = trusted_pair().await;
    let untrusted = node_without_trust().await;

    let error = first
        .transport
        .connect(target(&untrusted))
        .await
        .expect_err("untrusted user must be rejected");
    assert!(error.to_string().contains("handshake") || error.to_string().contains("stream"));

    first.transport.shutdown().await;
    untrusted.transport.shutdown().await;
}

#[tokio::test]
async fn closing_a_session_removes_it_and_allows_reconnect() {
    let (first, second) = trusted_pair().await;
    let session: Session = first
        .transport
        .connect(target(&second))
        .await
        .expect("connect");
    wait_until(|| first.transport.session_count() == 1).await;

    session.close(0, b"test");
    wait_until(|| first.transport.session_count() == 0 && second.transport.session_count() == 0)
        .await;

    first
        .transport
        .connect(target(&second))
        .await
        .expect("reconnect");
    wait_until(|| first.transport.session_count() == 1 && second.transport.session_count() == 1)
        .await;

    first.transport.shutdown().await;
    second.transport.shutdown().await;
}

#[tokio::test]
async fn a_new_session_replaces_the_previous_session_for_the_same_user() {
    let (first, second) = trusted_pair().await;
    first
        .transport
        .connect(target(&second))
        .await
        .expect("first connect");
    wait_until(|| first.transport.session_count() == 1 && second.transport.session_count() == 1)
        .await;

    first
        .transport
        .connect(target(&second))
        .await
        .expect("second connect");
    wait_until(|| first.transport.session_count() == 1 && second.transport.session_count() == 1)
        .await;

    first.transport.shutdown().await;
    second.transport.shutdown().await;
}

#[tokio::test]
async fn trust_alpn_is_routed_to_a_trust_event() {
    let (first, second) = trusted_pair().await;
    let mut events = second.transport.subscribe();

    let connection = first
        .transport
        .connect_trust(target(&second))
        .await
        .expect("trust connection");
    let event = tokio::time::timeout(Duration::from_secs(5), events.recv())
        .await
        .expect("trust event timeout")
        .expect("trust event");

    match event {
        TransportEvent::TrustRequested {
            fingerprint,
            address,
            ..
        } => {
            assert_eq!(fingerprint, first.fingerprint);
            assert_eq!(address.ip(), IpAddr::V4(Ipv4Addr::LOCALHOST));
        }
        other => panic!("unexpected event: {other:?}"),
    }

    connection.close(0u32.into(), b"test");
    first.transport.shutdown().await;
    second.transport.shutdown().await;
}

async fn trusted_pair() -> (Node, Node) {
    let first_identity = IdentityKeyPair::from_secret_bytes([71_u8; 32]);
    let second_identity = IdentityKeyPair::from_secret_bytes([72_u8; 32]);
    let first_fingerprint = Fingerprint::of(&first_identity.verifying_key());
    let second_fingerprint = Fingerprint::of(&second_identity.verifying_key());
    let first_node_id = NodeId::generate();
    let second_node_id = NodeId::generate();

    let first = node(
        first_identity,
        first_node_id,
        first_fingerprint,
        [(second_fingerprint.to_hex(), second_node_id)],
    )
    .await;
    let second = node(
        second_identity,
        second_node_id,
        second_fingerprint,
        [(first_fingerprint.to_hex(), first_node_id)],
    )
    .await;
    (first, second)
}

async fn node_without_trust() -> Node {
    let identity = IdentityKeyPair::from_secret_bytes([73_u8; 32]);
    let fingerprint = Fingerprint::of(&identity.verifying_key());
    node(identity, NodeId::generate(), fingerprint, []).await
}

async fn node(
    identity: IdentityKeyPair,
    node_id: NodeId,
    fingerprint: Fingerprint,
    trusted: impl IntoIterator<Item = (String, NodeId)>,
) -> Node {
    let bind_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 0);
    let transport = Transport::bind(TransportConfig {
        identity: identity.clone(),
        node_id,
        fingerprint,
        bind_addr,
        trust: Arc::new(StaticTrust(trusted.into_iter().collect())),
    })
    .await
    .expect("bind transport");
    let address = transport.local_addr().expect("local address");
    Node {
        transport,
        node_id,
        fingerprint,
        address,
    }
}

fn target(node: &Node) -> DialTarget {
    DialTarget::new(node.address, node.fingerprint).with_node_id(node.node_id)
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
