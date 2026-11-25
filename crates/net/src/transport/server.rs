use quinn::crypto::rustls::HandshakeData;
use tokio::task::JoinHandle;

use super::session;
use super::{
    SESSION_REJECTED, Session, Transport, TransportError, TransportEvent, user_certificate,
};
use crate::tls::SESSION_ALPN;
use crate::trust::TRUST_ALPN;

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
    let (fingerprint, public_key) = user_certificate(&connection)?;
    let alpn = negotiated_alpn(&connection);

    if alpn.as_deref() == Some(TRUST_ALPN) {
        let _ = transport.events().send(TransportEvent::TrustRequested {
            fingerprint,
            address: connection.remote_address(),
            connection,
        });
        return Ok(());
    }

    if alpn.as_deref() == Some(crate::tls::PROFILE_ALPN) {
        let _ = transport.events().send(TransportEvent::ProfileRequested {
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

    let fingerprint_hex = fingerprint.to_hex();
    if !transport.trust().is_trusted(&fingerprint_hex) {
        transport.note_failure();
        connection.close(SESSION_REJECTED.into(), b"untrusted user");
        return Err(TransportError::Untrusted);
    }

    let Some(expected_remote_node_id) = transport.trust().node_id_for_fingerprint(&fingerprint_hex)
    else {
        transport.note_failure();
        connection.close(SESSION_REJECTED.into(), b"identity mismatch");
        return Err(TransportError::IdentityMismatch);
    };

    let user = match session::server_handshake(
        &connection,
        transport.node_id(),
        transport.expected_fingerprint(),
        fingerprint,
        expected_remote_node_id,
        public_key,
    )
    .await
    {
        Ok(user) => user,
        Err(TransportError::IdentityMismatch) => {
            transport.note_failure();
            return Err(TransportError::IdentityMismatch);
        }
        Err(error) => return Err(error),
    };
    let session = Session::new(connection.clone(), user.clone());
    transport.register(session);

    let disconnect_transport = transport.clone();
    let disconnect_user = user;
    tokio::spawn(async move {
        connection.closed().await;
        disconnect_transport.unregister(connection.stable_id(), &disconnect_user);
    });

    Ok(())
}

fn negotiated_alpn(connection: &quinn::Connection) -> Option<Vec<u8>> {
    connection
        .handshake_data()
        .and_then(|data| data.downcast::<HandshakeData>().ok())
        .and_then(|data| data.protocol)
}
