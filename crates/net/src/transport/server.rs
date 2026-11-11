use quinn::crypto::rustls::HandshakeData;
use quinn::rustls::pki_types::CertificateDer;
use ticonnect_core::fingerprint::Fingerprint;
use tokio::task::JoinHandle;

use super::session;
use super::{SESSION_REJECTED, Session, Transport, TransportError, TransportEvent};
use crate::pairing::PAIRING_ALPN;
use crate::tls::{SESSION_ALPN, validate_certificate};

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
    let fingerprint = peer_certificate_fingerprint(&connection)?;
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

fn peer_certificate_fingerprint(
    connection: &quinn::Connection,
) -> Result<Fingerprint, TransportError> {
    let identity = connection
        .peer_identity()
        .ok_or_else(|| TransportError::Handshake("peer certificate is missing".to_owned()))?;
    let certificates = identity
        .downcast::<Vec<CertificateDer<'static>>>()
        .map_err(|_| TransportError::Handshake("peer identity type is invalid".to_owned()))?;
    let certificate = certificates
        .first()
        .ok_or_else(|| TransportError::Handshake("peer certificate is missing".to_owned()))?;
    validate_certificate(certificate).map_err(TransportError::Tls)
}
