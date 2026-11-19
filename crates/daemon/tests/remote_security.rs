mod support;

use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::Arc;
use std::sync::atomic::{AtomicI64, Ordering};
use std::time::Duration;

use support::{TestNode, connect_pair, wait_true};
use ticonnect_core::fingerprint::Fingerprint;
use ticonnect_core::id::NodeId;
use ticonnect_core::identity::IdentityKeyPair;
use ticonnect_core::remote::{InputKind, InputPacket, RemoteSignalFrame};
use ticonnect_daemon::remote::{RemoteError, RemoteEventMessage, state::RemoteSessionId};
use ticonnect_daemon::storage::{NewUser, UserPolicy};
use ticonnect_net::transport::{DialTarget, Transport, TransportConfig, UserInfo};

#[tokio::test]
async fn trusted_peer_claiming_another_node_id_cannot_connect() {
    let host = TestNode::start("remote-forged-host").await;
    let attacker_key = IdentityKeyPair::from_secret_bytes([91_u8; 32]);
    let attacker_fingerprint = Fingerprint::of(&attacker_key.verifying_key());
    let attacker_real_id = NodeId::generate();

    host.trust
        .commit(NewUser {
            id: attacker_real_id.to_string(),
            fingerprint: attacker_fingerprint.to_hex(),
            public_key: None,
            display_name: Some("Attacker".to_owned()),
            policy: UserPolicy::Accept,
            trusted_at: 1,
            last_seen: None,
        })
        .await
        .expect("trust attacker");

    let attacker = Transport::bind(TransportConfig {
        identity: attacker_key,
        node_id: host.node_id(),
        fingerprint: attacker_fingerprint,
        bind_addr: SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 0),
        trust: host.trust.clone(),
    })
    .await
    .expect("attacker transport");

    let target = DialTarget::new(
        host.transport.local_addr().expect("host address"),
        host.identity.fingerprint(),
    )
    .with_node_id(host.node_id());
    assert!(attacker.connect(target).await.is_err());

    tokio::time::sleep(Duration::from_millis(100)).await;
    assert_eq!(host.transport.session_count(), 0);

    attacker.shutdown().await;
    host.shutdown().await;
}

#[tokio::test]
async fn forged_node_id_request_is_rejected_before_consent() {
    let host = TestNode::start("remote-forged-request").await;
    let victim = TestNode::start("remote-forged-victim").await;
    let attacker_key = IdentityKeyPair::from_secret_bytes([92_u8; 32]);
    let attacker_fingerprint = Fingerprint::of(&attacker_key.verifying_key());
    let attacker_id = NodeId::generate();

    host.trust
        .commit(NewUser {
            id: attacker_id.to_string(),
            fingerprint: attacker_fingerprint.to_hex(),
            public_key: None,
            display_name: Some("Attacker".to_owned()),
            policy: UserPolicy::Accept,
            trusted_at: 1,
            last_seen: None,
        })
        .await
        .expect("trust attacker");

    let forged = UserInfo {
        node_id: victim.node_id(),
        fingerprint: attacker_fingerprint,
        public_key: [0; 32],
        address: SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 1),
    };
    let session_id = RemoteSessionId::generate();

    let result = host
        .remote
        .handle_signal(
            forged,
            RemoteSignalFrame::Request {
                session_id: session_id.as_bytes(),
            },
        )
        .await;

    assert!(matches!(result, Err(RemoteError::IdentityMismatch)));
    assert!(host.remote.snapshot().is_none());

    victim.shutdown().await;
    host.shutdown().await;
}

#[tokio::test]
async fn input_while_requested_is_rejected() {
    let first = TestNode::start("remote-input-pending-first").await;
    let second = TestNode::start("remote-input-pending-second").await;
    connect_pair(&first, &second).await;
    let viewer = first
        .remote
        .request_session(&second.node_id().to_string())
        .await
        .expect("request");
    wait_true(Duration::from_secs(5), || async {
        second.remote.snapshot().is_some()
    })
    .await
    .expect("host request");
    let user = second
        .transport
        .session_by_node_id(first.node_id())
        .expect("session")
        .user()
        .clone();
    let session_id: RemoteSessionId = viewer.session_id.parse().expect("session id");
    let packet = InputPacket {
        session_id: session_id.as_bytes(),
        kind: InputKind::Motion,
        x: 10,
        y: 20,
        pressed: false,
    };

    assert!(matches!(
        second.remote.inject_input(user, packet),
        Err(RemoteError::Inactive)
    ));

    first.shutdown().await;
    second.shutdown().await;
}

