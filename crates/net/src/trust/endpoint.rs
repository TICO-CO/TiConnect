use std::str::FromStr;
use std::time::Duration;

use thiserror::Error;
use ticonnect_core::fingerprint::{Fingerprint, FingerprintError};
use ticonnect_core::id::NodeId;
use tokio::time::timeout;

use super::TRUST_TIMEOUT_SECS;
use super::frames::{FrameError, TrustMessage, decode_frame, encode_frame};
use super::spake2::{
    Confirmation, Role, Spake2Error, Spake2Session, TrustSecret, TrustTranscript, UserIdentity,
};
use ticonnect_core::wire::{MAX_TRUST_FRAME, WireError};

const STEP_TIMEOUT: Duration = Duration::from_secs(TRUST_TIMEOUT_SECS);

#[derive(Debug, Error)]
pub enum EndpointError {
    #[error("trust connection failed: {0}")]
    Connection(String),

    #[error(transparent)]
    Frame(#[from] FrameError),

    #[error(transparent)]
    Spake2(#[from] Spake2Error),

    #[error(transparent)]
    Wire(#[from] WireError),

    #[error("trust exchange timed out")]
    Timeout,

    #[error("trust hello is invalid")]
    InvalidHello,

    #[error("user certificate fingerprint does not match")]
    CertificateMismatch,

    #[error("user identity does not match")]
    IdentityMismatch,

    #[error("trust confirmation does not match")]
    ConfirmationMismatch,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrustUser {
    pub node_id: NodeId,
    pub fingerprint: Fingerprint,
    pub certificate_fingerprint: Fingerprint,
}

impl TrustUser {
    fn identity(&self) -> UserIdentity {
        UserIdentity {
            node_id: self.node_id.to_string(),
            fingerprint: self.fingerprint.to_hex(),
            certificate_fingerprint: self.certificate_fingerprint.to_hex(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrustOutcome {
    pub user: TrustUser,
}

#[derive(Debug, Clone)]
pub struct InitiatorRequest {
    pub local: TrustUser,
    pub expected: TrustUser,
    pub token: [u8; 32],
}

#[derive(Debug, Clone)]
pub struct ResponderRequest {
    pub local: TrustUser,
    pub token: [u8; 32],
    pub certificate_fingerprint: Fingerprint,
}

pub async fn run_initiator(
    connection: &quinn::Connection,
    request: InitiatorRequest,
) -> Result<TrustOutcome, EndpointError> {
    timeout(STEP_TIMEOUT, run_initiator_inner(connection, request))
        .await
        .map_err(|_| EndpointError::Timeout)?
}

pub async fn run_responder(
    connection: &quinn::Connection,
    request: ResponderRequest,
) -> Result<TrustOutcome, EndpointError> {
    timeout(STEP_TIMEOUT, run_responder_inner(connection, request))
        .await
        .map_err(|_| EndpointError::Timeout)?
}

async fn run_initiator_inner(
    connection: &quinn::Connection,
    request: InitiatorRequest,
) -> Result<TrustOutcome, EndpointError> {
    let (mut send, mut recv) = connection
        .open_bi()
        .await
        .map_err(|error| EndpointError::Connection(error.to_string()))?;

    let hello = TrustMessage::Hello {
        node_id: request.local.node_id.to_string(),
        fingerprint: request.local.fingerprint.to_hex(),
        certificate_fingerprint: request.local.certificate_fingerprint.to_hex(),
    };
    write_message(&mut send, &hello).await?;

    let hello = expect_hello(read_message(&mut recv).await?)?;
    let remote = hello_to_user(hello)?;
    if remote.certificate_fingerprint != request.expected.certificate_fingerprint
        || remote.fingerprint != request.expected.fingerprint
    {
        return Err(EndpointError::IdentityMismatch);
    }
    if remote.node_id != request.expected.node_id {
        return Err(EndpointError::IdentityMismatch);
    }

    let transcript =
        TrustTranscript::new(request.local.identity(), remote.identity(), request.token);
    let hash = transcript.hash()?;
    let secret = TrustSecret::new(request.token);
    let (session, message) = Spake2Session::start(Role::Initiator, &secret, &transcript)?;
    write_message(
        &mut send,
        &TrustMessage::Spake2 {
            message: message.clone(),
        },
    )
    .await?;

    let user_message = expect_spake2(read_message(&mut recv).await?)?;
    let confirmation = session.finish(&user_message, &hash)?;
    write_message(
        &mut send,
        &TrustMessage::Confirm {
            value: *confirmation.as_bytes(),
        },
    )
    .await?;

    let user_confirmation = expect_confirm(read_message(&mut recv).await?)?;
    if !confirmation.matches(&Confirmation::from_bytes(user_confirmation)) {
        return Err(EndpointError::ConfirmationMismatch);
    }
    let _ = send.finish();

    Ok(TrustOutcome { user: remote })
}

async fn run_responder_inner(
    connection: &quinn::Connection,
    request: ResponderRequest,
) -> Result<TrustOutcome, EndpointError> {
    let (mut send, mut recv) = connection
        .accept_bi()
        .await
        .map_err(|error| EndpointError::Connection(error.to_string()))?;

    let hello = expect_hello(read_message(&mut recv).await?)?;
    let remote = hello_to_user(hello)?;
    if remote.certificate_fingerprint != request.certificate_fingerprint {
        return Err(EndpointError::CertificateMismatch);
    }

    let hello = TrustMessage::Hello {
        node_id: request.local.node_id.to_string(),
        fingerprint: request.local.fingerprint.to_hex(),
        certificate_fingerprint: request.local.certificate_fingerprint.to_hex(),
    };
    write_message(&mut send, &hello).await?;

    let transcript =
        TrustTranscript::new(remote.identity(), request.local.identity(), request.token);
    let hash = transcript.hash()?;
    let secret = TrustSecret::new(request.token);

    let user_message = expect_spake2(read_message(&mut recv).await?)?;
    let (session, message) = Spake2Session::start(Role::Responder, &secret, &transcript)?;
    write_message(
        &mut send,
        &TrustMessage::Spake2 {
            message: message.clone(),
        },
    )
    .await?;

    let confirmation = session.finish(&user_message, &hash)?;
    let user_confirmation = expect_confirm(read_message(&mut recv).await?)?;
    if !confirmation.matches(&Confirmation::from_bytes(user_confirmation)) {
        return Err(EndpointError::ConfirmationMismatch);
    }
    write_message(
        &mut send,
        &TrustMessage::Confirm {
            value: *confirmation.as_bytes(),
        },
    )
    .await?;
    let _ = send.finish();

    Ok(TrustOutcome { user: remote })
}

async fn write_message(
    send: &mut quinn::SendStream,
    message: &TrustMessage,
) -> Result<(), EndpointError> {
    let frame = encode_frame(message)?;
    let bytes = frame.encode();
    let length = u32::try_from(bytes.len())
        .map_err(|_| EndpointError::Connection("trust frame is too large".to_owned()))?;
    send.write_all(&length.to_be_bytes())
        .await
        .map_err(|error| EndpointError::Connection(error.to_string()))?;
    send.write_all(&bytes)
        .await
        .map_err(|error| EndpointError::Connection(error.to_string()))?;
    Ok(())
}

async fn read_message(recv: &mut quinn::RecvStream) -> Result<TrustMessage, EndpointError> {
    let mut length = [0_u8; 4];
    recv.read_exact(&mut length)
        .await
        .map_err(|error| EndpointError::Connection(error.to_string()))?;
    let length = u32::from_be_bytes(length) as usize;
    if length > MAX_TRUST_FRAME {
        return Err(EndpointError::InvalidHello);
    }
    let mut bytes = vec![0_u8; length];
    recv.read_exact(&mut bytes)
        .await
        .map_err(|error| EndpointError::Connection(error.to_string()))?;
    Ok(decode_frame(&ticonnect_core::wire::Frame::decode(&bytes)?)?)
}

fn expect_hello(message: TrustMessage) -> Result<HelloFields, EndpointError> {
    match message {
        TrustMessage::Hello {
            node_id,
            fingerprint,
            certificate_fingerprint,
        } => Ok(HelloFields {
            node_id,
            fingerprint,
            certificate_fingerprint,
        }),
        _ => Err(EndpointError::InvalidHello),
    }
}

fn hello_to_user(hello: HelloFields) -> Result<TrustUser, EndpointError> {
    let node_id = NodeId::from_str(&hello.node_id).map_err(|_| EndpointError::InvalidHello)?;
    let fingerprint = Fingerprint::from_str(&hello.fingerprint).map_err(user_fingerprint_error)?;
    let certificate_fingerprint =
        Fingerprint::from_str(&hello.certificate_fingerprint).map_err(user_fingerprint_error)?;
    Ok(TrustUser {
        node_id,
        fingerprint,
        certificate_fingerprint,
    })
}

fn expect_spake2(message: TrustMessage) -> Result<Vec<u8>, EndpointError> {
    match message {
        TrustMessage::Spake2 { message } => Ok(message),
        _ => Err(EndpointError::InvalidHello),
    }
}

fn expect_confirm(message: TrustMessage) -> Result<[u8; 32], EndpointError> {
    match message {
        TrustMessage::Confirm { value } => Ok(value),
        _ => Err(EndpointError::InvalidHello),
    }
}

fn user_fingerprint_error(_: FingerprintError) -> EndpointError {
    EndpointError::InvalidHello
}

struct HelloFields {
    node_id: String,
    fingerprint: String,
    certificate_fingerprint: String,
}
