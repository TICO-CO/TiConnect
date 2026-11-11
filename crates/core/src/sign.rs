use ed25519_dalek::{Signature, VerifyingKey};
use thiserror::Error;

use crate::identity::IdentityKeyPair;

pub const MAX_SIGNED_PAYLOAD: usize = 64 * 1024;

const MAGIC: &[u8; 4] = b"TICS";
const VERSION: u8 = 1;
const HEADER_LEN: usize = MAGIC.len() + 1 + 4;
const CONTEXT: &[u8] = b"TiConnect signed envelope v1\0";

#[derive(Debug, Error, PartialEq, Eq)]
pub enum SignError {
    #[error("signed payload exceeds {MAX_SIGNED_PAYLOAD} bytes")]
    PayloadTooLarge,

    #[error("signed envelope is truncated")]
    Truncated,

    #[error("signed envelope has trailing bytes")]
    TrailingBytes,

    #[error("signed envelope magic does not match")]
    BadMagic,

    #[error("signed envelope version {0} is not supported")]
    UnsupportedVersion(u8),

    #[error("signed envelope signature is invalid")]
    InvalidSignature,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignedEnvelope {
    payload: Vec<u8>,
    signature: [u8; 64],
}

impl SignedEnvelope {
    pub fn sign(identity: &IdentityKeyPair, payload: &[u8]) -> Result<Self, SignError> {
        if payload.len() > MAX_SIGNED_PAYLOAD {
            return Err(SignError::PayloadTooLarge);
        }

        let signature = identity.sign(&signed_message(payload));
        Ok(Self {
            payload: payload.to_vec(),
            signature: signature.to_bytes(),
        })
    }

    pub fn verify(&self, public_key: &VerifyingKey) -> Result<(), SignError> {
        let signature = Signature::from_bytes(&self.signature);
        public_key
            .verify_strict(&signed_message(&self.payload), &signature)
            .map_err(|_| SignError::InvalidSignature)
    }

    pub fn payload(&self) -> &[u8] {
        &self.payload
    }

    pub fn signature(&self) -> &[u8; 64] {
        &self.signature
    }

    pub fn encode(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(HEADER_LEN + self.payload.len() + 64);
        bytes.extend_from_slice(MAGIC);
        bytes.push(VERSION);
        bytes.extend_from_slice(
            &u32::try_from(self.payload.len())
                .unwrap_or(u32::MAX)
                .to_be_bytes(),
        );
        bytes.extend_from_slice(&self.payload);
        bytes.extend_from_slice(&self.signature);
        bytes
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, SignError> {
        if bytes.len() < HEADER_LEN {
            return Err(SignError::Truncated);
        }
        if &bytes[..MAGIC.len()] != MAGIC {
            return Err(SignError::BadMagic);
        }
        let version = bytes[MAGIC.len()];
        if version != VERSION {
            return Err(SignError::UnsupportedVersion(version));
        }

        let mut cursor = MAGIC.len() + 1;
        let payload_len = u32::from_be_bytes(
            bytes[cursor..cursor + 4]
                .try_into()
                .map_err(|_| SignError::Truncated)?,
        ) as usize;
        cursor += 4;

        if payload_len > MAX_SIGNED_PAYLOAD {
            return Err(SignError::PayloadTooLarge);
        }
        let end = cursor
            .checked_add(payload_len)
            .and_then(|value| value.checked_add(64))
            .ok_or(SignError::Truncated)?;
        if end > bytes.len() {
            return Err(SignError::Truncated);
        }
        if end < bytes.len() {
            return Err(SignError::TrailingBytes);
        }

        let payload = bytes[cursor..cursor + payload_len].to_vec();
        cursor += payload_len;
        let signature: [u8; 64] = bytes[cursor..cursor + 64]
            .try_into()
            .map_err(|_| SignError::Truncated)?;
        Ok(Self { payload, signature })
    }
}

fn signed_message(payload: &[u8]) -> Vec<u8> {
    let mut message = Vec::with_capacity(CONTEXT.len() + payload.len());
    message.extend_from_slice(CONTEXT);
    message.extend_from_slice(payload);
    message
}

pub fn verify(public_key: &VerifyingKey, payload: &[u8], signature: &[u8; 64]) -> bool {
    public_key
        .verify_strict(&signed_message(payload), &Signature::from_bytes(signature))
        .is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sign_and_verify_round_trip() {
        let identity = IdentityKeyPair::from_secret_bytes([17_u8; 32]);
        let envelope = SignedEnvelope::sign(&identity, b"pairing transcript").expect("sign");

        envelope
            .verify(&identity.verifying_key())
            .expect("verify own signature");
        assert_eq!(envelope.payload(), b"pairing transcript");
        assert!(verify(
            &identity.verifying_key(),
            envelope.payload(),
            envelope.signature()
        ));
    }

    #[test]
    fn tampered_payload_fails_verification() {
        let identity = IdentityKeyPair::from_secret_bytes([18_u8; 32]);
        let mut bytes = SignedEnvelope::sign(&identity, b"pairing transcript")
            .expect("sign")
            .encode();
        let payload_offset = HEADER_LEN;
        bytes[payload_offset] ^= 0xff;

        let envelope = SignedEnvelope::decode(&bytes).expect("decode");
        assert!(matches!(
            envelope.verify(&identity.verifying_key()),
            Err(SignError::InvalidSignature)
        ));
    }

    #[test]
    fn wrong_key_fails_verification() {
        let signer = IdentityKeyPair::from_secret_bytes([19_u8; 32]);
        let other = IdentityKeyPair::from_secret_bytes([20_u8; 32]);
        let envelope = SignedEnvelope::sign(&signer, b"payload").expect("sign");

        assert!(matches!(
            envelope.verify(&other.verifying_key()),
            Err(SignError::InvalidSignature)
        ));
    }

    #[test]
    fn rejects_malformed_envelopes() {
        let identity = IdentityKeyPair::from_secret_bytes([21_u8; 32]);
        let bytes = SignedEnvelope::sign(&identity, b"payload")
            .expect("sign")
            .encode();

        assert!(matches!(
            SignedEnvelope::decode(&bytes[..HEADER_LEN - 1]),
            Err(SignError::Truncated)
        ));

        let mut bad_magic = bytes.clone();
        bad_magic[0] ^= 0xff;
        assert!(matches!(
            SignedEnvelope::decode(&bad_magic),
            Err(SignError::BadMagic)
        ));

        let mut bad_version = bytes.clone();
        bad_version[MAGIC.len()] = 2;
        assert!(matches!(
            SignedEnvelope::decode(&bad_version),
            Err(SignError::UnsupportedVersion(2))
        ));

        let mut trailing = bytes;
        trailing.push(0);
        assert!(matches!(
            SignedEnvelope::decode(&trailing),
            Err(SignError::TrailingBytes)
        ));
    }
}
