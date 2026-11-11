use std::sync::Arc;
use std::time::Duration;

use quinn::crypto::rustls::{QuicClientConfig, QuicServerConfig};
use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified};
use rustls::crypto::WebPkiSupportedAlgorithms;
use rustls::pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer, ServerName, UnixTime};
use rustls::server::danger::{ClientCertVerified, ClientCertVerifier};
use rustls::{
    CertificateError, DigitallySignedStruct, DistinguishedName, Error as RustlsError,
    SignatureScheme,
};
use thiserror::Error;
use ticonnect_core::fingerprint::Fingerprint;
use ticonnect_core::identity::IdentityKeyPair;
use x509_parser::prelude::*;

use crate::pairing::PAIRING_ALPN;

pub const SESSION_ALPN: &[u8] = b"ticonnect/1";
pub const TLS_IDLE_TIMEOUT: Duration = Duration::from_secs(30);
pub const TLS_KEEP_ALIVE: Duration = Duration::from_secs(10);
pub const DATAGRAM_BUFFER: usize = 64 * 1024;
pub const ED25519_OID: &str = "1.3.101.112";

const PKCS8_ED25519_PREFIX: [u8; 16] = [
    0x30, 0x2e, 0x02, 0x01, 0x00, 0x30, 0x05, 0x06, 0x03, 0x2b, 0x65, 0x70, 0x04, 0x22, 0x04, 0x20,
];

#[derive(Debug, Error, PartialEq, Eq)]
pub enum TlsError {
    #[error("TLS configuration failed: {0}")]
    Config(String),

    #[error("certificate is not a valid self-signed Ed25519 identity certificate")]
    InvalidCertificate,

    #[error("certificate fingerprint does not match the pinned fingerprint")]
    PinMismatch,

    #[error("certificate generation failed: {0}")]
    Certificate(String),
}

#[derive(Debug)]
pub struct IdentityCertificate {
    certificate: CertificateDer<'static>,
    key: PrivateKeyDer<'static>,
    fingerprint: Fingerprint,
}

impl Clone for IdentityCertificate {
    fn clone(&self) -> Self {
        Self {
            certificate: self.certificate.clone(),
            key: self.key.clone_key(),
            fingerprint: self.fingerprint,
        }
    }
}

impl IdentityCertificate {
    pub fn from_identity(identity: &IdentityKeyPair) -> Result<Self, TlsError> {
        let mut pkcs8 = Vec::with_capacity(PKCS8_ED25519_PREFIX.len() + 32);
        pkcs8.extend_from_slice(&PKCS8_ED25519_PREFIX);
        pkcs8.extend_from_slice(&identity.secret_bytes());

        let key_pair = rcgen::KeyPair::from_pkcs8_der_and_sign_algo(
            &PrivatePkcs8KeyDer::from(pkcs8),
            &rcgen::PKCS_ED25519,
        )
        .map_err(|error| TlsError::Certificate(error.to_string()))?;
        let params = rcgen::CertificateParams::new(vec!["ticonnect".to_owned()])
            .map_err(|error| TlsError::Certificate(error.to_string()))?;
        let certificate = params
            .self_signed(&key_pair)
            .map_err(|error| TlsError::Certificate(error.to_string()))?;
        let certificate = CertificateDer::from(certificate.der().to_vec());
        let fingerprint = certificate_fingerprint(&certificate)?;

        let expected = Fingerprint::of(&identity.verifying_key());
        if fingerprint != expected {
            return Err(TlsError::InvalidCertificate);
        }

        Ok(Self {
            certificate,
            key: PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(key_pair.serialize_der())),
            fingerprint,
        })
    }

    pub fn certificate(&self) -> &CertificateDer<'static> {
        &self.certificate
    }

    pub fn key(&self) -> PrivateKeyDer<'static> {
        self.key.clone_key()
    }

    pub fn fingerprint(&self) -> Fingerprint {
        self.fingerprint
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClientAlpn {
    Session,
    Pairing,
}

impl ClientAlpn {
    fn value(self) -> &'static [u8] {
        match self {
            Self::Session => SESSION_ALPN,
            Self::Pairing => PAIRING_ALPN,
        }
    }
}

