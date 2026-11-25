use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::Arc;
use std::time::Duration;

use ticonnect_core::fingerprint::Fingerprint;
use ticonnect_core::id::NodeId;
use ticonnect_core::identity::IdentityKeyPair;
use ticonnect_net::transport::{
    DialTarget, Transport, TransportConfig, TransportEvent, TrustLookup,
};
use ticonnect_net::trust::{
    EndpointError, InitiatorRequest, ResponderRequest, TrustOutcome, TrustUser, run_initiator,
    run_responder,
};

struct NoTrust;

impl TrustLookup for NoTrust {
    fn is_trusted(&self, _fingerprint: &str) -> bool {
        false
    }

    fn node_id_for_fingerprint(&self, _fingerprint: &str) -> Option<NodeId> {
        None
    }
}

struct Node {
    transport: Transport,
    user: TrustUser,
    address: SocketAddr,
}

#[tokio::test]
async fn token_trust_completes_over_quic() {
    let first = node([91_u8; 32]).await;
    let second = node([92_u8; 32]).await;
    let token = [7_u8; 32];

    let (initiator, responder) = trust(&first, &second, token, token).await;

    assert_eq!(initiator.expect("initiator").user, second.user);
    assert_eq!(responder.expect("responder").user, first.user);

    first.transport.shutdown().await;
    second.transport.shutdown().await;
}

#[tokio::test]
async fn wrong_token_fails_the_confirmation() {
    let first = node([95_u8; 32]).await;
    let second = node([96_u8; 32]).await;

    let (initiator, responder) = trust(&first, &second, [7_u8; 32], [8_u8; 32]).await;

    assert!(initiator.is_err() || responder.is_err());

    first.transport.shutdown().await;
    second.transport.shutdown().await;
}

async fn trust(
    initiator: &Node,
    responder: &Node,
    initiator_token: [u8; 32],
    responder_token: [u8; 32],
) -> (
    Result<TrustOutcome, EndpointError>,
    Result<TrustOutcome, EndpointError>,
) {
    let mut events = responder.transport.subscribe();
    let target = DialTarget::new(responder.address, responder.user.fingerprint)
        .with_node_id(responder.user.node_id);
    let local = initiator.user.clone();
    let expected = responder.user.clone();
    let initiator_transport = initiator.transport.clone();

    let initiator_task = tokio::spawn(async move {
        let connection = initiator_transport
            .connect_trust(target)
            .await
            .expect("trust connection");
        run_initiator(
            &connection,
            InitiatorRequest {
                local,
                expected,
                token: initiator_token,
            },
        )
        .await
    });

    let event = tokio::time::timeout(Duration::from_secs(5), events.recv())
        .await
        .expect("trust event timeout")
        .expect("trust event");
    let TransportEvent::TrustRequested {
        connection,
        fingerprint,
        ..
    } = event
    else {
        panic!("unexpected transport event");
    };
    assert_eq!(fingerprint, initiator.user.fingerprint);

    let responder_result = run_responder(
        &connection,
        ResponderRequest {
            local: responder.user.clone(),
            token: responder_token,
            certificate_fingerprint: initiator.user.certificate_fingerprint,
        },
    )
    .await;
    let initiator_result = initiator_task.await.expect("initiator task");
    connection.close(0u32.into(), b"done");

    (initiator_result, responder_result)
}

async fn node(seed: [u8; 32]) -> Node {
    let identity = IdentityKeyPair::from_secret_bytes(seed);
    let fingerprint = Fingerprint::of(&identity.verifying_key());
    let node_id = NodeId::generate();
    let transport = Transport::bind(TransportConfig {
        identity: identity.clone(),
        node_id,
        fingerprint,
        bind_addr: SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 0),
        trust: Arc::new(NoTrust),
    })
    .await
    .expect("bind transport");
    let address = transport.local_addr().expect("local address");
    Node {
        transport,
        user: TrustUser {
            node_id,
            fingerprint,
            certificate_fingerprint: fingerprint,
        },
        address,
    }
}
