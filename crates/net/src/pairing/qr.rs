use std::net::IpAddr;
use std::str::FromStr;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use rand_core::{OsRng, RngCore};
use thiserror::Error;
use ticonnect_core::fingerprint::{Fingerprint, FingerprintError};
use ticonnect_core::id::NodeId;

pub const QR_PREFIX: &str = "ticonnect://pair/v1";
pub const TOKEN_BYTES: usize = 32;
pub const TOKEN_TTL: Duration = Duration::from_secs(120);
pub const MAX_QR_LEN: usize = 1024;

const NODE_ID: &str = "node_id";
const FINGERPRINT: &str = "fp";
const ADDRESS: &str = "addr";
const PORT: &str = "port";
const TOKEN: &str = "token";
const EXPIRES: &str = "exp";

#[derive(Debug, Error, PartialEq, Eq)]
pub enum QrError {
    #[error("QR payload exceeds {MAX_QR_LEN} bytes")]
    TooLong,

    #[error("QR payload prefix is invalid")]
    BadPrefix,

    #[error("QR payload is missing {0}")]
    MissingField(&'static str),

    #[error("QR payload has an unknown field")]
    UnknownField,

    #[error("QR payload repeats a field")]
    DuplicateField,

    #[error("QR payload node id is invalid")]
    InvalidNodeId,

    #[error("QR payload fingerprint is invalid")]
    InvalidFingerprint,

    #[error("QR payload address is invalid")]
    InvalidAddress,

    #[error("QR payload port is invalid")]
    InvalidPort,

    #[error("QR payload token is invalid")]
    InvalidToken,

    #[error("QR payload expiry is invalid")]
    InvalidExpiry,

    #[error("QR payload has expired")]
    Expired,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QrPayload {
    node_id: NodeId,
    fingerprint: Fingerprint,
    address: IpAddr,
    port: u16,
    token: [u8; TOKEN_BYTES],
    expires_at: u64,
}

impl QrPayload {
    pub fn generate(
        node_id: NodeId,
        fingerprint: Fingerprint,
        address: IpAddr,
        port: u16,
    ) -> Result<Self, QrError> {
        Self::generate_with_ttl(node_id, fingerprint, address, port, TOKEN_TTL)
    }

    pub fn generate_with_ttl(
        node_id: NodeId,
        fingerprint: Fingerprint,
        address: IpAddr,
        port: u16,
        ttl: Duration,
    ) -> Result<Self, QrError> {
        if port == 0 {
            return Err(QrError::InvalidPort);
        }
        let mut token = [0_u8; TOKEN_BYTES];
        OsRng.fill_bytes(&mut token);
        Ok(Self {
            node_id,
            fingerprint,
            address,
            port,
            token,
            expires_at: now_unix().saturating_add(ttl.as_secs()),
        })
    }

    pub fn node_id(&self) -> NodeId {
        self.node_id
    }

    pub fn fingerprint(&self) -> Fingerprint {
        self.fingerprint
    }

    pub fn address(&self) -> IpAddr {
        self.address
    }

    pub fn port(&self) -> u16 {
        self.port
    }

    pub fn token(&self) -> &[u8; TOKEN_BYTES] {
        &self.token
    }

    pub fn token_hex(&self) -> String {
        hex::encode(self.token)
    }

    pub fn expires_at_unix(&self) -> u64 {
        self.expires_at
    }

    pub fn is_expired(&self, now_unix: u64) -> bool {
        now_unix >= self.expires_at
    }

    pub fn encode(&self) -> String {
        format!(
            "{QR_PREFIX}?{NODE_ID}={}&{FINGERPRINT}={}&{ADDRESS}={}&{PORT}={}&{TOKEN}={}&{EXPIRES}={}",
            self.node_id,
            self.fingerprint.to_hex(),
            self.address,
            self.port,
            self.token_hex(),
            self.expires_at,
        )
    }

    pub fn decode(value: &str) -> Result<Self, QrError> {
        if value.len() > MAX_QR_LEN {
            return Err(QrError::TooLong);
        }
        let rest = value
            .strip_prefix(QR_PREFIX)
            .and_then(|rest| rest.strip_prefix('?'))
            .ok_or(QrError::BadPrefix)?;

        let mut node_id = None;
        let mut fingerprint = None;
        let mut address = None;
        let mut port = None;
        let mut token = None;
        let mut expires_at = None;

        for pair in rest.split('&') {
            let (key, value) = pair.split_once('=').ok_or(QrError::UnknownField)?;
            match key {
                NODE_ID => {
                    if node_id.is_some() {
                        return Err(QrError::DuplicateField);
                    }
                    node_id = Some(NodeId::from_str(value).map_err(|_| QrError::InvalidNodeId)?);
                }
                FINGERPRINT => {
                    if fingerprint.is_some() {
                        return Err(QrError::DuplicateField);
                    }
                    fingerprint = Some(Fingerprint::from_str(value).map_err(qr_fingerprint_error)?);
                }
                ADDRESS => {
                    if address.is_some() {
                        return Err(QrError::DuplicateField);
                    }
                    address = Some(IpAddr::from_str(value).map_err(|_| QrError::InvalidAddress)?);
                }
                PORT => {
                    if port.is_some() {
                        return Err(QrError::DuplicateField);
                    }
                    let parsed = value.parse::<u16>().map_err(|_| QrError::InvalidPort)?;
                    if parsed == 0 {
                        return Err(QrError::InvalidPort);
                    }
                    port = Some(parsed);
                }
                TOKEN => {
                    if token.is_some() {
                        return Err(QrError::DuplicateField);
                    }
                    let decoded = hex::decode(value).map_err(|_| QrError::InvalidToken)?;
                    token = Some(decoded.try_into().map_err(|_| QrError::InvalidToken)?);
                }
                EXPIRES => {
                    if expires_at.is_some() {
                        return Err(QrError::DuplicateField);
                    }
                    let parsed = value.parse::<u64>().map_err(|_| QrError::InvalidExpiry)?;
                    if parsed == 0 {
                        return Err(QrError::InvalidExpiry);
                    }
                    expires_at = Some(parsed);
                }
                _ => return Err(QrError::UnknownField),
            }
        }

        Ok(Self {
            node_id: node_id.ok_or(QrError::MissingField(NODE_ID))?,
            fingerprint: fingerprint.ok_or(QrError::MissingField(FINGERPRINT))?,
            address: address.ok_or(QrError::MissingField(ADDRESS))?,
            port: port.ok_or(QrError::MissingField(PORT))?,
            token: token.ok_or(QrError::MissingField(TOKEN))?,
            expires_at: expires_at.ok_or(QrError::MissingField(EXPIRES))?,
        })
    }
}

fn qr_fingerprint_error(_: FingerprintError) -> QrError {
    QrError::InvalidFingerprint
}

fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_secs())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::Ipv4Addr;

