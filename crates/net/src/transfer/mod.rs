pub mod chunk;
pub mod resume;

use serde::{Deserialize, Serialize};
use thiserror::Error;
use ticonnect_core::attachment::AttachmentHash;
use ticonnect_core::wire::{Frame, FrameKind, MAX_TRANSFER_FRAME, WireError};

pub use chunk::{Chunk, ChunkError, DEFAULT_CHUNK_SIZE, MAX_CHUNK_SIZE};
pub use resume::{ResumeError, ResumePlan};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TransferFrame {
    Request {
        hash: [u8; 32],
        size: u64,
        ranges: Vec<(u64, u64)>,
    },
    Chunk {
        hash: [u8; 32],
        offset: u64,
        data: Vec<u8>,
    },
    Complete {
        hash: [u8; 32],
    },
    Cancel {
        hash: [u8; 32],
    },
    Error {
        code: u8,
    },
}

impl TransferFrame {
    pub fn request(
        hash: AttachmentHash,
        size: u64,
        ranges: impl IntoIterator<Item = (u64, u64)>,
    ) -> Self {
        Self::Request {
            hash: *hash.as_bytes(),
            size,
            ranges: ranges.into_iter().collect(),
        }
    }

    pub fn chunk(chunk: Chunk) -> Self {
        Self::Chunk {
            hash: *chunk.hash().as_bytes(),
            offset: chunk.offset(),
            data: chunk.data().to_vec(),
        }
    }

    pub fn complete(hash: AttachmentHash) -> Self {
        Self::Complete {
            hash: *hash.as_bytes(),
        }
    }

    pub fn cancel(hash: AttachmentHash) -> Self {
        Self::Cancel {
            hash: *hash.as_bytes(),
        }
    }

    pub fn hash(&self) -> Option<AttachmentHash> {
        match self {
            Self::Request { hash, .. }
            | Self::Chunk { hash, .. }
            | Self::Complete { hash }
            | Self::Cancel { hash } => Some(AttachmentHash::from_bytes(*hash)),
            Self::Error { .. } => None,
        }
    }

    pub fn encode(&self) -> Result<Vec<u8>, TransferError> {
        self.validate()?;
        let bytes = postcard::to_allocvec(self)
            .map_err(|error| TransferError::Encode(error.to_string()))?;
        if bytes.len() > MAX_TRANSFER_FRAME {
            return Err(TransferError::TooLarge);
        }
        Ok(bytes)
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, TransferError> {
        if bytes.len() > MAX_TRANSFER_FRAME {
            return Err(TransferError::TooLarge);
        }
        let frame: Self = postcard::from_bytes(bytes)
            .map_err(|error| TransferError::Decode(error.to_string()))?;
        frame.validate()?;
        Ok(frame)
    }

    pub fn validate(&self) -> Result<(), TransferError> {
        match self {
            Self::Request { ranges, .. } => {
                let mut last_end = 0_u64;
                for (start, end) in ranges {
                    if start >= end || *start < last_end {
                        return Err(TransferError::InvalidRanges);
                    }
                    last_end = *end;
                }
                Ok(())
            }
            Self::Chunk { data, offset, .. } => {
                if data.len() > MAX_CHUNK_SIZE {
                    return Err(TransferError::TooLarge);
                }
                if data.is_empty() {
                    return Err(TransferError::InvalidChunk);
                }
                offset
                    .checked_add(
                        u64::try_from(data.len()).map_err(|_| TransferError::InvalidChunk)?,
                    )
                    .ok_or(TransferError::InvalidChunk)?;
                Ok(())
            }
            Self::Complete { .. } | Self::Cancel { .. } | Self::Error { .. } => Ok(()),
        }
    }

    pub fn to_wire(&self) -> Result<Frame, TransferError> {
        Ok(Frame::new(FrameKind::Transfer, self.encode()?)?)
    }

    pub fn from_wire(frame: &Frame) -> Result<Self, TransferError> {
        if frame.kind() != FrameKind::Transfer {
            return Err(TransferError::UnexpectedKind);
        }
        Self::decode(frame.payload())
    }
}

pub fn transfer_frame_from_wire(frame: &Frame) -> Result<TransferFrame, WireError> {
    if frame.kind() != FrameKind::Transfer {
        return Err(WireError::UnsupportedKind(frame.kind().as_u8()));
    }
    TransferFrame::decode(frame.payload()).map_err(|error| WireError::SyncDecode(error.to_string()))
}

#[derive(Debug, Error)]
pub enum TransferError {
    #[error(transparent)]
    Chunk(#[from] ChunkError),

    #[error(transparent)]
    Wire(#[from] WireError),

    #[error("transfer frame could not be encoded: {0}")]
    Encode(String),

    #[error("transfer frame could not be decoded: {0}")]
    Decode(String),

    #[error("transfer frame exceeds the maximum size")]
    TooLarge,

    #[error("transfer frame kind is unexpected")]
    UnexpectedKind,

    #[error("transfer ranges are invalid")]
    InvalidRanges,

    #[error("transfer chunk is invalid")]
    InvalidChunk,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transfer_frames_round_trip() {
        let hash = AttachmentHash::of(b"content");
        let frames = [
            TransferFrame::request(hash, 100, [(0, 50), (50, 100)]),
            TransferFrame::chunk(Chunk::new(hash, 0, vec![7_u8; 100]).expect("chunk")),
            TransferFrame::complete(hash),
            TransferFrame::cancel(hash),
            TransferFrame::Error { code: 2 },
        ];
        for frame in frames {
            let bytes = frame.encode().expect("encode");
            assert_eq!(TransferFrame::decode(&bytes).expect("decode"), frame);
            let wire = frame.to_wire().expect("wire");
            assert_eq!(TransferFrame::from_wire(&wire).expect("from wire"), frame);
        }
    }

    #[test]
    fn rejects_malformed_requests_and_chunks() {
        assert!(matches!(
            TransferFrame::Request {
                hash: [0; 32],
                size: 10,
                ranges: vec![(5, 4)]
            }
            .encode(),
            Err(TransferError::InvalidRanges)
        ));
        let oversized = TransferFrame::Chunk {
            hash: [0; 32],
            offset: 0,
            data: vec![0; MAX_CHUNK_SIZE + 1],
        };
        assert!(matches!(oversized.encode(), Err(TransferError::TooLarge)));
    }
}
