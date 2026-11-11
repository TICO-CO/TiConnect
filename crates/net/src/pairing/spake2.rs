use spake2::{Ed25519Group, Identity, Password, Spake2};
use subtle::ConstantTimeEq;
use thiserror::Error;
use ticonnect_core::canonical::{CanonicalError, Encoder};
use ticonnect_core::wire::PROTOCOL_VERSION;

pub const TRANSCRIPT_DOMAIN: &str = "pairing-transcript";
pub const CONFIRM_DOMAIN: &[u8] = b"ticonnect pairing confirmation v1";
pub const MAX_SPAKE2_MESSAGE: usize = 64;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PairingMode {
    Pin,
    Qr,
}

impl PairingMode {
    pub fn as_u8(self) -> u8 {
        match self {
            Self::Pin => 1,
            Self::Qr => 2,
        }
    }

    pub fn from_u8(value: u8) -> Option<Self> {
        match value {
            1 => Some(Self::Pin),
            2 => Some(Self::Qr),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    Initiator,
    Responder,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PeerIdentity {
    pub node_id: String,
    pub fingerprint: String,
    pub certificate_fingerprint: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PairingTranscript {
    pub mode: PairingMode,
    pub initiator: PeerIdentity,
    pub responder: PeerIdentity,
    pub token: Option<[u8; 32]>,
}

impl PairingTranscript {
    pub fn new(
        mode: PairingMode,
        initiator: PeerIdentity,
        responder: PeerIdentity,
        token: Option<[u8; 32]>,
    ) -> Self {
        Self {
            mode,
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
            .u8(self.mode.as_u8())
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
        match self.token {
            Some(token) => {
                encoder.u8(1).map_err(Spake2Error::Transcript)?;
                encoder.bytes(&token).map_err(Spake2Error::Transcript)?;
            }
            None => {
                encoder.u8(0).map_err(Spake2Error::Transcript)?;
            }
        }
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
pub enum PairingSecret {
    Pin(String),
    Token([u8; 32]),
}

impl PairingSecret {
    pub fn as_bytes(&self) -> &[u8] {
        match self {
            Self::Pin(pin) => pin.as_bytes(),
            Self::Token(token) => token,
        }
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum Spake2Error {
    #[error("pairing transcript is invalid: {0}")]
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
        secret: &PairingSecret,
        transcript: &PairingTranscript,
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
        peer_message: &[u8],
        transcript_hash: &[u8; 32],
    ) -> Result<Confirmation, Spake2Error> {
        if peer_message.len() > MAX_SPAKE2_MESSAGE {
            return Err(Spake2Error::MessageTooLarge);
        }
        let state = self.state.take().ok_or_else(|| {
            Spake2Error::Exchange("SPAKE2 session was already finished".to_owned())
        })?;
        let shared = state
            .finish(peer_message)
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

    #[test]
    fn same_pin_derives_matching_confirmation() {
        let transcript = fixture(PairingMode::Pin, None);
        let hash = transcript.hash().expect("transcript");
        let secret = PairingSecret::Pin("123456".to_owned());

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
    fn different_pins_derive_different_confirmation() {
        let transcript = fixture(PairingMode::Pin, None);
        let hash = transcript.hash().expect("transcript");
        let initiator_secret = PairingSecret::Pin("123456".to_owned());
        let responder_secret = PairingSecret::Pin("654321".to_owned());

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
    fn qr_token_is_part_of_the_transcript() {
        let token = [9_u8; 32];
        let first = fixture(PairingMode::Qr, Some(token));
        let second = fixture(PairingMode::Qr, Some([8_u8; 32]));

        assert_ne!(first.hash().expect("hash"), second.hash().expect("hash"));
    }

    fn fixture(mode: PairingMode, token: Option<[u8; 32]>) -> PairingTranscript {
        PairingTranscript::new(
            mode,
            PeerIdentity {
                node_id: "018f0000-0000-7000-8000-000000000001".to_owned(),
                fingerprint: "11".repeat(32),
                certificate_fingerprint: "22".repeat(32),
            },
            PeerIdentity {
                node_id: "018f0000-0000-7000-8000-000000000002".to_owned(),
                fingerprint: "33".repeat(32),
                certificate_fingerprint: "44".repeat(32),
            },
            token,
        )
    }
}
