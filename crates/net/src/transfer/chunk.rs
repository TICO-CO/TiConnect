use thiserror::Error;
use ticonnect_core::attachment::AttachmentHash;

pub const DEFAULT_CHUNK_SIZE: usize = 64 * 1024;
pub const MAX_CHUNK_SIZE: usize = 128 * 1024;
const CHUNK_MAGIC: &[u8; 4] = b"TICH";
const HEADER_LEN: usize = 4 + 1 + 32 + 8 + 4;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Chunk {
    hash: AttachmentHash,
    offset: u64,
    data: Vec<u8>,
}

impl Chunk {
    pub fn new(
        hash: AttachmentHash,
        offset: u64,
        data: impl Into<Vec<u8>>,
    ) -> Result<Self, ChunkError> {
        let data = data.into();
        if data.is_empty() {
            return Err(ChunkError::Empty);
        }
        if data.len() > MAX_CHUNK_SIZE {
            return Err(ChunkError::TooLarge);
        }
        offset
            .checked_add(u64::try_from(data.len()).map_err(|_| ChunkError::TooLarge)?)
            .ok_or(ChunkError::Overflow)?;
        Ok(Self { hash, offset, data })
    }

    pub fn hash(&self) -> AttachmentHash {
        self.hash
    }

    pub fn offset(&self) -> u64 {
        self.offset
    }

    pub fn data(&self) -> &[u8] {
        &self.data
    }

    pub fn len(&self) -> usize {
        self.data.len()
    }

    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    pub fn end(&self) -> u64 {
        self.offset.saturating_add(self.data.len() as u64)
    }

    pub fn encode(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(HEADER_LEN + self.data.len());
        bytes.extend_from_slice(CHUNK_MAGIC);
        bytes.push(1);
        bytes.extend_from_slice(self.hash.as_bytes());
        bytes.extend_from_slice(&self.offset.to_be_bytes());
        bytes.extend_from_slice(
            &u32::try_from(self.data.len())
                .unwrap_or(u32::MAX)
                .to_be_bytes(),
        );
        bytes.extend_from_slice(&self.data);
        bytes
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, ChunkError> {
        if bytes.len() < HEADER_LEN {
            return Err(ChunkError::Truncated);
        }
        if &bytes[..4] != CHUNK_MAGIC {
            return Err(ChunkError::BadMagic);
        }
        if bytes[4] != 1 {
            return Err(ChunkError::UnsupportedVersion(bytes[4]));
        }
        let hash =
            AttachmentHash::from_bytes(bytes[5..37].try_into().map_err(|_| ChunkError::Truncated)?);
        let offset = u64::from_be_bytes(
            bytes[37..45]
                .try_into()
                .map_err(|_| ChunkError::Truncated)?,
        );
        let length = u32::from_be_bytes(
            bytes[45..49]
                .try_into()
                .map_err(|_| ChunkError::Truncated)?,
        ) as usize;
        if length == 0 {
            return Err(ChunkError::Empty);
        }
        if length > MAX_CHUNK_SIZE {
            return Err(ChunkError::TooLarge);
        }
        let end = HEADER_LEN.checked_add(length).ok_or(ChunkError::Overflow)?;
        if end > bytes.len() {
            return Err(ChunkError::Truncated);
        }
        if end < bytes.len() {
            return Err(ChunkError::TrailingBytes);
        }
        Self::new(hash, offset, bytes[HEADER_LEN..end].to_vec())
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ChunkError {
    #[error("chunk is truncated")]
    Truncated,

    #[error("chunk has trailing bytes")]
    TrailingBytes,

    #[error("chunk magic does not match")]
    BadMagic,

    #[error("chunk version {0} is not supported")]
    UnsupportedVersion(u8),

    #[error("chunk is empty")]
    Empty,

    #[error("chunk exceeds the maximum size")]
    TooLarge,

    #[error("chunk offset overflows")]
    Overflow,

    #[error("chunk I/O failed: {0}")]
    Io(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips() {
        let chunk = Chunk::new(AttachmentHash::of(b"data"), 42, vec![1, 2, 3]).expect("chunk");
        assert_eq!(Chunk::decode(&chunk.encode()).expect("decode"), chunk);
        assert_eq!(chunk.end(), 45);
    }

    #[test]
    fn rejects_malformed() {
        let hash = AttachmentHash::of(b"data");
        let mut bytes = Chunk::new(hash, 0, vec![1]).expect("chunk").encode();
        bytes[0] = b'X';
        assert!(matches!(Chunk::decode(&bytes), Err(ChunkError::BadMagic)));

        let mut truncated = Chunk::new(hash, 0, vec![1]).expect("chunk").encode();
        truncated.pop();
        assert!(matches!(
            Chunk::decode(&truncated),
            Err(ChunkError::Truncated)
        ));

        let mut trailing = Chunk::new(hash, 0, vec![1]).expect("chunk").encode();
        trailing.push(9);
        assert!(matches!(
            Chunk::decode(&trailing),
            Err(ChunkError::TrailingBytes)
        ));
    }

    #[test]
    fn decoder_never_panics_for_arbitrary_input() {
        for length in 0..=64 {
            let bytes = vec![0xa5_u8; length];
            let _ = Chunk::decode(&bytes);
        }
    }
}