#[tokio::test]
async fn input_from_wrong_fingerprint_is_rejected() {
    let first = TestNode::start("remote-input-wrong-first").await;
    let second = TestNode::start("remote-input-wrong-second").await;
    connect_pair(&first, &second).await;
    let viewer = first
        .remote
        .request_session(&second.node_id().to_string())
        .await
        .expect("request");
    wait_true(Duration::from_secs(5), || async {
        second.remote.snapshot().is_some()
    })
    .await
    .expect("host request");
    second
        .remote
        .accept_session(&viewer.session_id)
        .await
        .expect("consent");
    assert_eq!(
        second.remote.snapshot().expect("host session").state,
        "active"
    );

    let wrong_key = IdentityKeyPair::from_secret_bytes([93_u8; 32]);
    let user = UserInfo {
        node_id: first.node_id(),
        fingerprint: Fingerprint::of(&wrong_key.verifying_key()),
        public_key: [0; 32],
        address: SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 2),
    };
    let session_id: RemoteSessionId = viewer.session_id.parse().expect("session id");
    let packet = InputPacket {
        session_id: session_id.as_bytes(),
        kind: InputKind::Motion,
        x: 10,
        y: 20,
        pressed: false,
    };

    assert!(matches!(
        second.remote.inject_input(user, packet),
        Err(RemoteError::Inactive)
    ));

    first.shutdown().await;
    second.shutdown().await;
}

#[tokio::test]
async fn forged_disconnect_does_not_end_session() {
    let first = TestNode::start("remote-disconnect-first").await;
    let second = TestNode::start("remote-disconnect-second").await;
    connect_pair(&first, &second).await;
    let viewer = first
        .remote
        .request_session(&second.node_id().to_string())
        .await
        .expect("request");
    wait_true(Duration::from_secs(5), || async {
        second.remote.snapshot().is_some()
    })
    .await
    .expect("host request");
    second
        .remote
        .accept_session(&viewer.session_id)
        .await
        .expect("consent");

    let wrong_key = IdentityKeyPair::from_secret_bytes([94_u8; 32]);
    let forged = UserInfo {
        node_id: first.node_id(),
        fingerprint: Fingerprint::of(&wrong_key.verifying_key()),
        public_key: [0; 32],
        address: SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 3),
    };
    second.remote.handle_disconnect(&forged);

    assert_eq!(
        second.remote.snapshot().expect("host session").state,
        "active"
    );

    first.shutdown().await;
    second.shutdown().await;
}

#[tokio::test]
async fn forged_start_signal_is_rejected() {
    let first = TestNode::start("remote-start-first").await;
    let second = TestNode::start("remote-start-second").await;
    connect_pair(&first, &second).await;
    let viewer = first
        .remote
        .request_session(&second.node_id().to_string())
        .await
        .expect("request");
    wait_true(Duration::from_secs(5), || async {
        second.remote.snapshot().is_some()
    })
    .await
    .expect("host request");
    let user = second
        .transport
        .session_by_node_id(first.node_id())
        .expect("session")
        .user()
        .clone();
    let session_id: RemoteSessionId = viewer.session_id.parse().expect("session id");

    let result = second
        .remote
        .handle_signal(
            user,
            RemoteSignalFrame::Start {
                session_id: session_id.as_bytes(),
            },
        )
        .await;

    assert!(matches!(
        result,
        Err(RemoteError::State(
            ticonnect_daemon::remote::state::RemoteStateError::InvalidSession
        ))
    ));
    assert_eq!(
        second.remote.snapshot().expect("host session").state,
        "requested"
    );

    first.shutdown().await;
    second.shutdown().await;
}

#[tokio::test]
async fn requested_session_expires_without_accept() {
    let clock = Arc::new(AtomicI64::new(1_000_000));
    let first_clock = {
        let clock = clock.clone();
        Arc::new(move || clock.load(Ordering::Relaxed)) as Arc<dyn Fn() -> i64 + Send + Sync>
    };
    let second_clock = {
        let clock = clock.clone();
        Arc::new(move || clock.load(Ordering::Relaxed)) as Arc<dyn Fn() -> i64 + Send + Sync>
    };
    let first = TestNode::start_with_clock("remote-expire-first", first_clock).await;
    let second = TestNode::start_with_clock("remote-expire-second", second_clock).await;
    connect_pair(&first, &second).await;
    let mut events = second.remote.subscribe();
    let viewer = first
        .remote
        .request_session(&second.node_id().to_string())
        .await
        .expect("request");
    wait_true(Duration::from_secs(5), || async {
        second.remote.snapshot().is_some()
    })
    .await
    .expect("host request");
    assert_eq!(
        second.remote.snapshot().expect("host session").state,
        "requested"
    );

    clock.fetch_add(400_000, Ordering::Relaxed);
    second.remote.expire_sessions().await;

    assert!(second.remote.snapshot().is_none());
    let ended = tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            if let Ok(RemoteEventMessage::Ended { session_id, reason }) = events.recv().await
                && session_id == viewer.session_id
            {
                return reason;
            }
        }
    })
    .await
    .expect("ended event");
    assert_eq!(ended, "expired");

    wait_true(Duration::from_secs(5), || async {
        first.remote.snapshot().is_none()
    })
    .await
    .expect("viewer cleanup");

    first.shutdown().await;
    second.shutdown().await;
}