pub fn server_config(certificate: &IdentityCertificate) -> Result<quinn::ServerConfig, TlsError> {
    let provider = Arc::new(rustls::crypto::ring::default_provider());
    let mut config = rustls::ServerConfig::builder_with_provider(provider.clone())
        .with_protocol_versions(&[&rustls::version::TLS13])
        .map_err(config_error)?
        .with_client_cert_verifier(Arc::new(IdentityVerifier {
            algorithms: provider.signature_verification_algorithms,
        }))
        .with_single_cert(vec![certificate.certificate().clone()], certificate.key())
        .map_err(config_error)?;
    config.alpn_protocols = vec![SESSION_ALPN.to_vec(), PAIRING_ALPN.to_vec()];
    config.max_early_data_size = 0;

    let crypto = QuicServerConfig::try_from(config).map_err(config_error)?;
    let mut server = quinn::ServerConfig::with_crypto(Arc::new(crypto));
    server.transport_config(Arc::new(transport_config()?));
    Ok(server)
}

pub fn client_config(
    certificate: &IdentityCertificate,
    expected: Fingerprint,
    alpn: ClientAlpn,
) -> Result<quinn::ClientConfig, TlsError> {
    let provider = Arc::new(rustls::crypto::ring::default_provider());
    let mut config = rustls::ClientConfig::builder_with_provider(provider.clone())
        .with_protocol_versions(&[&rustls::version::TLS13])
        .map_err(config_error)?
        .dangerous()
        .with_custom_certificate_verifier(Arc::new(PinnedVerifier {
            expected,
            algorithms: provider.signature_verification_algorithms,
        }))
        .with_client_auth_cert(vec![certificate.certificate().clone()], certificate.key())
        .map_err(config_error)?;
    config.alpn_protocols = vec![alpn.value().to_vec()];
    config.enable_early_data = false;

    let crypto = QuicClientConfig::try_from(config).map_err(config_error)?;
    let mut client = quinn::ClientConfig::new(Arc::new(crypto));
    client.transport_config(Arc::new(transport_config()?));
    Ok(client)
}

pub fn transport_config() -> Result<quinn::TransportConfig, TlsError> {
    let mut transport = quinn::TransportConfig::default();
    transport.max_idle_timeout(Some(
        TLS_IDLE_TIMEOUT
            .try_into()
            .map_err(|error| TlsError::Config(format!("{error:?}")))?,
    ));
    transport.keep_alive_interval(Some(TLS_KEEP_ALIVE));
    transport.datagram_receive_buffer_size(Some(DATAGRAM_BUFFER));
    transport.datagram_send_buffer_size(DATAGRAM_BUFFER);
    Ok(transport)
}

pub fn certificate_fingerprint(der: &CertificateDer<'_>) -> Result<Fingerprint, TlsError> {
    let (_, certificate) =
        X509Certificate::from_der(der.as_ref()).map_err(|_| TlsError::InvalidCertificate)?;
    if certificate.public_key().algorithm.algorithm.to_id_string() != ED25519_OID {
        return Err(TlsError::InvalidCertificate);
    }
    let key = certificate.public_key().subject_public_key.data.as_ref();
    let key: [u8; 32] = key.try_into().map_err(|_| TlsError::InvalidCertificate)?;
    Ok(Fingerprint::from_bytes(*blake3::hash(&key).as_bytes()))
}

pub fn validate_certificate(der: &CertificateDer<'_>) -> Result<Fingerprint, TlsError> {
    let (_, certificate) =
        X509Certificate::from_der(der.as_ref()).map_err(|_| TlsError::InvalidCertificate)?;
    if certificate.subject() != certificate.issuer() {
        return Err(TlsError::InvalidCertificate);
    }
    if !certificate.validity().is_valid() {
        return Err(TlsError::InvalidCertificate);
    }
    certificate
        .verify_signature(None)
        .map_err(|_| TlsError::InvalidCertificate)?;
    certificate_fingerprint(der)
}

pub fn pin_certificate(der: &CertificateDer<'_>, expected: Fingerprint) -> Result<(), TlsError> {
    if validate_certificate(der)? == expected {
        Ok(())
    } else {
        Err(TlsError::PinMismatch)
    }
}

#[derive(Debug)]
struct PinnedVerifier {
    expected: Fingerprint,
    algorithms: WebPkiSupportedAlgorithms,
}

