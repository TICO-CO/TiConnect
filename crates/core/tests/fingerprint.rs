use ticonnect_core::fingerprint::Fingerprint;
use ticonnect_core::identity::IdentityKeyPair;

#[test]
fn fingerprint_is_stable_and_derived_from_public_key() {
    let pair = IdentityKeyPair::from_secret_bytes([5_u8; 32]);
    let first = Fingerprint::of(&pair.verifying_key());
    let second = Fingerprint::of(&pair.verifying_key());

    assert_eq!(first, second);
    assert_eq!(
        first.as_bytes(),
        blake3::hash(pair.verifying_key().as_bytes()).as_bytes()
    );
}

#[test]
fn fingerprint_display_is_grouped_hex() {
    let pair = IdentityKeyPair::from_secret_bytes([6_u8; 32]);
    let display = Fingerprint::of(&pair.verifying_key()).to_string();

    assert_eq!(display.len(), 79);
    assert_eq!(display.matches(' ').count(), 15);
}
