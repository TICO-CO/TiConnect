use ticonnect_core::identity::IdentityKeyPair;
use ticonnect_core::sign::{SignError, SignedEnvelope};

#[test]
fn signed_envelope_verifies_with_the_signer_key() {
    let identity = IdentityKeyPair::from_secret_bytes([41_u8; 32]);
    let envelope = SignedEnvelope::sign(&identity, b"canonical transcript").expect("sign");

    envelope
        .verify(&identity.verifying_key())
        .expect("verify signature");
    assert_eq!(envelope.payload(), b"canonical transcript");
}

#[test]
fn signed_envelope_rejects_tampering_and_wrong_keys() {
    let identity = IdentityKeyPair::from_secret_bytes([42_u8; 32]);
    let other = IdentityKeyPair::from_secret_bytes([43_u8; 32]);
    let mut bytes = SignedEnvelope::sign(&identity, b"payload")
        .expect("sign")
        .encode();
    bytes[12] ^= 1;

    let envelope = SignedEnvelope::decode(&bytes).expect("decode");
    assert_eq!(
        envelope.verify(&identity.verifying_key()),
        Err(SignError::InvalidSignature)
    );
    assert_eq!(
        envelope.verify(&other.verifying_key()),
        Err(SignError::InvalidSignature)
    );
}

#[test]
fn signed_envelope_decode_rejects_bad_headers() {
    let identity = IdentityKeyPair::from_secret_bytes([44_u8; 32]);
    let bytes = SignedEnvelope::sign(&identity, b"payload")
        .expect("sign")
        .encode();

    assert!(matches!(
        SignedEnvelope::decode(&bytes[..8]),
        Err(SignError::Truncated)
    ));

    let mut bad_magic = bytes.clone();
    bad_magic[0] = b'X';
    assert!(matches!(
        SignedEnvelope::decode(&bad_magic),
        Err(SignError::BadMagic)
    ));

    let mut bad_version = bytes.clone();
    bad_version[4] = 9;
    assert!(matches!(
        SignedEnvelope::decode(&bad_version),
        Err(SignError::UnsupportedVersion(9))
    ));

    let mut trailing = bytes;
    trailing.push(0);
    assert!(matches!(
        SignedEnvelope::decode(&trailing),
        Err(SignError::TrailingBytes)
    ));
}
