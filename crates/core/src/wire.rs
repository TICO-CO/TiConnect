use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

use crate::fingerprint::Fingerprint;
use crate::id::NodeId;

pub const WIRE_VERSION: u8 = 1;
pub const PROTOCOL_VERSION: u16 = 1;
pub const MAX_CONTROL_FRAME: usize = 64 * 1024;
pub const MAX_PAIRING_FRAME: usize = 4 * 1024;
pub const MAX_HANDSHAKE_FRAME: usize = 256;
pub const MAX_SYNC_FRAME: usize = MAX_CONTROL_FRAME + 1024;
pub const MAX_MESSAGE_FRAME: usize = MAX_CONTROL_FRAME + 1024;
pub const MAX_ACK_FRAME: usize = 4 * 1024;
pub const MAX_TRANSFER_FRAME: usize = 128 * 1024 + 1024;
pub const MAX_CALL_FRAME: usize = 16 * 1024;
pub const MAX_AUDIO_FRAME: usize = 4 * 1024;
pub const MAX_VIDEO_FRAME: usize = 128 * 1024 + 1024;
pub const MAX_INPUT_FRAME: usize = 1024;
pub const MAX_REMOTE_FRAME: usize = 16 * 1024;
pub const MAX_WIRE_FRAME: usize = MAX_VIDEO_FRAME;

