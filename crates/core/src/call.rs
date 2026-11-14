use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

use crate::wire::{Frame, FrameKind, MAX_AUDIO_FRAME, MAX_CALL_FRAME, WireError};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CallId(Uuid);

impl CallId {
    pub fn generate() -> Self {
        Self(Uuid::now_v7())
    }

    pub fn from_uuid(uuid: Uuid) -> Self {
        Self(uuid)
    }

    pub fn as_uuid(&self) -> &Uuid {
        &self.0
    }

    pub fn as_bytes(&self) -> &[u8; 16] {
        self.0.as_bytes()
    }
}

impl fmt::Display for CallId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

impl FromStr for CallId {
    type Err = CallError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Uuid::parse_str(value)
            .map(Self)
            .map_err(|_| CallError::InvalidCallId)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CallDirection {
    Incoming,
    Outgoing,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CallState {
    Idle,
    Ringing,
    InCall,
    Ended,
}

impl CallState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::Ringing => "ringing",
            Self::InCall => "in-call",
            Self::Ended => "ended",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CallEvent {
    Start,
    Incoming,
    Accept,
    Decline,
    Hangup,
    Timeout,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum CallSignalFrame {
    Offer(CallId),
    Ringing(CallId),
    Accept(CallId),
    Decline(CallId),
    Hangup(CallId),
    Busy(CallId),
    Error { call_id: CallId, code: u8 },
}

impl CallSignalFrame {
    pub fn call_id(&self) -> CallId {
        match self {
            Self::Offer(id)
            | Self::Ringing(id)
            | Self::Accept(id)
            | Self::Decline(id)
            | Self::Hangup(id)
            | Self::Busy(id)
            | Self::Error { call_id: id, .. } => *id,
        }
    }

    pub fn encode(&self) -> Result<Vec<u8>, CallError> {
        let bytes =
            postcard::to_allocvec(self).map_err(|error| CallError::Encode(error.to_string()))?;
        if bytes.len() > MAX_CALL_FRAME {
            return Err(CallError::TooLarge);
        }
        Ok(bytes)
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, CallError> {
        if bytes.len() > MAX_CALL_FRAME {
            return Err(CallError::TooLarge);
        }
        postcard::from_bytes(bytes).map_err(|error| CallError::Decode(error.to_string()))
    }

    pub fn to_wire(&self) -> Result<Frame, CallError> {
        Ok(Frame::new(FrameKind::Call, self.encode()?)?)
    }

    pub fn from_wire(frame: &Frame) -> Result<Self, CallError> {
        if frame.kind() != FrameKind::Call {
            return Err(CallError::UnexpectedFrame);
        }
        Self::decode(frame.payload())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AudioPacket {
    pub call_id: [u8; 16],
    pub sequence: u32,
    pub timestamp: u32,
    pub payload: Vec<u8>,
}

impl AudioPacket {
    pub fn new(call_id: CallId, sequence: u32, timestamp: u32, payload: Vec<u8>) -> Self {
        Self {
            call_id: *call_id.as_bytes(),
            sequence,
            timestamp,
            payload,
        }
    }

    pub fn id(&self) -> CallId {
        CallId::from_uuid(Uuid::from_bytes(self.call_id))
    }

    pub fn encode(&self) -> Result<Vec<u8>, CallError> {
        if self.payload.len() > MAX_AUDIO_FRAME - 64 {
            return Err(CallError::TooLarge);
        }
        postcard::to_allocvec(self).map_err(|error| CallError::Encode(error.to_string()))
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, CallError> {
        if bytes.len() > MAX_AUDIO_FRAME {
            return Err(CallError::TooLarge);
        }
        postcard::from_bytes(bytes).map_err(|error| CallError::Decode(error.to_string()))
    }

    pub fn to_wire(&self) -> Result<Frame, CallError> {
        Ok(Frame::new(FrameKind::Audio, self.encode()?)?)
    }
}

#[derive(Debug, Error)]
pub enum CallError {
    #[error(transparent)]
    Wire(#[from] WireError),

    #[error("call id is invalid")]
    InvalidCallId,

    #[error("call frame could not be encoded: {0}")]
    Encode(String),

    #[error("call frame could not be decoded: {0}")]
    Decode(String),

    #[error("call frame exceeds the maximum size")]
    TooLarge,

    #[error("unexpected call frame kind")]
    UnexpectedFrame,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn call_id_round_trips() {
        let id = CallId::generate();
        assert_eq!(id.to_string().parse::<CallId>().expect("parse"), id);
        assert!("nope".parse::<CallId>().is_err());
    }

    #[test]
    fn signals_round_trip_and_reject_bad_frames() {
        let id = CallId::generate();
        for signal in [
            CallSignalFrame::Offer(id),
            CallSignalFrame::Ringing(id),
            CallSignalFrame::Accept(id),
            CallSignalFrame::Decline(id),
            CallSignalFrame::Hangup(id),
            CallSignalFrame::Busy(id),
            CallSignalFrame::Error {
                call_id: id,
                code: 3,
            },
        ] {
            assert_eq!(signal.call_id(), id);
            assert_eq!(
                CallSignalFrame::decode(&signal.encode().expect("encode")).expect("decode"),
                signal
            );
            let wire = signal.to_wire().expect("wire");
            assert_eq!(
                CallSignalFrame::from_wire(&wire).expect("from wire"),
                signal
            );
        }
    }

    #[test]
    fn audio_packets_round_trip() {
        let id = CallId::generate();
        let packet = AudioPacket::new(id, 7, 960, vec![1_u8; 40]);
        let bytes = packet.encode().expect("encode");
        assert_eq!(AudioPacket::decode(&bytes).expect("decode"), packet);
        assert!(
            AudioPacket::new(id, 0, 0, vec![0; MAX_AUDIO_FRAME])
                .encode()
                .is_err()
        );
    }
}