    #[test]
    fn qr_payload_round_trips() {
        let payload = fixture();
        let decoded = QrPayload::decode(&payload.encode()).expect("decode");

        assert_eq!(decoded, payload);
        assert_eq!(decoded.port(), 45871);
        assert_eq!(decoded.token().len(), TOKEN_BYTES);
    }

    #[test]
    fn qr_payload_rejects_malformed_values() {
        let payload = fixture();
        let encoded = payload.encode();

        assert_eq!(QrPayload::decode("nope"), Err(QrError::BadPrefix));
        assert_eq!(
            QrPayload::decode(&encoded.replace("token=", "tokn=")),
            Err(QrError::UnknownField)
        );
        assert_eq!(
            QrPayload::decode(&encoded.replace("port=45871", "port=0")),
            Err(QrError::InvalidPort)
        );
        assert_eq!(
            QrPayload::decode(&encoded.replace("token=", "token=00")),
            Err(QrError::InvalidToken)
        );
        assert_eq!(
            QrPayload::decode(&format!("{encoded}&node_id={}", payload.node_id())),
            Err(QrError::DuplicateField)
        );
    }

    #[test]
    fn qr_payload_reports_missing_fields() {
        let encoded = fixture().encode();
        let without_fp = encoded
            .split('&')
            .filter(|part| !part.starts_with("fp="))
            .collect::<Vec<_>>()
            .join("&");
        assert_eq!(
            QrPayload::decode(&without_fp),
            Err(QrError::MissingField("fp"))
        );
    }

    #[test]
    fn qr_payload_expires() {
        let payload = fixture();
        assert!(!payload.is_expired(now_unix()));
        assert!(payload.is_expired(payload.expires_at_unix()));
    }

    fn fixture() -> QrPayload {
        let fingerprint = Fingerprint::from_bytes([7_u8; 32]);
        QrPayload::generate_with_ttl(
            NodeId::generate(),
            fingerprint,
            IpAddr::V4(Ipv4Addr::new(192, 168, 1, 20)),
            45871,
            Duration::from_secs(60),
        )
        .expect("qr payload")
    }
}