const MAGIC: &[u8; 4] = b"TICW";
const HEADER_LEN: usize = MAGIC.len() + 1 + 1 + 4;
const HANDSHAKE_LEN: usize = 2 + 16 + 32;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum WireError {
    #[error("wire frame exceeds the maximum size")]
    FrameTooLarge,

    #[error("wire frame is truncated")]
    Truncated,

    #[error("wire frame has trailing bytes")]
    TrailingBytes,

    #[error("wire frame magic does not match")]
    BadMagic,

    #[error("wire version {0} is not supported")]
    UnsupportedVersion(u8),

    #[error("wire frame kind {0} is not supported")]
    UnsupportedKind(u8),

    #[error("handshake frame has an unsupported protocol version {0}")]
    UnsupportedProtocolVersion(u16),

    #[error("sync frame could not be encoded: {0}")]
    SyncEncode(String),

    #[error("sync frame could not be decoded: {0}")]
    SyncDecode(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum FrameKind {
    Handshake = 1,
    Pairing = 2,
    Sync = 3,
    Message = 4,
    Ack = 5,
    Transfer = 6,
    Call = 7,
    Audio = 8,
    Video = 9,
    Input = 10,
    Remote = 11,
}

impl FrameKind {
    pub fn as_u8(self) -> u8 {
        self as u8
    }

    pub fn limit(self) -> usize {
        match self {
            Self::Handshake => MAX_HANDSHAKE_FRAME,
            Self::Pairing => MAX_PAIRING_FRAME,
            Self::Sync => MAX_SYNC_FRAME,
            Self::Message => MAX_MESSAGE_FRAME,
            Self::Ack => MAX_ACK_FRAME,
            Self::Transfer => MAX_TRANSFER_FRAME,
            Self::Call => MAX_CALL_FRAME,
            Self::Audio => MAX_AUDIO_FRAME,
            Self::Video => MAX_VIDEO_FRAME,
            Self::Input => MAX_INPUT_FRAME,
            Self::Remote => MAX_REMOTE_FRAME,
        }
    }
}

impl TryFrom<u8> for FrameKind {
    type Error = WireError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            1 => Ok(Self::Handshake),
            2 => Ok(Self::Pairing),
            3 => Ok(Self::Sync),
            4 => Ok(Self::Message),
            5 => Ok(Self::Ack),
            6 => Ok(Self::Transfer),
            7 => Ok(Self::Call),
            8 => Ok(Self::Audio),
            9 => Ok(Self::Video),
            10 => Ok(Self::Input),
            11 => Ok(Self::Remote),
            other => Err(WireError::UnsupportedKind(other)),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Frame {
    kind: FrameKind,
    payload: Vec<u8>,
}

impl Frame {
    pub fn new(kind: FrameKind, payload: impl Into<Vec<u8>>) -> Result<Self, WireError> {
        let payload = payload.into();
        if payload.len() > kind.limit() {
            return Err(WireError::FrameTooLarge);
        }
        Ok(Self { kind, payload })
    }

    pub fn kind(&self) -> FrameKind {
        self.kind
    }

    pub fn payload(&self) -> &[u8] {
        &self.payload
    }

    pub fn encode(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(HEADER_LEN + self.payload.len());
        bytes.extend_from_slice(MAGIC);
        bytes.push(WIRE_VERSION);
        bytes.push(self.kind.as_u8());
        bytes.extend_from_slice(
            &u32::try_from(self.payload.len())
                .unwrap_or(u32::MAX)
                .to_be_bytes(),
        );
        bytes.extend_from_slice(&self.payload);
        bytes
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, WireError> {
        if bytes.len() < HEADER_LEN {
            return Err(WireError::Truncated);
        }
        if &bytes[..MAGIC.len()] != MAGIC {
            return Err(WireError::BadMagic);
        }

        let mut cursor = MAGIC.len();
        let version = bytes[cursor];
        cursor += 1;
        if version != WIRE_VERSION {
            return Err(WireError::UnsupportedVersion(version));
        }

        let kind = FrameKind::try_from(bytes[cursor])?;
        cursor += 1;

        let payload_len = u32::from_be_bytes(
            bytes[cursor..cursor + 4]
                .try_into()
                .map_err(|_| WireError::Truncated)?,
        ) as usize;
        cursor += 4;

        if payload_len > kind.limit() {
            return Err(WireError::FrameTooLarge);
        }
        let end = cursor
            .checked_add(payload_len)
            .ok_or(WireError::Truncated)?;
        if end > bytes.len() {
            return Err(WireError::Truncated);
        }
        if end < bytes.len() {
            return Err(WireError::TrailingBytes);
        }

        Ok(Self {
            kind,
            payload: bytes[cursor..end].to_vec(),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HandshakeFrame {
    pub node_id: NodeId,
    pub fingerprint: Fingerprint,
}

impl HandshakeFrame {
    pub fn new(node_id: NodeId, fingerprint: Fingerprint) -> Self {
        Self {
            node_id,
            fingerprint,
        }
    }

    pub fn encode(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(HANDSHAKE_LEN);
        bytes.extend_from_slice(&PROTOCOL_VERSION.to_be_bytes());
        bytes.extend_from_slice(self.node_id.as_uuid().as_bytes());
        bytes.extend_from_slice(self.fingerprint.as_bytes());
        bytes
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, WireError> {
        if bytes.len() < HANDSHAKE_LEN {
            return Err(WireError::Truncated);
        }
        if bytes.len() > HANDSHAKE_LEN {
            return Err(WireError::TrailingBytes);
        }

        let version = u16::from_be_bytes(bytes[0..2].try_into().map_err(|_| WireError::Truncated)?);
        if version != PROTOCOL_VERSION {
            return Err(WireError::UnsupportedProtocolVersion(version));
        }

        let node_id = NodeId::from_uuid(Uuid::from_bytes(
            bytes[2..18].try_into().map_err(|_| WireError::Truncated)?,
        ));
        let fingerprint =
            Fingerprint::from_bytes(bytes[18..50].try_into().map_err(|_| WireError::Truncated)?);
        Ok(Self {
            node_id,
            fingerprint,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SyncFrame {
    Hello {
        last_sent_seq: u64,
        last_recv_seq: u64,
    },
    Request {
        from_seq: u64,
        to_seq: u64,
    },
    Record {
        envelope: Vec<u8>,
    },
    Done {
        count: u32,
    },
    Complete,
    Error {
        code: u8,
    },
}

pub fn encode_sync_frame(frame: &SyncFrame) -> Result<Vec<u8>, WireError> {
    let bytes =
        postcard::to_allocvec(frame).map_err(|error| WireError::SyncEncode(error.to_string()))?;
    if bytes.len() > MAX_SYNC_FRAME {
        return Err(WireError::FrameTooLarge);
    }
    Ok(bytes)
}

pub fn decode_sync_frame(bytes: &[u8]) -> Result<SyncFrame, WireError> {
    if bytes.len() > MAX_SYNC_FRAME {
        return Err(WireError::FrameTooLarge);
    }
    postcard::from_bytes(bytes).map_err(|error| WireError::SyncDecode(error.to_string()))
}

pub fn sync_frame_to_wire(frame: &SyncFrame) -> Result<Frame, WireError> {
    Frame::new(FrameKind::Sync, encode_sync_frame(frame)?)
}

pub fn sync_frame_from_wire(frame: &Frame) -> Result<SyncFrame, WireError> {
    if frame.kind() != FrameKind::Sync {
        return Err(WireError::UnsupportedKind(frame.kind().as_u8()));
    }
    decode_sync_frame(frame.payload())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::identity::IdentityKeyPair;

    #[test]
    fn frame_round_trips_for_each_kind() {
        for kind in [
            FrameKind::Handshake,
            FrameKind::Pairing,
            FrameKind::Sync,
            FrameKind::Message,
            FrameKind::Ack,
            FrameKind::Transfer,
            FrameKind::Call,
            FrameKind::Audio,
            FrameKind::Video,
            FrameKind::Input,
            FrameKind::Remote,
        ] {
            let frame = Frame::new(kind, b"payload".to_vec()).expect("frame");
            let decoded = Frame::decode(&frame.encode()).expect("decode");

            assert_eq!(decoded, frame);
            assert_eq!(decoded.kind(), kind);
            assert_eq!(decoded.payload(), b"payload");
        }
    }

    #[test]
    fn rejects_oversized_pairing_frame() {
        assert!(matches!(
            Frame::new(FrameKind::Pairing, vec![0_u8; MAX_PAIRING_FRAME + 1]),
            Err(WireError::FrameTooLarge)
        ));

        let mut header = Vec::new();
        header.extend_from_slice(MAGIC);
        header.push(WIRE_VERSION);
        header.push(FrameKind::Pairing.as_u8());
        header.extend_from_slice(&u32::try_from(MAX_PAIRING_FRAME + 1).unwrap().to_be_bytes());
        header.extend(std::iter::repeat_n(0_u8, MAX_PAIRING_FRAME + 1));
        assert!(matches!(
            Frame::decode(&header),
            Err(WireError::FrameTooLarge)
        ));
    }

    #[test]
    fn rejects_malformed_frames() {
        let valid = Frame::new(FrameKind::Handshake, Vec::new())
            .expect("frame")
            .encode();

        assert!(matches!(
            Frame::decode(&valid[..HEADER_LEN - 1]),
            Err(WireError::Truncated)
        ));

        let mut bad_magic = valid.clone();
        bad_magic[0] ^= 0xff;
        assert!(matches!(
            Frame::decode(&bad_magic),
            Err(WireError::BadMagic)
        ));

        let mut bad_version = valid.clone();
        bad_version[MAGIC.len()] = 2;
        assert!(matches!(
            Frame::decode(&bad_version),
            Err(WireError::UnsupportedVersion(2))
        ));

        let mut bad_kind = valid.clone();
        bad_kind[MAGIC.len() + 1] = 12;
        assert!(matches!(
            Frame::decode(&bad_kind),
            Err(WireError::UnsupportedKind(12))
        ));

        let mut trailing = valid;
        trailing.push(0);
        assert!(matches!(
            Frame::decode(&trailing),
            Err(WireError::TrailingBytes)
        ));
    }

    #[test]
    fn handshake_round_trips_and_rejects_malformed() {
        let identity = IdentityKeyPair::from_secret_bytes([31_u8; 32]);
        let frame = HandshakeFrame::new(
            NodeId::generate(),
            Fingerprint::of(&identity.verifying_key()),
        );
        let decoded = HandshakeFrame::decode(&frame.encode()).expect("decode");

        assert_eq!(decoded, frame);

        assert!(matches!(
            HandshakeFrame::decode(&frame.encode()[..HANDSHAKE_LEN - 1]),
            Err(WireError::Truncated)
        ));

        let mut trailing = frame.encode();
        trailing.push(0);
        assert!(matches!(
            HandshakeFrame::decode(&trailing),
            Err(WireError::TrailingBytes)
        ));

        let mut bad_version = frame.encode();
        bad_version[0..2].copy_from_slice(&9_u16.to_be_bytes());
        assert!(matches!(
            HandshakeFrame::decode(&bad_version),
            Err(WireError::UnsupportedProtocolVersion(9))
        ));
    }

    #[test]
    fn sync_frames_round_trip_and_reject_garbage() {
        let frames = [
            SyncFrame::Hello {
                last_sent_seq: 7,
                last_recv_seq: 3,
            },
            SyncFrame::Request {
                from_seq: 4,
                to_seq: 7,
            },
            SyncFrame::Record {
                envelope: vec![1_u8; 32],
            },
            SyncFrame::Done { count: 4 },
            SyncFrame::Complete,
            SyncFrame::Error { code: 2 },
        ];

        for frame in frames {
            let bytes = encode_sync_frame(&frame).expect("encode");
            assert_eq!(decode_sync_frame(&bytes).expect("decode"), frame);
            let wire = sync_frame_to_wire(&frame).expect("wire");
            assert_eq!(sync_frame_from_wire(&wire).expect("decode wire"), frame);
        }

        for length in 0..=64 {
            let bytes = vec![0xa5_u8; length];
            let _ = decode_sync_frame(&bytes);
        }
    }
}
