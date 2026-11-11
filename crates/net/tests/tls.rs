use ticonnect_core::fingerprint::Fingerprint;
use ticonnect_core::identity::IdentityKeyPair;
use ticonnect_net::tls::{
    ClientAlpn, IdentityCertificate, TlsError, certificate_fingerprint, client_config,
    pin_certificate, server_config, validate_certificate,
};

#[test]
fn generated_certificate_is_bound_to_the_identity_key() {
    let identity = IdentityKeyPair::from_secret_bytes([81_u8; 32]);
    let certificate = IdentityCertificate::from_identity(&identity).expect("certificate");
    let expected = Fingerprint::of(&identity.verifying_key());

    assert_eq!(certificate.fingerprint(), expected);
    assert_eq!(
        certificate_fingerprint(certificate.certificate()).expect("fingerprint"),
        expected
    );
    assert_eq!(
        validate_certificate(certificate.certificate()).expect("validate"),
        expected
    );
}

#[test]
fn pinned_certificate_requires_the_expected_fingerprint() {
    let identity = IdentityKeyPair::from_secret_bytes([82_u8; 32]);
    let certificate = IdentityCertificate::from_identity(&identity).expect("certificate");
    let other = Fingerprint::of(&IdentityKeyPair::from_secret_bytes([83_u8; 32]).verifying_key());

    pin_certificate(certificate.certificate(), certificate.fingerprint()).expect("pin");
    assert_eq!(
        pin_certificate(certificate.certificate(), other),
        Err(TlsError::PinMismatch)
    );
}

#[test]
fn tls_configs_offer_session_and_pairing_alpn() {
    let identity = IdentityKeyPair::from_secret_bytes([84_u8; 32]);
    let certificate = IdentityCertificate::from_identity(&identity).expect("certificate");

    server_config(&certificate).expect("server config");
    client_config(&certificate, certificate.fingerprint(), ClientAlpn::Session)
        .expect("session config");
    client_config(&certificate, certificate.fingerprint(), ClientAlpn::Pairing)
        .expect("pairing config");
}
