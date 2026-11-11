use std::net::SocketAddr;
use std::time::Duration;

use ticonnect_core::id::NodeId;
use ticonnect_net::discovery::broadcast::Beacon;
use ticonnect_net::discovery::candidate::Candidate;
use ticonnect_net::discovery::mdns::TxtRecord;
use ticonnect_net::discovery::{
    Discovery, DiscoveryConfig, DiscoveryError, DiscoveryEvent, LanDiscovery, PROTOCOL_VERSION,
};
use tokio::net::UdpSocket;
use tokio::time::timeout;

#[test]
fn txt_record_round_trips() {
    let record = TxtRecord::new(
        PROTOCOL_VERSION,
        NodeId::generate().to_string(),
        "ab".repeat(32),
        "Desk",
        0,
    );

    let decoded = TxtRecord::decode(&record.encode()).expect("decode TXT");
    assert_eq!(decoded, record);
}

#[test]
fn txt_record_rejects_missing_and_invalid_fields() {
    let node_id = NodeId::generate().to_string();
    let valid = TxtRecord::new(PROTOCOL_VERSION, &node_id, "ab".repeat(32), "Desk", 0).encode();

    let mut missing = valid.clone();
    missing.remove("fp");
    assert!(TxtRecord::decode(&missing).is_err());

    let mut bad_version = valid.clone();
    bad_version.insert("v".to_owned(), (PROTOCOL_VERSION + 1).to_string());
    assert!(matches!(
        TxtRecord::decode(&bad_version),
        Err(DiscoveryError::ProtocolVersion(_))
    ));

    let mut bad_fingerprint = valid;
    bad_fingerprint.insert("fp".to_owned(), "not-hex".to_owned());
    assert!(TxtRecord::decode(&bad_fingerprint).is_err());
}

#[test]
fn beacon_round_trips_and_rejects_malformed_packets() {
    let beacon = Beacon::new(
        PROTOCOL_VERSION,
        NodeId::generate().to_string(),
        "ab".repeat(32),
        0,
        "192.168.1.10",
        "Desk",
    );
    let bytes = beacon.encode().expect("encode beacon");
    assert_eq!(Beacon::decode(&bytes).expect("decode beacon"), beacon);

    for length in 0..bytes.len() {
        assert!(Beacon::decode(&bytes[..length]).is_err());
    }

    let mut bad_magic = bytes.clone();
    bad_magic[0] ^= 0xff;
    assert!(Beacon::decode(&bad_magic).is_err());

    let mut bad_version = bytes.clone();
    bad_version[8..10].copy_from_slice(&(PROTOCOL_VERSION + 1).to_be_bytes());
    assert!(matches!(
        Beacon::decode(&bad_version),
        Err(DiscoveryError::ProtocolVersion(_))
    ));

    let mut trailing = bytes;
    trailing.push(0);
    assert!(Beacon::decode(&trailing).is_err());
}

#[tokio::test]
async fn two_discovery_instances_find_each_other_over_loopback() {
    let port_a = free_port().await;
    let port_b = free_port().await;
    let node_a = NodeId::generate().to_string();
    let node_b = NodeId::generate().to_string();
    let fingerprint_a = fingerprint(1);
    let fingerprint_b = fingerprint(2);

    let config_a = DiscoveryConfig::new("A", &node_a, &fingerprint_a)
        .with_beacon_override(port_a, [SocketAddr::from(([127, 0, 0, 1], port_b))]);
    let config_b = DiscoveryConfig::new("B", &node_b, &fingerprint_b)
        .with_beacon_override(port_b, [SocketAddr::from(([127, 0, 0, 1], port_a))]);

    let mut config_a = config_a;
    config_a.mdns = false;
    config_a.beacon_interval = Duration::from_millis(200);
    let mut config_b = config_b;
    config_b.mdns = false;
    config_b.beacon_interval = Duration::from_millis(200);

    let discovery_a = LanDiscovery::new(config_a);
    let discovery_b = LanDiscovery::new(config_b);
    let mut events_a = discovery_a.subscribe();
    let mut events_b = discovery_b.subscribe();

    discovery_a.start().expect("start A");
    discovery_b.start().expect("start B");

    let candidate_a = wait_found(&mut events_a).await.expect("A finds B");
    let candidate_b = wait_found(&mut events_b).await.expect("B finds A");

    assert_eq!(candidate_a.fingerprint(), fingerprint_b);
    assert_eq!(candidate_a.node_id(), node_b);
    assert_eq!(candidate_b.fingerprint(), fingerprint_a);
    assert_eq!(candidate_b.node_id(), node_a);

    discovery_a.stop_and_wait().await;
    discovery_b.stop_and_wait().await;
}

