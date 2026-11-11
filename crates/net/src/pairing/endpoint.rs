use std::str::FromStr;
use std::time::Duration;

use thiserror::Error;
use ticonnect_core::fingerprint::{Fingerprint, FingerprintError};
use ticonnect_core::id::NodeId;
use tokio::time::timeout;

use super::PAIRING_TIMEOUT_SECS;
use super::frames::{FrameError, PairingMessage, decode_frame, encode_frame};
use super::spake2::{
    Confirmation, PairingMode, PairingSecret, PairingTranscript, PeerIdentity, Role, Spake2Error,
    Spake2Session,
};
use ticonnect_core::wire::{MAX_PAIRING_FRAME, WireError};

const STEP_TIMEOUT: Duration = Duration::from_secs(PAIRING_TIMEOUT_SECS);

#[derive(Debug, Error)]
pub enum EndpointError {
    #[error("pairing connection failed: {0}")]
    Connection(String),

    #[error(transparent)]
    Frame(#[from] FrameError),

    #[error(transparent)]
    Spake2(#[from] Spake2Error),

    #[error(transparent)]
    Wire(#[from] WireError),

    #[error("pairing timed out")]
    Timeout,

    #[error("pairing hello is invalid")]
    InvalidHello,

    #[error("pairing mode does not match")]
    ModeMismatch,

    #[error("peer certificate fingerprint does not match")]
    CertificateMismatch,

    #[error("peer identity does not match")]
    IdentityMismatch,

    #[error("pairing confirmation does not match")]
    ConfirmationMismatch,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PairingPeer {
    pub node_id: NodeId,
    pub fingerprint: Fingerprint,
    pub certificate_fingerprint: Fingerprint,
}

impl PairingPeer {
    fn identity(&self) -> PeerIdentity {
        PeerIdentity {
            node_id: self.node_id.to_string(),
            fingerprint: self.fingerprint.to_hex(),
            certificate_fingerprint: self.certificate_fingerprint.to_hex(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PairingOutcome {
    pub peer: PairingPeer,
    pub mode: PairingMode,
}

#[derive(Debug, Clone)]
pub struct InitiatorRequest {
    pub local: PairingPeer,
    pub expected: PairingPeer,
    pub mode: PairingMode,
    pub secret: PairingSecret,
    pub token: Option<[u8; 32]>,
}

#[derive(Debug, Clone)]
pub struct ResponderRequest {
    pub local: PairingPeer,
    pub pin_secret: Option<PairingSecret>,
    pub qr_secret: Option<PairingSecret>,
    pub token: Option<[u8; 32]>,
    pub certificate_fingerprint: Fingerprint,
}

pub async fn run_initiator(
    connection: &quinn::Connection,
    request: InitiatorRequest,
) -> Result<PairingOutcome, EndpointError> {
    timeout(STEP_TIMEOUT, run_initiator_inner(connection, request))
        .await
        .map_err(|_| EndpointError::Timeout)?
}

pub async fn run_responder(
    connection: &quinn::Connection,
    request: ResponderRequest,
) -> Result<PairingOutcome, EndpointError> {
    timeout(STEP_TIMEOUT, run_responder_inner(connection, request))
        .await
        .map_err(|_| EndpointError::Timeout)?
}

async fn run_initiator_inner(
    connection: &quinn::Connection,
    request: InitiatorRequest,
) -> Result<PairingOutcome, EndpointError> {
    if request.mode == PairingMode::Qr && request.token.is_none() {
        return Err(EndpointError::InvalidHello);
    }

    let (mut send, mut recv) = connection
        .open_bi()
        .await
        .map_err(|error| EndpointError::Connection(error.to_string()))?;

    let hello = PairingMessage::Hello {
        node_id: request.local.node_id.to_string(),
        fingerprint: request.local.fingerprint.to_hex(),
        certificate_fingerprint: request.local.certificate_fingerprint.to_hex(),
        mode: request.mode.as_u8(),
    };
    write_message(&mut send, &hello).await?;

    let hello = expect_hello(read_message(&mut recv).await?)?;
    if hello.mode != request.mode.as_u8() {
        return Err(EndpointError::ModeMismatch);
    }
    let remote = hello_to_peer(hello)?;
    if remote.certificate_fingerprint != request.expected.certificate_fingerprint
        || remote.fingerprint != request.expected.fingerprint
    {
        return Err(EndpointError::IdentityMismatch);
    }
    if remote.node_id != request.expected.node_id {
        return Err(EndpointError::IdentityMismatch);
    }

    let transcript = PairingTranscript::new(
        request.mode,
        request.local.identity(),
        remote.identity(),
        request.token,
    );
    let hash = transcript.hash()?;
    let (session, message) = Spake2Session::start(Role::Initiator, &request.secret, &transcript)?;
    write_message(
        &mut send,
        &PairingMessage::Spake2 {
            message: message.clone(),
        },
    )
    .await?;

    let peer_message = expect_spake2(read_message(&mut recv).await?)?;
    let confirmation = session.finish(&peer_message, &hash)?;
    write_message(
        &mut send,
        &PairingMessage::Confirm {
            value: *confirmation.as_bytes(),
        },
    )
    .await?;

    let peer_confirmation = expect_confirm(read_message(&mut recv).await?)?;
    if !confirmation.matches(&Confirmation::from_bytes(peer_confirmation)) {
        return Err(EndpointError::ConfirmationMismatch);
    }
    let _ = send.finish();

    Ok(PairingOutcome {
        peer: remote,
        mode: request.mode,
    })
}

async fn run_responder_inner(
    connection: &quinn::Connection,
    request: ResponderRequest,
) -> Result<PairingOutcome, EndpointError> {
    let (mut send, mut recv) = connection
        .accept_bi()
        .await
        .map_err(|error| EndpointError::Connection(error.to_string()))?;

    let hello = expect_hello(read_message(&mut recv).await?)?;
    let mode = PairingMode::from_u8(hello.mode).ok_or(EndpointError::ModeMismatch)?;
    let (secret, token) = match mode {
        PairingMode::Pin => (
            request
                .pin_secret
                .clone()
                .ok_or(EndpointError::ModeMismatch)?,
            None,
        ),
        PairingMode::Qr => (
            request
                .qr_secret
                .clone()
                .ok_or(EndpointError::ModeMismatch)?,
            request.token,
        ),
    };
    if mode == PairingMode::Qr && token.is_none() {
        return Err(EndpointError::InvalidHello);
    }
    let remote = hello_to_peer(hello)?;
    if remote.certificate_fingerprint != request.certificate_fingerprint {
        return Err(EndpointError::CertificateMismatch);
    }

    let hello = PairingMessage::Hello {
        node_id: request.local.node_id.to_string(),
        fingerprint: request.local.fingerprint.to_hex(),
        certificate_fingerprint: request.local.certificate_fingerprint.to_hex(),
        mode: mode.as_u8(),
    };
    write_message(&mut send, &hello).await?;

    let transcript =
        PairingTranscript::new(mode, remote.identity(), request.local.identity(), token);
    let hash = transcript.hash()?;

    let peer_message = expect_spake2(read_message(&mut recv).await?)?;
    let (session, message) = Spake2Session::start(Role::Responder, &secret, &transcript)?;
    write_message(
        &mut send,
        &PairingMessage::Spake2 {
            message: message.clone(),
        },
    )
    .await?;

    let confirmation = session.finish(&peer_message, &hash)?;
    let peer_confirmation = expect_confirm(read_message(&mut recv).await?)?;
    if !confirmation.matches(&Confirmation::from_bytes(peer_confirmation)) {
        return Err(EndpointError::ConfirmationMismatch);
    }
    write_message(
        &mut send,
        &PairingMessage::Confirm {
            value: *confirmation.as_bytes(),
        },
    )
    .await?;
    let _ = send.finish();

    Ok(PairingOutcome { peer: remote, mode })
}

async fn write_message(
    send: &mut quinn::SendStream,
    message: &PairingMessage,
) -> Result<(), EndpointError> {
    let frame = encode_frame(message)?;
    let bytes = frame.encode();
    let length = u32::try_from(bytes.len())
        .map_err(|_| EndpointError::Connection("pairing frame is too large".to_owned()))?;
    send.write_all(&length.to_be_bytes())
        .await
        .map_err(|error| EndpointError::Connection(error.to_string()))?;
    send.write_all(&bytes)
        .await
        .map_err(|error| EndpointError::Connection(error.to_string()))?;
    Ok(())
}

async fn read_message(recv: &mut quinn::RecvStream) -> Result<PairingMessage, EndpointError> {
    let mut length = [0_u8; 4];
    recv.read_exact(&mut length)
        .await
        .map_err(|error| EndpointError::Connection(error.to_string()))?;
    let length = u32::from_be_bytes(length) as usize;
    if length > MAX_PAIRING_FRAME {
        return Err(EndpointError::InvalidHello);
    }
    let mut bytes = vec![0_u8; length];
    recv.read_exact(&mut bytes)
        .await
        .map_err(|error| EndpointError::Connection(error.to_string()))?;
    Ok(decode_frame(&ticonnect_core::wire::Frame::decode(&bytes)?)?)
}

fn expect_hello(message: PairingMessage) -> Result<HelloFields, EndpointError> {
    match message {
        PairingMessage::Hello {
            node_id,
            fingerprint,
            certificate_fingerprint,
            mode,
        } => Ok(HelloFields {
            node_id,
            fingerprint,
            certificate_fingerprint,
            mode,
        }),
        _ => Err(EndpointError::InvalidHello),
    }
}

fn hello_to_peer(hello: HelloFields) -> Result<PairingPeer, EndpointError> {
    let node_id = NodeId::from_str(&hello.node_id).map_err(|_| EndpointError::InvalidHello)?;
    let fingerprint = Fingerprint::from_str(&hello.fingerprint).map_err(peer_fingerprint_error)?;
    let certificate_fingerprint =
        Fingerprint::from_str(&hello.certificate_fingerprint).map_err(peer_fingerprint_error)?;
    if PairingMode::from_u8(hello.mode).is_none() {
        return Err(EndpointError::InvalidHello);
    }
    Ok(PairingPeer {
        node_id,
        fingerprint,
        certificate_fingerprint,
    })
}

fn expect_spake2(message: PairingMessage) -> Result<Vec<u8>, EndpointError> {
    match message {
        PairingMessage::Spake2 { message } => Ok(message),
        _ => Err(EndpointError::InvalidHello),
    }
}

fn expect_confirm(message: PairingMessage) -> Result<[u8; 32], EndpointError> {
    match message {
        PairingMessage::Confirm { value } => Ok(value),
        _ => Err(EndpointError::InvalidHello),
    }
}

fn peer_fingerprint_error(_: FingerprintError) -> EndpointError {
    EndpointError::InvalidHello
}

struct HelloFields {
    node_id: String,
    fingerprint: String,
    certificate_fingerprint: String,
    mode: u8,
}
