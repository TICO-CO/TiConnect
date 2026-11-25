use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::wire::{Frame, FrameKind, MAX_WATCH_PARTY_FRAME, WireError};

pub const MAX_VIDEO_ID: usize = 32;
pub const MAX_PARTY_PARTICIPANTS: usize = 16;

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
    Leave {
        party_id: [u8; 16],
    },
    Command {
        party_id: [u8; 16],
        position_ms: u64,
        playing: bool,
    },
    Sync {
        party_id: [u8; 16],
        state: String,
        participants: Vec<String>,
        invited: Vec<String>,
        position_ms: u64,
        playing: bool,
        revision: u64,
        actor: String,
        updated_at: u64,
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
            | Self::Leave { party_id }
            | Self::Command { party_id, .. }
            | Self::Sync { party_id, .. }
            | Self::End { party_id } => *party_id,
        }
    }

    pub fn encode(&self) -> Result<Vec<u8>, WatchPartyError> {
        match self {
            Self::Invite { video_id, .. } if !valid_video_id(video_id) => {
                return Err(WatchPartyError::InvalidVideoId);
            }
            Self::Sync {
                participants,
                invited,
                ..
            } if participants.len() > MAX_PARTY_PARTICIPANTS
                || invited.len() > MAX_PARTY_PARTICIPANTS =>
            {
                return Err(WatchPartyError::TooManyParticipants);
            }
            _ => {}
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

    #[error("watch party has too many participants")]
    TooManyParticipants,

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
            WatchPartyFrame::Leave {
                party_id: [4_u8; 16],
            },
            WatchPartyFrame::Command {
                party_id: [5_u8; 16],
                position_ms: 42_000,
                playing: true,
            },
            WatchPartyFrame::Sync {
                party_id: [6_u8; 16],
                state: "active".to_owned(),
                participants: vec!["host".to_owned(), "guest".to_owned()],
                invited: vec!["pending".to_owned()],
                position_ms: 42_000,
                playing: true,
                revision: 7,
                actor: "guest".to_owned(),
                updated_at: 1_700_000_000_000,
            },
            WatchPartyFrame::End {
                party_id: [7_u8; 16],
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

    #[test]
    fn rejects_oversized_participant_lists() {
        let frame = WatchPartyFrame::Sync {
            party_id: [1_u8; 16],
            state: "active".to_owned(),
            participants: (0..=MAX_PARTY_PARTICIPANTS)
                .map(|index| format!("participant-{index}"))
                .collect(),
            invited: Vec::new(),
            position_ms: 0,
            playing: false,
            revision: 0,
            actor: String::new(),
            updated_at: 0,
        };
        assert_eq!(frame.encode(), Err(WatchPartyError::TooManyParticipants));
    }
}
