use ed25519_dalek::{Signature, Signer, SigningKey, VerifyingKey};
use rand_core::OsRng;

#[derive(Clone)]
pub struct IdentityKeyPair {
    signing: SigningKey,
}

impl IdentityKeyPair {
    pub fn generate() -> Self {
        Self {
            signing: SigningKey::generate(&mut OsRng),
        }
    }

    pub fn from_secret_bytes(bytes: [u8; 32]) -> Self {
        Self {
            signing: SigningKey::from_bytes(&bytes),
        }
    }

    pub fn secret_bytes(&self) -> [u8; 32] {
        self.signing.to_bytes()
    }

    pub fn verifying_key(&self) -> VerifyingKey {
        self.signing.verifying_key()
    }

    pub fn sign(&self, message: &[u8]) -> Signature {
        self.signing.sign(message)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generated_pair_round_trips_through_secret_bytes() {
        let pair = IdentityKeyPair::generate();
        let restored = IdentityKeyPair::from_secret_bytes(pair.secret_bytes());

        assert_eq!(pair.verifying_key(), restored.verifying_key());
    }

    #[test]
    fn fixed_secret_derives_stable_public_key() {
        let first = IdentityKeyPair::from_secret_bytes([7_u8; 32]);
        let second = IdentityKeyPair::from_secret_bytes([7_u8; 32]);

        assert_eq!(first.verifying_key(), second.verifying_key());
    }
}
