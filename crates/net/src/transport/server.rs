use quinn::crypto::rustls::HandshakeData;
use tokio::task::JoinHandle;

use super::session;
use super::{
    SESSION_REJECTED, Session, Transport, TransportError, TransportEvent, peer_certificate,
};
use crate::pairing::PAIRING_ALPN;
use crate::tls::SESSION_ALPN;

pub(crate) fn spawn_accept_loop(transport: Transport) -> JoinHandle<()> {
    let endpoint = transport.endpoint().clone();
    tokio::spawn(async move {
        while let Some(incoming) = endpoint.accept().await {
            let transport = transport.clone();
            tokio::spawn(async move {
                if let Err(error) = handle_incoming(transport, incoming).await {
                    tracing::debug!(%error, "incoming transport connection rejected");
                }
            });
        }
    })
}

async fn handle_incoming(
    transport: Transport,
    incoming: quinn::Incoming,
) -> Result<(), TransportError> {
    let connection = incoming
        .await
        .map_err(|error| TransportError::Handshake(error.to_string()))?;
    let (fingerprint, public_key) = peer_certificate(&connection)?;
    let alpn = negotiated_alpn(&connection);

    if alpn.as_deref() == Some(PAIRING_ALPN) {
        let _ = transport.events().send(TransportEvent::PairingRequested {
            fingerprint,
            address: connection.remote_address(),
            connection,
        });
        return Ok(());
    }

    if alpn.as_deref() != Some(SESSION_ALPN) {
        transport.note_failure();
        connection.close(SESSION_REJECTED.into(), b"unsupported alpn");
        return Err(TransportError::Handshake(
            "unsupported application protocol".to_owned(),
        ));
    }

    if !transport.trust().is_trusted(&fingerprint.to_hex()) {
        transport.note_failure();
        connection.close(SESSION_REJECTED.into(), b"untrusted peer");
        return Err(TransportError::Untrusted);
    }

    let peer = session::server_handshake(
        &connection,
        transport.node_id(),
        transport.expected_fingerprint(),
        fingerprint,
        public_key,
    )
    .await?;
    let session = Session::new(connection.clone(), peer.clone());
    transport.register(session);

    let disconnect_transport = transport.clone();
    let disconnect_peer = peer;
    tokio::spawn(async move {
        connection.closed().await;
        disconnect_transport.unregister(connection.stable_id(), &disconnect_peer);
    });

    Ok(())
}

fn negotiated_alpn(connection: &quinn::Connection) -> Option<Vec<u8>> {
    connection
        .handshake_data()
        .and_then(|data| data.downcast::<HandshakeData>().ok())
        .and_then(|data| data.protocol)
}
