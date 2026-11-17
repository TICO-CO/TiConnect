use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::wire::{Frame, FrameKind, MAX_WATCH_PARTY_FRAME, WireError};

pub const MAX_VIDEO_ID: usize = 32;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WatchPartyCommand {
    pub revision: u64,
    pub actor: String,
    pub position_ms: u64,
    pub playing: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum WatchPartyFrame {
    Invite {
        party_id: [u8; 16],
        video_id: String,
    },
    Accept {
        party_id: [u8; 16],
    },
    Decline {
        party_id: [u8; 16],
    },
    Command {
        party_id: [u8; 16],
        command: WatchPartyCommand,
    },
    End {
        party_id: [u8; 16],
    },
}

impl WatchPartyFrame {
    pub fn party_id(&self) -> [u8; 16] {
        match self {
            Self::Invite { party_id, .. }
            | Self::Accept { party_id }
            | Self::Decline { party_id }
            | Self::Command { party_id, .. }
            | Self::End { party_id } => *party_id,
        }
    }

    pub fn encode(&self) -> Result<Vec<u8>, WatchPartyError> {
        if let Self::Invite { video_id, .. } = self
            && !valid_video_id(video_id)
        {
            return Err(WatchPartyError::InvalidVideoId);
        }
        let bytes = postcard::to_allocvec(self)
            .map_err(|error| WatchPartyError::Encode(error.to_string()))?;
        if bytes.len() > MAX_WATCH_PARTY_FRAME {
            return Err(WatchPartyError::TooLarge);
        }
        Ok(bytes)
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, WatchPartyError> {
        if bytes.len() > MAX_WATCH_PARTY_FRAME {
            return Err(WatchPartyError::TooLarge);
        }
        postcard::from_bytes(bytes).map_err(|error| WatchPartyError::Decode(error.to_string()))
    }

    pub fn to_wire(&self) -> Result<Frame, WatchPartyError> {
        Ok(Frame::new(FrameKind::WatchParty, self.encode()?)?)
    }

    pub fn from_wire(frame: &Frame) -> Result<Self, WatchPartyError> {
        if frame.kind() != FrameKind::WatchParty {
            return Err(WatchPartyError::UnexpectedFrame);
        }
        Self::decode(frame.payload())
    }
}

pub fn valid_video_id(video_id: &str) -> bool {
    !video_id.is_empty()
        && video_id.len() <= MAX_VIDEO_ID
        && video_id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_')
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum WatchPartyError {
    #[error("watch party frame is too large")]
    TooLarge,

    #[error("watch party frame could not be encoded: {0}")]
    Encode(String),

    #[error("watch party frame could not be decoded: {0}")]
    Decode(String),

    #[error("wire frame is not a watch party frame")]
    UnexpectedFrame,

    #[error("video id is invalid")]
    InvalidVideoId,

    #[error(transparent)]
    Wire(#[from] WireError),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn watch_party_frames_round_trip() {
        let frames = [
            WatchPartyFrame::Invite {
                party_id: [1_u8; 16],
                video_id: "dQw4w9WgXcQ".to_owned(),
            },
            WatchPartyFrame::Accept {
                party_id: [2_u8; 16],
            },
            WatchPartyFrame::Decline {
                party_id: [3_u8; 16],
            },
            WatchPartyFrame::Command {
                party_id: [4_u8; 16],
                command: WatchPartyCommand {
                    revision: 7,
                    actor: "018f0000-0000-7000-8000-000000000001".to_owned(),
                    position_ms: 42_000,
                    playing: true,
                },
            },
            WatchPartyFrame::End {
                party_id: [5_u8; 16],
            },
        ];

        for frame in frames {
            let bytes = frame.encode().expect("encode");
            assert_eq!(WatchPartyFrame::decode(&bytes).expect("decode"), frame);
            let wire = frame.to_wire().expect("wire");
            assert_eq!(WatchPartyFrame::from_wire(&wire).expect("from wire"), frame);
        }
    }

    #[test]
    fn rejects_invalid_video_ids() {
        assert!(valid_video_id("dQw4w9WgXcQ"));
        assert!(!valid_video_id(""));
        assert!(!valid_video_id("../bad"));
        assert!(!valid_video_id(&"x".repeat(MAX_VIDEO_ID + 1)));
    }
}
