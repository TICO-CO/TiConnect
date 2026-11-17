use spake2::{Ed25519Group, Identity, Password, Spake2};
use subtle::ConstantTimeEq;
use thiserror::Error;
use ticonnect_core::canonical::{CanonicalError, Encoder};
use ticonnect_core::wire::PROTOCOL_VERSION;

pub const TRANSCRIPT_DOMAIN: &str = "trust-transcript";
pub const CONFIRM_DOMAIN: &[u8] = b"ticonnect trust confirmation v1";
pub const MAX_SPAKE2_MESSAGE: usize = 64;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    Initiator,
    Responder,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UserIdentity {
    pub node_id: String,
    pub fingerprint: String,
    pub certificate_fingerprint: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrustTranscript {
    pub initiator: UserIdentity,
    pub responder: UserIdentity,
    pub token: [u8; 32],
}

impl TrustTranscript {
    pub fn new(initiator: UserIdentity, responder: UserIdentity, token: [u8; 32]) -> Self {
        Self {
            initiator,
            responder,
            token,
        }
    }

    pub fn hash(&self) -> Result<[u8; 32], Spake2Error> {
        let mut encoder = Encoder::new(TRANSCRIPT_DOMAIN).map_err(Spake2Error::Transcript)?;
        encoder
            .u16(PROTOCOL_VERSION)
            .map_err(Spake2Error::Transcript)?;
        encoder
            .string(&self.initiator.node_id)
            .map_err(Spake2Error::Transcript)?;
        encoder
            .string(&self.initiator.fingerprint)
            .map_err(Spake2Error::Transcript)?;
        encoder
            .string(&self.initiator.certificate_fingerprint)
            .map_err(Spake2Error::Transcript)?;
        encoder
            .string(&self.responder.node_id)
            .map_err(Spake2Error::Transcript)?;
        encoder
            .string(&self.responder.fingerprint)
            .map_err(Spake2Error::Transcript)?;
        encoder
            .string(&self.responder.certificate_fingerprint)
            .map_err(Spake2Error::Transcript)?;
        encoder
            .bytes(&self.token)
            .map_err(Spake2Error::Transcript)?;
        Ok(*blake3::hash(&encoder.finish()).as_bytes())
    }

    fn identity_strings(&self) -> (String, String) {
        (
            self.initiator.node_id.clone(),
            self.responder.node_id.clone(),
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrustSecret([u8; 32]);

impl TrustSecret {
    pub fn new(token: [u8; 32]) -> Self {
        Self(token)
    }

    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum Spake2Error {
    #[error("trust transcript is invalid: {0}")]
    Transcript(CanonicalError),

    #[error("SPAKE2 message is too large")]
    MessageTooLarge,

    #[error("SPAKE2 exchange failed: {0}")]
    Exchange(String),
}

pub struct Spake2Session {
    state: Option<Spake2<Ed25519Group>>,
}

impl Spake2Session {
    pub fn start(
        role: Role,
        secret: &TrustSecret,
        transcript: &TrustTranscript,
    ) -> Result<(Self, Vec<u8>), Spake2Error> {
        let (initiator, responder) = transcript.identity_strings();
        let password = Password::new(secret.as_bytes());
        let initiator = Identity::new(initiator.as_bytes());
        let responder = Identity::new(responder.as_bytes());

        let (state, message) = match role {
            Role::Initiator => Spake2::<Ed25519Group>::start_a(&password, &initiator, &responder),
            Role::Responder => Spake2::<Ed25519Group>::start_b(&password, &initiator, &responder),
        };
        Ok((Self { state: Some(state) }, message))
    }

    pub fn finish(
        mut self,
        user_message: &[u8],
        transcript_hash: &[u8; 32],
    ) -> Result<Confirmation, Spake2Error> {
        if user_message.len() > MAX_SPAKE2_MESSAGE {
            return Err(Spake2Error::MessageTooLarge);
        }
        let state = self.state.take().ok_or_else(|| {
            Spake2Error::Exchange("SPAKE2 session was already finished".to_owned())
        })?;
        let shared = state
            .finish(user_message)
            .map_err(|error| Spake2Error::Exchange(error.to_string()))?;

        let mut hasher = blake3::Hasher::new();
        hasher.update(CONFIRM_DOMAIN);
        hasher.update(&shared);
        hasher.update(transcript_hash);
        Ok(Confirmation(*hasher.finalize().as_bytes()))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Confirmation([u8; 32]);

impl Confirmation {
    pub fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    pub fn matches(&self, other: &Self) -> bool {
        bool::from(self.0.ct_eq(&other.0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> TrustTranscript {
        TrustTranscript::new(
            UserIdentity {
                node_id: "018f0000-0000-7000-8000-000000000001".to_owned(),
                fingerprint: "11".repeat(32),
                certificate_fingerprint: "22".repeat(32),
            },
            UserIdentity {
                node_id: "018f0000-0000-7000-8000-000000000002".to_owned(),
                fingerprint: "33".repeat(32),
                certificate_fingerprint: "44".repeat(32),
            },
            [9_u8; 32],
        )
    }

    #[test]
    fn matching_tokens_derive_matching_confirmation() {
        let transcript = fixture();
        let hash = transcript.hash().expect("transcript");
        let secret = TrustSecret::new([9_u8; 32]);

        let (initiator, initiator_message) =
            Spake2Session::start(Role::Initiator, &secret, &transcript).expect("initiator");
        let (responder, responder_message) =
            Spake2Session::start(Role::Responder, &secret, &transcript).expect("responder");

        let initiator_confirmation = initiator
            .finish(&responder_message, &hash)
            .expect("initiator confirm");
        let responder_confirmation = responder
            .finish(&initiator_message, &hash)
            .expect("responder confirm");

        assert!(initiator_confirmation.matches(&responder_confirmation));
    }

    #[test]
    fn different_tokens_derive_different_confirmation() {
        let transcript = fixture();
        let hash = transcript.hash().expect("transcript");
        let initiator_secret = TrustSecret::new([9_u8; 32]);
        let responder_secret = TrustSecret::new([8_u8; 32]);

        let (initiator, initiator_message) =
            Spake2Session::start(Role::Initiator, &initiator_secret, &transcript)
                .expect("initiator");
        let (responder, responder_message) =
            Spake2Session::start(Role::Responder, &responder_secret, &transcript)
                .expect("responder");

        let initiator_confirmation = initiator
            .finish(&responder_message, &hash)
            .expect("initiator confirm");
        let responder_confirmation = responder
            .finish(&initiator_message, &hash)
            .expect("responder confirm");

        assert!(!initiator_confirmation.matches(&responder_confirmation));
    }

    #[test]
    fn token_is_part_of_the_transcript() {
        let first = TrustTranscript::new(fixture().initiator, fixture().responder, [9_u8; 32]);
        let second = TrustTranscript::new(fixture().initiator, fixture().responder, [8_u8; 32]);

        assert_ne!(first.hash().expect("hash"), second.hash().expect("hash"));
    }
}
