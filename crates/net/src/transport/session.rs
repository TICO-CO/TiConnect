use std::net::SocketAddr;
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use ticonnect_core::fingerprint::Fingerprint;
use ticonnect_core::id::NodeId;
use ticonnect_core::wire::{Frame, FrameKind, HandshakeFrame, MAX_CONTROL_FRAME};
use tokio::time::timeout;

use super::TransportError;

const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PeerInfo {
    pub node_id: NodeId,
    pub fingerprint: Fingerprint,
    pub address: SocketAddr,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionState {
    Connecting,
    Connected,
    Disconnected,
    Failed,
}

#[derive(Clone)]
pub struct Session {
    connection: quinn::Connection,
    peer: PeerInfo,
    state: Arc<Mutex<SessionState>>,
}

impl Session {
    pub(crate) fn new(connection: quinn::Connection, peer: PeerInfo) -> Self {
        Self {
            connection,
            peer,
            state: Arc::new(Mutex::new(SessionState::Connected)),
        }
    }

    pub fn state(&self) -> SessionState {
        *self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    pub fn peer(&self) -> &PeerInfo {
        &self.peer
    }

    pub fn connection(&self) -> &quinn::Connection {
        &self.connection
    }

    pub fn close(&self, code: u32, reason: &[u8]) {
        *self.state.lock().unwrap_or_else(PoisonError::into_inner) = SessionState::Disconnected;
        self.connection.close(code.into(), reason);
    }
}

impl std::fmt::Debug for Session {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Session")
            .field("peer", &self.peer)
            .field("state", &self.state())
            .field("stable_id", &self.connection.stable_id())
            .finish()
    }
}

pub(crate) async fn client_handshake(
    connection: &quinn::Connection,
    node_id: NodeId,
    fingerprint: Fingerprint,
    expected: Fingerprint,
    expected_node_id: Option<NodeId>,
) -> Result<PeerInfo, TransportError> {
    let (mut send, mut recv) = timeout(HANDSHAKE_TIMEOUT, connection.open_bi())
        .await
        .map_err(|_| TransportError::Handshake("open stream timed out".to_owned()))?
        .map_err(|error| TransportError::Handshake(error.to_string()))?;

    let handshake = HandshakeFrame::new(node_id, fingerprint);
    write_frame(
        &mut send,
        &Frame::new(FrameKind::Handshake, handshake.encode())?,
    )
    .await?;

    let frame = read_frame(&mut recv).await?;
    if frame.kind() != FrameKind::Handshake {
        return Err(TransportError::Handshake(
            "expected handshake frame".to_owned(),
        ));
    }
    let peer = HandshakeFrame::decode(frame.payload())?;
    if peer.fingerprint != expected {
        return Err(TransportError::IdentityMismatch);
    }
    if let Some(expected_node_id) = expected_node_id
        && peer.node_id != expected_node_id
    {
        return Err(TransportError::IdentityMismatch);
    }

    Ok(PeerInfo {
        node_id: peer.node_id,
        fingerprint: peer.fingerprint,
        address: connection.remote_address(),
    })
}

pub(crate) async fn server_handshake(
    connection: &quinn::Connection,
    node_id: NodeId,
    fingerprint: Fingerprint,
    certificate_fingerprint: Fingerprint,
) -> Result<PeerInfo, TransportError> {
    let (mut send, mut recv) = timeout(HANDSHAKE_TIMEOUT, connection.accept_bi())
        .await
        .map_err(|_| TransportError::Handshake("accept stream timed out".to_owned()))?
        .map_err(|error| TransportError::Handshake(error.to_string()))?;

    let frame = read_frame(&mut recv).await?;
    if frame.kind() != FrameKind::Handshake {
        return Err(TransportError::Handshake(
            "expected handshake frame".to_owned(),
        ));
    }
    let peer = HandshakeFrame::decode(frame.payload())?;
    if peer.fingerprint != certificate_fingerprint {
        return Err(TransportError::IdentityMismatch);
    }

    let handshake = HandshakeFrame::new(node_id, fingerprint);
    write_frame(
        &mut send,
        &Frame::new(FrameKind::Handshake, handshake.encode())?,
    )
    .await?;

    Ok(PeerInfo {
        node_id: peer.node_id,
        fingerprint: peer.fingerprint,
        address: connection.remote_address(),
    })
}

async fn write_frame(send: &mut quinn::SendStream, frame: &Frame) -> Result<(), TransportError> {
    let bytes = frame.encode();
    let length = u32::try_from(bytes.len())
        .map_err(|_| TransportError::Handshake("frame is too large".to_owned()))?;
    send.write_all(&length.to_be_bytes())
        .await
        .map_err(|error| TransportError::Stream(error.to_string()))?;
    send.write_all(&bytes)
        .await
        .map_err(|error| TransportError::Stream(error.to_string()))?;
    send.finish()
        .map_err(|error| TransportError::Stream(error.to_string()))?;
    Ok(())
}

async fn read_frame(recv: &mut quinn::RecvStream) -> Result<Frame, TransportError> {
    let mut length = [0_u8; 4];
    timeout(HANDSHAKE_TIMEOUT, recv.read_exact(&mut length))
        .await
        .map_err(|_| TransportError::Handshake("read timed out".to_owned()))?
        .map_err(|error| TransportError::Stream(error.to_string()))?;
    let length = u32::from_be_bytes(length) as usize;
    if length > MAX_CONTROL_FRAME {
        return Err(TransportError::Handshake("frame is too large".to_owned()));
    }

    let mut bytes = vec![0_u8; length];
    timeout(HANDSHAKE_TIMEOUT, recv.read_exact(&mut bytes))
        .await
        .map_err(|_| TransportError::Handshake("read timed out".to_owned()))?
        .map_err(|error| TransportError::Stream(error.to_string()))?;
    Ok(Frame::decode(&bytes)?)
}
