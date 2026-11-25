use std::collections::HashMap;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::Arc;
use std::time::Duration;

use ticonnect_core::fingerprint::Fingerprint;
use ticonnect_core::id::NodeId;
use ticonnect_core::identity::IdentityKeyPair;
use ticonnect_net::transport::{DialTarget, Transport, TransportConfig, TrustLookup};

struct StaticTrust(HashMap<String, Option<NodeId>>);

impl TrustLookup for StaticTrust {
    fn is_trusted(&self, fingerprint: &str) -> bool {
        self.0.contains_key(fingerprint)
    }

    fn node_id_for_fingerprint(&self, fingerprint: &str) -> Option<NodeId> {
        self.0.get(fingerprint).copied().flatten()
    }
}

struct Node {
    transport: Transport,
    node_id: NodeId,
    fingerprint: Fingerprint,
    address: SocketAddr,
}

#[tokio::test]
async fn matching_node_id_is_accepted() {
    let (first, second) = trusted_pair().await;

    first
        .transport
        .connect(target(&second))
        .await
        .expect("trusted identity connects");

    wait_until(|| first.transport.session_count() == 1 && second.transport.session_count() == 1)
        .await;

    first.transport.shutdown().await;
    second.transport.shutdown().await;
}

#[tokio::test]
async fn trusted_peer_claiming_another_node_id_is_rejected() {
    let client_seed = [81_u8; 32];
    let server_seed = [82_u8; 32];
    let client_fingerprint = fingerprint(client_seed);
    let server_fingerprint = fingerprint(server_seed);
    let client_real_node_id = NodeId::generate();
    let server_node_id = NodeId::generate();

    let server = node(
        server_seed,
        server_node_id,
        HashMap::from([(client_fingerprint.to_hex(), Some(client_real_node_id))]),
    )
    .await;
    let attacker = node(
        client_seed,
        NodeId::generate(),
        HashMap::from([(server_fingerprint.to_hex(), Some(server_node_id))]),
    )
    .await;

    let result = attacker.transport.connect(target(&server)).await;
    assert!(result.is_err(), "forged node id must be rejected");

    tokio::time::sleep(Duration::from_millis(100)).await;
    assert_eq!(server.transport.session_count(), 0);

    attacker.transport.shutdown().await;
    server.transport.shutdown().await;
}

#[tokio::test]
async fn peer_claiming_a_different_node_id_is_rejected_when_we_dial() {
    let client_seed = [87_u8; 32];
    let server_seed = [88_u8; 32];
    let client_fingerprint = fingerprint(client_seed);
    let server_fingerprint = fingerprint(server_seed);
    let client_node_id = NodeId::generate();
    let server_real_node_id = NodeId::generate();
    let wrong_server_node_id = NodeId::generate();

    let server = node(
        server_seed,
        server_real_node_id,
        HashMap::from([(client_fingerprint.to_hex(), Some(client_node_id))]),
    )
    .await;
    let client = node(
        client_seed,
        client_node_id,
        HashMap::from([(server_fingerprint.to_hex(), Some(wrong_server_node_id))]),
    )
    .await;

    let target = DialTarget::new(server.address, server.fingerprint);
    let result = client.transport.connect(target).await;
    assert!(result.is_err(), "outbound claimed node id must be rejected");

    tokio::time::sleep(Duration::from_millis(100)).await;
    assert_eq!(client.transport.session_count(), 0);

    client.transport.shutdown().await;
    server.transport.shutdown().await;
}

#[tokio::test]
async fn trusted_fingerprint_without_cached_node_id_is_rejected() {
    let client_seed = [83_u8; 32];
    let server_seed = [84_u8; 32];
    let client_fingerprint = fingerprint(client_seed);
    let server_fingerprint = fingerprint(server_seed);
    let client_node_id = NodeId::generate();
    let server_node_id = NodeId::generate();

    let server = node(
        server_seed,
        server_node_id,
        HashMap::from([(client_fingerprint.to_hex(), None)]),
    )
    .await;
    let client = node(
        client_seed,
        client_node_id,
        HashMap::from([(server_fingerprint.to_hex(), Some(server_node_id))]),
    )
    .await;

    let result = client.transport.connect(target(&server)).await;
    assert!(result.is_err(), "missing node id must fail closed");

    tokio::time::sleep(Duration::from_millis(100)).await;
    assert_eq!(server.transport.session_count(), 0);

    client.transport.shutdown().await;
    server.transport.shutdown().await;
}

async fn trusted_pair() -> (Node, Node) {
    let first_seed = [85_u8; 32];
    let second_seed = [86_u8; 32];
    let first_fingerprint = fingerprint(first_seed);
    let second_fingerprint = fingerprint(second_seed);
    let first_node_id = NodeId::generate();
    let second_node_id = NodeId::generate();

    let first = node(
        first_seed,
        first_node_id,
        HashMap::from([(second_fingerprint.to_hex(), Some(second_node_id))]),
    )
    .await;
    let second = node(
        second_seed,
        second_node_id,
        HashMap::from([(first_fingerprint.to_hex(), Some(first_node_id))]),
    )
    .await;
    (first, second)
}

fn fingerprint(seed: [u8; 32]) -> Fingerprint {
    Fingerprint::of(&IdentityKeyPair::from_secret_bytes(seed).verifying_key())
}

async fn node(seed: [u8; 32], node_id: NodeId, trusted: HashMap<String, Option<NodeId>>) -> Node {
    let identity = IdentityKeyPair::from_secret_bytes(seed);
    let fingerprint = Fingerprint::of(&identity.verifying_key());
    let transport = Transport::bind(TransportConfig {
        identity,
        node_id,
        fingerprint,
        bind_addr: SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 0),
        trust: Arc::new(StaticTrust(trusted)),
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
