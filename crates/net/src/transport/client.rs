use std::time::Duration;

use tokio::time::timeout;

use super::session;
use super::{
    DialTarget, RECONNECT_ATTEMPTS, RECONNECT_BASE_DELAY, Session, Transport, TransportError,
    peer_certificate,
};
use crate::tls::ClientAlpn;

const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);

pub(crate) async fn connect(
    transport: &Transport,
    target: DialTarget,
) -> Result<Session, TransportError> {
    let result = connect_once(transport, target).await;
    if result.is_err() {
        transport.note_failure();
    }
    result
}

pub(crate) async fn connect_with_retry(
    transport: &Transport,
    target: DialTarget,
) -> Result<Session, TransportError> {
    let mut last_error = None;
    for attempt in 0..RECONNECT_ATTEMPTS {
        match connect_once(transport, target.clone()).await {
            Ok(session) => return Ok(session),
            Err(error) => {
                transport.note_failure();
                last_error = Some(error);
                if attempt + 1 < RECONNECT_ATTEMPTS {
                    let factor = 1_u32 << u32::try_from(attempt).unwrap_or_default();
                    let delay = RECONNECT_BASE_DELAY
                        .checked_mul(factor)
                        .unwrap_or(RECONNECT_BASE_DELAY);
                    tokio::time::sleep(delay).await;
                }
            }
        }
    }

    Err(last_error
        .unwrap_or_else(|| TransportError::Handshake("transport retry did not run".to_owned())))
}

pub(crate) async fn connect_pairing(
    transport: &Transport,
    target: DialTarget,
) -> Result<quinn::Connection, TransportError> {
    let result = open_connection(transport, &target, ClientAlpn::Pairing).await;
    if result.is_err() {
        transport.note_failure();
    }
    result
}

pub(crate) async fn connect_pairing_with_retry(
    transport: &Transport,
    target: DialTarget,
) -> Result<quinn::Connection, TransportError> {
    let mut last_error = None;
    for attempt in 0..RECONNECT_ATTEMPTS {
        match open_connection(transport, &target, ClientAlpn::Pairing).await {
            Ok(connection) => return Ok(connection),
            Err(error) => {
                transport.note_failure();
                last_error = Some(error);
                if attempt + 1 < RECONNECT_ATTEMPTS {
                    let factor = 1_u32 << u32::try_from(attempt).unwrap_or_default();
                    let delay = RECONNECT_BASE_DELAY
                        .checked_mul(factor)
                        .unwrap_or(RECONNECT_BASE_DELAY);
                    tokio::time::sleep(delay).await;
                }
            }
        }
    }

    Err(last_error
        .unwrap_or_else(|| TransportError::Handshake("pairing retry did not run".to_owned())))
}

async fn open_connection(
    transport: &Transport,
    target: &DialTarget,
    alpn: ClientAlpn,
) -> Result<quinn::Connection, TransportError> {
    let config = transport.client_config(target.fingerprint, alpn)?;
    let connecting = transport
        .endpoint()
        .connect_with(config, target.address, "ticonnect")
        .map_err(|error| TransportError::Handshake(error.to_string()))?;
    timeout(CONNECT_TIMEOUT, connecting)
        .await
        .map_err(|_| TransportError::Handshake("connection timed out".to_owned()))?
        .map_err(|error| TransportError::Handshake(error.to_string()))
}

async fn connect_once(
    transport: &Transport,
    target: DialTarget,
) -> Result<Session, TransportError> {
    let connection = open_connection(transport, &target, ClientAlpn::Session).await?;
    let (certificate_fingerprint, public_key) = peer_certificate(&connection)?;
    if certificate_fingerprint != target.fingerprint {
        return Err(TransportError::IdentityMismatch);
    }

    let peer = session::client_handshake(
        &connection,
        transport.node_id(),
        transport.expected_fingerprint(),
        target.fingerprint,
        target.node_id,
        public_key,
    )
    .await?;
    let session = Session::new(connection.clone(), peer.clone());
    transport.register(session.clone());

    let disconnect_transport = transport.clone();
    let disconnect_peer = peer;
    tokio::spawn(async move {
        connection.closed().await;
        disconnect_transport.unregister(connection.stable_id(), &disconnect_peer);
    });

    Ok(session)
}
