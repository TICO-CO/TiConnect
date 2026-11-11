use ticonnect_core::identity::IdentityKeyPair;

#[test]
fn generated_keypair_has_public_key() {
    let pair = IdentityKeyPair::generate();
    let public = pair.verifying_key();

    assert_eq!(public.as_bytes().len(), 32);
}

#[test]
fn keypair_round_trips_through_secret_bytes() {
    let pair = IdentityKeyPair::from_secret_bytes([11_u8; 32]);
    let restored = IdentityKeyPair::from_secret_bytes(pair.secret_bytes());

    assert_eq!(pair.verifying_key(), restored.verifying_key());
}
