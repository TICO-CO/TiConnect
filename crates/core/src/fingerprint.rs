use std::fmt;

use ed25519_dalek::VerifyingKey;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Fingerprint([u8; 32]);

impl Fingerprint {
    pub fn of(public_key: &VerifyingKey) -> Self {
        let hash = blake3::hash(public_key.as_bytes());
        Self(*hash.as_bytes())
    }

    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    pub fn to_hex(&self) -> String {
        hex::encode(self.0)
    }
}

impl fmt::Display for Fingerprint {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let hex = self.to_hex();
        for (index, chunk) in hex.as_bytes().chunks(4).enumerate() {
            if index > 0 {
                formatter.write_str(" ")?;
            }
            formatter.write_str(std::str::from_utf8(chunk).unwrap_or_default())?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::identity::IdentityKeyPair;

    #[test]
    fn fingerprint_is_blake3_of_public_key() {
        let pair = IdentityKeyPair::from_secret_bytes([3_u8; 32]);
        let public = pair.verifying_key();
        let fingerprint = Fingerprint::of(&public);

        assert_eq!(
            fingerprint.as_bytes(),
            blake3::hash(public.as_bytes()).as_bytes()
        );
        assert_eq!(fingerprint.to_hex().len(), 64);
    }

    #[test]
    fn display_is_grouped_lowercase_hex() {
        let pair = IdentityKeyPair::from_secret_bytes([9_u8; 32]);
        let fingerprint = Fingerprint::of(&pair.verifying_key()).to_string();

        assert_eq!(fingerprint.len(), 79);
        assert!(
            fingerprint
                .chars()
                .all(|character| { character.is_ascii_hexdigit() || character == ' ' })
        );
        assert_eq!(fingerprint, fingerprint.to_lowercase());
    }
}
