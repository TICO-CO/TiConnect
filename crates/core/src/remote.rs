use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::wire::{Frame, FrameKind, MAX_INPUT_FRAME, MAX_REMOTE_FRAME, WireError};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum RemoteSignalFrame {
    Request { session_id: [u8; 16] },
    Consent { session_id: [u8; 16] },
    Pin { session_id: [u8; 16], pin: String },
    Deny { session_id: [u8; 16] },
    Start { session_id: [u8; 16] },
    Hangup { session_id: [u8; 16] },
    KillSwitch { session_id: [u8; 16] },
    KeyframeRequest { session_id: [u8; 16] },
}

impl RemoteSignalFrame {
    pub fn session_id(&self) -> [u8; 16] {
        match self {
            Self::Request { session_id }
            | Self::Consent { session_id }
            | Self::Pin { session_id, .. }
            | Self::Deny { session_id }
            | Self::Start { session_id }
            | Self::Hangup { session_id }
            | Self::KillSwitch { session_id }
            | Self::KeyframeRequest { session_id } => *session_id,
        }
    }

    pub fn encode(&self) -> Result<Vec<u8>, RemoteError> {
        let bytes =
            postcard::to_allocvec(self).map_err(|error| RemoteError::Encode(error.to_string()))?;
        if bytes.len() > MAX_REMOTE_FRAME {
            return Err(RemoteError::TooLarge);
        }
        Ok(bytes)
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, RemoteError> {
        if bytes.len() > MAX_REMOTE_FRAME {
            return Err(RemoteError::TooLarge);
        }
        postcard::from_bytes(bytes).map_err(|error| RemoteError::Decode(error.to_string()))
    }

    pub fn to_wire(&self) -> Result<Frame, RemoteError> {
        Ok(Frame::new(FrameKind::Remote, self.encode()?)?)
    }

    pub fn from_wire(frame: &Frame) -> Result<Self, RemoteError> {
        if frame.kind() != FrameKind::Remote {
            return Err(RemoteError::UnexpectedFrame);
        }
        Self::decode(frame.payload())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum InputKind {
    Motion,
    Button,
    Axis,
    Key,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct InputPacket {
    pub session_id: [u8; 16],
    pub kind: InputKind,
    pub x: i32,
    pub y: i32,
    pub pressed: bool,
}

impl InputPacket {
    pub fn encode(&self) -> Result<Vec<u8>, RemoteError> {
        let bytes =
            postcard::to_allocvec(self).map_err(|error| RemoteError::Encode(error.to_string()))?;
        if bytes.len() > MAX_INPUT_FRAME {
            return Err(RemoteError::TooLarge);
        }
        Ok(bytes)
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, RemoteError> {
        if bytes.len() > MAX_INPUT_FRAME {
            return Err(RemoteError::TooLarge);
        }
        postcard::from_bytes(bytes).map_err(|error| RemoteError::Decode(error.to_string()))
    }

    pub fn to_wire(&self) -> Result<Frame, RemoteError> {
        Ok(Frame::new(FrameKind::Input, self.encode()?)?)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VideoFrame {
    pub session_id: [u8; 16],
    pub sequence: u64,
    pub keyframe: bool,
    pub last: bool,
    pub data: Vec<u8>,
}

impl VideoFrame {
    pub fn encode(&self) -> Result<Vec<u8>, RemoteError> {
        let bytes =
            postcard::to_allocvec(self).map_err(|error| RemoteError::Encode(error.to_string()))?;
        if bytes.len() > crate::wire::MAX_VIDEO_FRAME {
            return Err(RemoteError::TooLarge);
        }
        Ok(bytes)
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, RemoteError> {
        if bytes.len() > crate::wire::MAX_VIDEO_FRAME {
            return Err(RemoteError::TooLarge);
        }
        postcard::from_bytes(bytes).map_err(|error| RemoteError::Decode(error.to_string()))
    }
}

#[derive(Debug, Error)]
pub enum RemoteError {
    #[error(transparent)]
    Wire(#[from] WireError),

    #[error("remote frame could not be encoded: {0}")]
    Encode(String),

    #[error("remote frame could not be decoded: {0}")]
    Decode(String),

    #[error("remote frame exceeds the maximum size")]
    TooLarge,

    #[error("unexpected remote frame kind")]
    UnexpectedFrame,
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    #[test]
    fn signals_and_packets_round_trip() {
        let session_id = *Uuid::now_v7().as_bytes();
        for signal in [
            RemoteSignalFrame::Request { session_id },
            RemoteSignalFrame::Consent { session_id },
            RemoteSignalFrame::Pin {
                session_id,
                pin: "123456".to_owned(),
            },
            RemoteSignalFrame::Deny { session_id },
            RemoteSignalFrame::Start { session_id },
            RemoteSignalFrame::Hangup { session_id },
            RemoteSignalFrame::KillSwitch { session_id },
            RemoteSignalFrame::KeyframeRequest { session_id },
        ] {
            assert_eq!(signal.session_id(), session_id);
            let wire = signal.to_wire().expect("wire");
            assert_eq!(RemoteSignalFrame::from_wire(&wire).expect("decode"), signal);
        }
        let packet = InputPacket {
            session_id,
            kind: InputKind::Motion,
            x: 10,
            y: 20,
            pressed: false,
        };
        let wire = packet.to_wire().expect("wire");
        assert_eq!(InputPacket::decode(wire.payload()).expect("decode"), packet);
    }
}
