use std::net::SocketAddr;
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use ticonnect_core::fingerprint::Fingerprint;
use ticonnect_core::id::NodeId;
use ticonnect_core::wire::{Frame, FrameKind, HandshakeFrame, MAX_WIRE_FRAME};
use tokio::time::timeout;

use super::TransportError;

const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(5);
pub const STREAM_TIMEOUT: Duration = Duration::from_secs(30);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PeerInfo {
    pub node_id: NodeId,
    pub fingerprint: Fingerprint,
    pub public_key: [u8; 32],
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

    pub async fn open_uni(&self) -> Result<quinn::SendStream, TransportError> {
        self.connection
            .open_uni()
            .await
            .map_err(|error| TransportError::Stream(error.to_string()))
    }

    pub async fn open_bi(&self) -> Result<(quinn::SendStream, quinn::RecvStream), TransportError> {
        self.connection
            .open_bi()
            .await
            .map_err(|error| TransportError::Stream(error.to_string()))
    }

    pub async fn accept_uni(&self) -> Result<quinn::RecvStream, TransportError> {
        self.connection
            .accept_uni()
            .await
            .map_err(|error| TransportError::Stream(error.to_string()))
    }

    pub async fn accept_bi(
        &self,
    ) -> Result<(quinn::SendStream, quinn::RecvStream), TransportError> {
        self.connection
            .accept_bi()
            .await
            .map_err(|error| TransportError::Stream(error.to_string()))
    }

    pub fn send_datagram(&self, data: &[u8]) -> Result<(), TransportError> {
        self.connection
            .send_datagram(bytes::Bytes::copy_from_slice(data))
            .map_err(|error| TransportError::Stream(error.to_string()))
    }

    pub async fn read_datagram(&self) -> Result<bytes::Bytes, TransportError> {
        self.connection
            .read_datagram()
            .await
            .map_err(|error| TransportError::Stream(error.to_string()))
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
    public_key: [u8; 32],
) -> Result<PeerInfo, TransportError> {
    let (mut send, mut recv) = timeout(HANDSHAKE_TIMEOUT, connection.open_bi())
        .await
        .map_err(|_| TransportError::Handshake("open stream timed out".to_owned()))?
        .map_err(|error| TransportError::Handshake(error.to_string()))?;

    let handshake = HandshakeFrame::new(node_id, fingerprint);
    write_frame_with_timeout(
        &mut send,
        &Frame::new(FrameKind::Handshake, handshake.encode())?,
        HANDSHAKE_TIMEOUT,
        true,
    )
    .await?;

    let frame = read_frame_with_timeout(&mut recv, HANDSHAKE_TIMEOUT).await?;
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
        public_key,
        address: connection.remote_address(),
    })
}

pub(crate) async fn server_handshake(
    connection: &quinn::Connection,
    node_id: NodeId,
    fingerprint: Fingerprint,
    certificate_fingerprint: Fingerprint,
    public_key: [u8; 32],
) -> Result<PeerInfo, TransportError> {
    let (mut send, mut recv) = timeout(HANDSHAKE_TIMEOUT, connection.accept_bi())
        .await
        .map_err(|_| TransportError::Handshake("accept stream timed out".to_owned()))?
        .map_err(|error| TransportError::Handshake(error.to_string()))?;

    let frame = read_frame_with_timeout(&mut recv, HANDSHAKE_TIMEOUT).await?;
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
    write_frame_with_timeout(
        &mut send,
        &Frame::new(FrameKind::Handshake, handshake.encode())?,
        HANDSHAKE_TIMEOUT,
        true,
    )
    .await?;

    Ok(PeerInfo {
        node_id: peer.node_id,
        fingerprint: peer.fingerprint,
        public_key,
        address: connection.remote_address(),
    })
}

pub async fn write_frame(
    send: &mut quinn::SendStream,
    frame: &Frame,
) -> Result<(), TransportError> {
    write_frame_with_timeout(send, frame, STREAM_TIMEOUT, true).await
}

pub async fn write_stream_frame(
    send: &mut quinn::SendStream,
    frame: &Frame,
) -> Result<(), TransportError> {
    write_frame_with_timeout(send, frame, STREAM_TIMEOUT, false).await
}

pub async fn read_frame(recv: &mut quinn::RecvStream) -> Result<Frame, TransportError> {
    read_frame_with_timeout(recv, STREAM_TIMEOUT).await
}

async fn write_frame_with_timeout(
    send: &mut quinn::SendStream,
    frame: &Frame,
    timeout_duration: Duration,
    finish: bool,
) -> Result<(), TransportError> {
    let bytes = frame.encode();
    let length = u32::try_from(bytes.len())
        .map_err(|_| TransportError::Handshake("frame is too large".to_owned()))?;
    timeout(timeout_duration, send.write_all(&length.to_be_bytes()))
        .await
        .map_err(|_| TransportError::Stream("write timed out".to_owned()))?
        .map_err(|error| TransportError::Stream(error.to_string()))?;
    timeout(timeout_duration, send.write_all(&bytes))
        .await
        .map_err(|_| TransportError::Stream("write timed out".to_owned()))?
        .map_err(|error| TransportError::Stream(error.to_string()))?;
    if finish {
        send.finish()
            .map_err(|error| TransportError::Stream(error.to_string()))?;
    }
    Ok(())
}

async fn read_frame_with_timeout(
    recv: &mut quinn::RecvStream,
    timeout_duration: Duration,
) -> Result<Frame, TransportError> {
    let mut length = [0_u8; 4];
    timeout(timeout_duration, recv.read_exact(&mut length))
        .await
        .map_err(|_| TransportError::Handshake("read timed out".to_owned()))?
        .map_err(|error| TransportError::Stream(error.to_string()))?;
    let length = u32::from_be_bytes(length) as usize;
    if length > MAX_WIRE_FRAME {
        return Err(TransportError::Handshake("frame is too large".to_owned()));
    }

    let mut bytes = vec![0_u8; length];
    timeout(timeout_duration, recv.read_exact(&mut bytes))
        .await
        .map_err(|_| TransportError::Handshake("read timed out".to_owned()))?
        .map_err(|error| TransportError::Stream(error.to_string()))?;
    Ok(Frame::decode(&bytes)?)
}
