use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::Arc;
use std::time::Duration;

use ticonnect_core::fingerprint::Fingerprint;
use ticonnect_core::id::NodeId;
use ticonnect_core::identity::IdentityKeyPair;
use ticonnect_net::pairing::{
    InitiatorRequest, PairingMode, PairingPeer, PairingSecret, ResponderRequest, run_initiator,
    run_responder,
};
use ticonnect_net::transport::{
    DialTarget, Transport, TransportConfig, TransportEvent, TrustLookup,
};

struct NoTrust;

impl TrustLookup for NoTrust {
    fn is_trusted(&self, _fingerprint: &str) -> bool {
        false
    }
}

struct Node {
    transport: Transport,
    peer: PairingPeer,
    address: SocketAddr,
}

#[tokio::test]
async fn pin_pairing_completes_over_quic() {
    let first = node([91_u8; 32]).await;
    let second = node([92_u8; 32]).await;
    let secret = PairingSecret::Pin("123456".to_owned());

    let (initiator, responder) = pair(&first, &second, secret.clone(), secret, None, None).await;

    assert_eq!(initiator.expect("initiator").peer, second.peer);
    assert_eq!(responder.expect("responder").peer, first.peer);

    first.transport.shutdown().await;
    second.transport.shutdown().await;
}

#[tokio::test]
async fn qr_pairing_completes_over_quic() {
    let first = node([93_u8; 32]).await;
    let second = node([94_u8; 32]).await;
    let token = [7_u8; 32];

    let (initiator, responder) = pair(
        &first,
        &second,
        PairingSecret::Token(token),
        PairingSecret::Token(token),
        Some(PairingMode::Qr),
        Some(token),
    )
    .await;

    assert_eq!(initiator.expect("initiator").peer, second.peer);
    assert_eq!(responder.expect("responder").peer, first.peer);

    first.transport.shutdown().await;
    second.transport.shutdown().await;
}

#[tokio::test]
async fn wrong_pin_fails_the_confirmation() {
    let first = node([95_u8; 32]).await;
    let second = node([96_u8; 32]).await;
    let initiator_secret = PairingSecret::Pin("123456".to_owned());
    let responder_secret = PairingSecret::Pin("654321".to_owned());

    let (initiator, responder) = pair(
        &first,
        &second,
        initiator_secret,
        responder_secret,
        None,
        None,
    )
    .await;

    assert!(initiator.is_err() || responder.is_err());

    first.transport.shutdown().await;
    second.transport.shutdown().await;
}

async fn pair(
    initiator: &Node,
    responder: &Node,
    initiator_secret: PairingSecret,
    responder_secret: PairingSecret,
    mode: Option<PairingMode>,
    token: Option<[u8; 32]>,
) -> (
    Result<ticonnect_net::pairing::PairingOutcome, ticonnect_net::pairing::EndpointError>,
    Result<ticonnect_net::pairing::PairingOutcome, ticonnect_net::pairing::EndpointError>,
) {
    let mode = mode.unwrap_or(PairingMode::Pin);
    let mut events = responder.transport.subscribe();
    let target = DialTarget::new(responder.address, responder.peer.fingerprint)
        .with_node_id(responder.peer.node_id);
    let local = initiator.peer.clone();
    let expected = responder.peer.clone();
    let initiator_transport = initiator.transport.clone();

    let initiator_task = tokio::spawn(async move {
        let connection = initiator_transport
            .connect_pairing(target)
            .await
            .expect("pairing connection");
        run_initiator(
            &connection,
            InitiatorRequest {
                local,
                expected,
                mode,
                secret: initiator_secret,
                token,
            },
        )
        .await
    });

    let event = tokio::time::timeout(Duration::from_secs(5), events.recv())
        .await
        .expect("pairing event timeout")
        .expect("pairing event");
    let TransportEvent::PairingRequested {
        connection,
        fingerprint,
        ..
    } = event
    else {
        panic!("unexpected transport event");
    };
    assert_eq!(fingerprint, initiator.peer.fingerprint);

    let responder_result = run_responder(
        &connection,
        ResponderRequest {
            local: responder.peer.clone(),
            pin_secret: matches!(mode, PairingMode::Pin).then(|| responder_secret.clone()),
            qr_secret: matches!(mode, PairingMode::Qr).then(|| responder_secret.clone()),
            token,
            certificate_fingerprint: initiator.peer.certificate_fingerprint,
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
        peer: PairingPeer {
            node_id,
            fingerprint,
            certificate_fingerprint: fingerprint,
        },
        address,
    }
}