#[tokio::test]
async fn self_candidates_are_ignored_and_duplicates_deduplicate() {
    let port = free_port().await;
    let node = NodeId::generate().to_string();
    let own_fingerprint = fingerprint(3);
    let mut config = DiscoveryConfig::new("Desk", &node, &own_fingerprint)
        .with_beacon_override(port, [SocketAddr::from(([127, 0, 0, 1], 1))]);
    config.mdns = false;
    config.beacon_interval = Duration::from_secs(30);

    let discovery = LanDiscovery::new(config);
    let mut events = discovery.subscribe();
    discovery.start().expect("start discovery");

    let sender = UdpSocket::bind("127.0.0.1:0").await.expect("sender socket");
    let destination = SocketAddr::from(([127, 0, 0, 1], port));

    let own = Beacon::new(
        PROTOCOL_VERSION,
        &node,
        &own_fingerprint,
        0,
        "127.0.0.1",
        "Desk",
    );
    sender
        .send_to(&own.encode().expect("own beacon"), destination)
        .await
        .expect("send own beacon");

    let other_node = NodeId::generate().to_string();
    let other_fingerprint = fingerprint(4);
    let other = Beacon::new(
        PROTOCOL_VERSION,
        &other_node,
        &other_fingerprint,
        0,
        "127.0.0.1",
        "Other",
    );
    let bytes = other.encode().expect("other beacon");
    sender
        .send_to(&bytes, destination)
        .await
        .expect("send other beacon");
    sender
        .send_to(&bytes, destination)
        .await
        .expect("send duplicate beacon");

    let candidate = wait_found(&mut events).await.expect("find candidate");
    assert_eq!(candidate.fingerprint(), other_fingerprint);
    assert_eq!(discovery.snapshot().len(), 1);
    assert!(discovery.status().dropped_packets == 0);

    discovery.stop_and_wait().await;
}

#[tokio::test]
async fn malformed_beacons_are_dropped_and_counted() {
    let port = free_port().await;
    let mut config = DiscoveryConfig::new("Desk", NodeId::generate().to_string(), fingerprint(5))
        .with_beacon_override(port, [SocketAddr::from(([127, 0, 0, 1], 1))]);
    config.mdns = false;
    config.beacon_interval = Duration::from_secs(30);

    let discovery = LanDiscovery::new(config);
    discovery.start().expect("start discovery");
    let sender = UdpSocket::bind("127.0.0.1:0").await.expect("sender socket");
    let destination = SocketAddr::from(([127, 0, 0, 1], port));

    sender
        .send_to(b"not a beacon", destination)
        .await
        .expect("send junk");

    timeout(Duration::from_secs(2), async {
        loop {
            if discovery.status().dropped_packets > 0 {
                return;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .expect("dropped packet count");

    discovery.stop_and_wait().await;
}

#[tokio::test]
async fn candidates_expire_after_three_beacon_intervals() {
    let port = free_port().await;
    let mut config = DiscoveryConfig::new("Desk", NodeId::generate().to_string(), fingerprint(6))
        .with_beacon_override(port, [SocketAddr::from(([127, 0, 0, 1], 1))]);
    config.mdns = false;
    config.beacon_interval = Duration::from_millis(100);

    let discovery = LanDiscovery::new(config);
    let mut events = discovery.subscribe();
    discovery.start().expect("start discovery");

    let sender = UdpSocket::bind("127.0.0.1:0").await.expect("sender socket");
    let destination = SocketAddr::from(([127, 0, 0, 1], port));
    let other = Beacon::new(
        PROTOCOL_VERSION,
        NodeId::generate().to_string(),
        fingerprint(7),
        0,
        "127.0.0.1",
        "Other",
    );
    sender
        .send_to(&other.encode().expect("beacon"), destination)
        .await
        .expect("send beacon");

    wait_found(&mut events).await.expect("found candidate");

    let lost = timeout(Duration::from_secs(4), async {
        loop {
            match events.recv().await {
                Ok(DiscoveryEvent::Lost(fingerprint)) => return fingerprint,
                Ok(DiscoveryEvent::Found(_)) => {}
                Err(error) => panic!("event stream closed: {error}"),
            }
        }
    })
    .await
    .expect("candidate expiry");

    assert_eq!(lost, other.fingerprint);
    assert!(discovery.snapshot().is_empty());

    discovery.stop_and_wait().await;
}

async fn wait_found(
    events: &mut tokio::sync::broadcast::Receiver<DiscoveryEvent>,
) -> Option<Candidate> {
    timeout(Duration::from_secs(3), async {
        loop {
            match events.recv().await {
                Ok(DiscoveryEvent::Found(candidate)) => return candidate,
                Ok(DiscoveryEvent::Lost(_)) => {}
                Err(error) => panic!("event stream closed: {error}"),
            }
        }
    })
    .await
    .ok()
}

async fn free_port() -> u16 {
    UdpSocket::bind("127.0.0.1:0")
        .await
        .expect("free port")
        .local_addr()
        .expect("local addr")
        .port()
}

fn fingerprint(seed: u8) -> String {
    format!("{seed:02x}").repeat(32)
}