impl rustls::client::danger::ServerCertVerifier for PinnedVerifier {
    fn verify_server_cert(
        &self,
        end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _server_name: &ServerName<'_>,
        _ocsp_response: &[u8],
        _now: UnixTime,
    ) -> Result<ServerCertVerified, RustlsError> {
        pin_certificate(end_entity, self.expected)
            .map(|()| ServerCertVerified::assertion())
            .map_err(|_| {
                RustlsError::InvalidCertificate(CertificateError::ApplicationVerificationFailure)
            })
    }

    fn verify_tls12_signature(
        &self,
        _message: &[u8],
        _cert: &CertificateDer<'_>,
        _dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, RustlsError> {
        Err(RustlsError::PeerIncompatible(
            rustls::PeerIncompatible::Tls12NotOffered,
        ))
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, RustlsError> {
        rustls::crypto::verify_tls13_signature(message, cert, dss, &self.algorithms)
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        self.algorithms.supported_schemes()
    }
}

#[derive(Debug)]
struct IdentityVerifier {
    algorithms: WebPkiSupportedAlgorithms,
}

impl ClientCertVerifier for IdentityVerifier {
    fn offer_client_auth(&self) -> bool {
        true
    }

    fn client_auth_mandatory(&self) -> bool {
        true
    }

    fn root_hint_subjects(&self) -> &[DistinguishedName] {
        &[]
    }

    fn verify_client_cert(
        &self,
        end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _now: UnixTime,
    ) -> Result<ClientCertVerified, RustlsError> {
        validate_certificate(end_entity)
            .map(|_| ClientCertVerified::assertion())
            .map_err(|_| RustlsError::InvalidCertificate(CertificateError::BadEncoding))
    }

    fn verify_tls12_signature(
        &self,
        _message: &[u8],
        _cert: &CertificateDer<'_>,
        _dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, RustlsError> {
        Err(RustlsError::PeerIncompatible(
            rustls::PeerIncompatible::Tls12NotOffered,
        ))
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, RustlsError> {
        rustls::crypto::verify_tls13_signature(message, cert, dss, &self.algorithms)
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        self.algorithms.supported_schemes()
    }
}

fn config_error(error: impl std::fmt::Display) -> TlsError {
    TlsError::Config(error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identity_certificate_uses_the_identity_key() {
        let identity = IdentityKeyPair::from_secret_bytes([61_u8; 32]);
        let certificate = IdentityCertificate::from_identity(&identity).expect("certificate");

        assert_eq!(
            certificate.fingerprint(),
            Fingerprint::of(&identity.verifying_key())
        );
        assert_eq!(
            certificate_fingerprint(certificate.certificate()).expect("fingerprint"),
            certificate.fingerprint()
        );
        assert_eq!(
            validate_certificate(certificate.certificate()).expect("validate"),
            certificate.fingerprint()
        );
    }

    #[test]
    fn pinning_accepts_only_the_expected_fingerprint() {
        let identity = IdentityKeyPair::from_secret_bytes([62_u8; 32]);
        let certificate = IdentityCertificate::from_identity(&identity).expect("certificate");
        let other =
            Fingerprint::of(&IdentityKeyPair::from_secret_bytes([63_u8; 32]).verifying_key());

        pin_certificate(certificate.certificate(), certificate.fingerprint()).expect("pin");
        assert_eq!(
            pin_certificate(certificate.certificate(), other),
            Err(TlsError::PinMismatch)
        );
    }

    #[test]
    fn server_requires_client_certificates() {
        let provider = rustls::crypto::ring::default_provider();
        let verifier = IdentityVerifier {
            algorithms: provider.signature_verification_algorithms,
        };
        assert!(verifier.offer_client_auth());
        assert!(verifier.client_auth_mandatory());
    }

    #[test]
    fn tls_configs_build_with_tls13_and_alpn() {
        let identity = IdentityKeyPair::from_secret_bytes([64_u8; 32]);
        let certificate = IdentityCertificate::from_identity(&identity).expect("certificate");

        server_config(&certificate).expect("server config");
        client_config(&certificate, certificate.fingerprint(), ClientAlpn::Session)
            .expect("client config");
        client_config(&certificate, certificate.fingerprint(), ClientAlpn::Pairing)
            .expect("pairing config");
    }
}
