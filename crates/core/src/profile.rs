use std::str::FromStr;

use ed25519_dalek::VerifyingKey;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::id::NodeId;
use crate::identity::IdentityKeyPair;
use crate::sign::{MAX_SIGNED_PAYLOAD, SignError, SignedEnvelope};

pub const PROFILE_VERSION: u16 = 1;
pub const MAX_PROFILE_NAME: usize = 64;
pub const MAX_AVATAR_BYTES: usize = 160 * 1024;
pub const AVATAR_MIME_PNG: &str = "image/png";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProfileRecord {
    pub version: u16,
    pub node_id: String,
    pub display_name: String,
    pub avatar_hash: [u8; 32],
    pub avatar_mime: String,
    pub avatar: Vec<u8>,
}

impl ProfileRecord {
    pub fn new(
        node_id: impl Into<String>,
        display_name: impl Into<String>,
        avatar: impl Into<Vec<u8>>,
    ) -> Result<Self, ProfileError> {
        let avatar = avatar.into();
        let avatar_hash = if avatar.is_empty() {
            [0_u8; 32]
        } else {
            *blake3::hash(&avatar).as_bytes()
        };
        let record = Self {
            version: PROFILE_VERSION,
            node_id: node_id.into(),
            display_name: display_name.into(),
            avatar_hash,
            avatar_mime: if avatar.is_empty() {
                String::new()
            } else {
                AVATAR_MIME_PNG.to_owned()
            },
            avatar,
        };
        record.validate()?;
        Ok(record)
    }

    pub fn avatar_hash_hex(&self) -> Option<String> {
        (!self.avatar.is_empty()).then(|| hex::encode(self.avatar_hash))
    }

    pub fn validate(&self) -> Result<(), ProfileError> {
        if self.version != PROFILE_VERSION {
            return Err(ProfileError::UnsupportedVersion(self.version));
        }
        NodeId::from_str(&self.node_id)
            .map_err(|error| ProfileError::InvalidNodeId(error.to_string()))?;
        if self.display_name.len() > MAX_PROFILE_NAME {
            return Err(ProfileError::NameTooLong);
        }
        if self.avatar.len() > MAX_AVATAR_BYTES {
            return Err(ProfileError::AvatarTooLarge);
        }
        if self.avatar.is_empty() {
            if self.avatar_hash != [0_u8; 32] || !self.avatar_mime.is_empty() {
                return Err(ProfileError::InvalidAvatar);
            }
            return Ok(());
        }
        if self.avatar_mime != AVATAR_MIME_PNG {
            return Err(ProfileError::UnsupportedMime(self.avatar_mime.clone()));
        }
        if self.avatar_hash != *blake3::hash(&self.avatar).as_bytes() {
            return Err(ProfileError::AvatarHashMismatch);
        }
        Ok(())
    }

    pub fn encode(&self, identity: &IdentityKeyPair) -> Result<Vec<u8>, ProfileError> {
        self.validate()?;
        let payload =
            postcard::to_allocvec(self).map_err(|error| ProfileError::Encode(error.to_string()))?;
        if payload.len() > MAX_SIGNED_PAYLOAD {
            return Err(ProfileError::PayloadTooLarge);
        }
        SignedEnvelope::sign(identity, &payload)
            .map(|envelope| envelope.encode())
            .map_err(ProfileError::Sign)
    }

    pub fn decode(bytes: &[u8], public_key: &VerifyingKey) -> Result<Self, ProfileError> {
        let envelope = SignedEnvelope::decode(bytes).map_err(ProfileError::Sign)?;
        envelope.verify(public_key).map_err(ProfileError::Sign)?;
        let record: Self = postcard::from_bytes(envelope.payload())
            .map_err(|error| ProfileError::Decode(error.to_string()))?;
        record.validate()?;
        Ok(record)
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ProfileError {
    #[error("profile version {0} is not supported")]
    UnsupportedVersion(u16),

    #[error("profile node id is invalid: {0}")]
    InvalidNodeId(String),

    #[error("profile display name is too long")]
    NameTooLong,

    #[error("profile avatar exceeds {MAX_AVATAR_BYTES} bytes")]
    AvatarTooLarge,

    #[error("profile avatar metadata is invalid")]
    InvalidAvatar,

    #[error("profile avatar MIME type {0} is not supported")]
    UnsupportedMime(String),

    #[error("profile avatar hash does not match its bytes")]
    AvatarHashMismatch,

    #[error("profile payload is too large")]
    PayloadTooLarge,

    #[error("profile encoding failed: {0}")]
    Encode(String),

    #[error("profile decoding failed: {0}")]
    Decode(String),

    #[error(transparent)]
    Sign(#[from] SignError),
}

#[cfg(test)]
mod tests {
    use super::*;

    fn identity() -> IdentityKeyPair {
        IdentityKeyPair::from_secret_bytes([41_u8; 32])
    }

    #[test]
    fn profile_round_trips_and_verifies() {
        let identity = identity();
        let record = ProfileRecord::new(
            NodeId::generate().to_string(),
            "Ada",
            b"\x89PNG\r\n\x1a\navatar".to_vec(),
        )
        .expect("record");
        let bytes = record.encode(&identity).expect("encode");
        let decoded = ProfileRecord::decode(&bytes, &identity.verifying_key()).expect("decode");
        assert_eq!(decoded, record);
    }

    #[test]
    fn profile_rejects_wrong_key_and_tampering() {
        let identity = identity();
        let other = IdentityKeyPair::from_secret_bytes([42_u8; 32]);
        let record = ProfileRecord::new(NodeId::generate().to_string(), "Ada", b"png".to_vec())
            .expect("record");
        let bytes = record.encode(&identity).expect("encode");
        assert!(ProfileRecord::decode(&bytes, &other.verifying_key()).is_err());

        let mut tampered = bytes;
        let last = tampered.len() - 1;
        tampered[last] ^= 0xff;
        assert!(ProfileRecord::decode(&tampered, &identity.verifying_key()).is_err());
    }

    #[test]
    fn profile_rejects_oversized_avatar() {
        let record = ProfileRecord::new(
            NodeId::generate().to_string(),
            "Ada",
            vec![0_u8; MAX_AVATAR_BYTES + 1],
        );
        assert!(matches!(record, Err(ProfileError::AvatarTooLarge)));
    }
}
