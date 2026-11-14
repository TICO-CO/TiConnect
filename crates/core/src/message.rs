use std::str::FromStr;

use ed25519_dalek::VerifyingKey;
use thiserror::Error;
use uuid::Uuid;

use crate::attachment::{
    AttachmentError, AttachmentHash, AttachmentRef, validate_mime, validate_name,
};
use crate::canonical::{CanonicalError, Decoder, Encoder};
use crate::id::{IdError, MessageId, NodeId};
use crate::identity::IdentityKeyPair;
use crate::sign::{SignError, SignedEnvelope};
use crate::wire::PROTOCOL_VERSION;

pub const MESSAGE_DOMAIN: &str = "message-record";
pub const CONVERSATION_DOMAIN: &[u8] = b"TiConnect conversation id v1\0";
pub const MAX_MESSAGE_BODY: usize = 32 * 1024;
pub const MAX_MESSAGE_METADATA: usize = 256;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum MessageKind {
    Text = 1,
    Image = 2,
    Audio = 3,
    File = 4,
    Ack = 5,
}

impl MessageKind {
    pub fn as_u8(self) -> u8 {
        self as u8
    }

    pub fn from_u8(value: u8) -> Option<Self> {
        match value {
            1 => Some(Self::Text),
            2 => Some(Self::Image),
            3 => Some(Self::Audio),
            4 => Some(Self::File),
            5 => Some(Self::Ack),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Text => "text",
            Self::Image => "image",
            Self::Audio => "audio",
            Self::File => "file",
            Self::Ack => "ack",
        }
    }
}

impl FromStr for MessageKind {
    type Err = MessageError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "text" => Ok(Self::Text),
            "image" => Ok(Self::Image),
            "audio" => Ok(Self::Audio),
            "file" => Ok(Self::File),
            "ack" => Ok(Self::Ack),
            other => Err(MessageError::UnknownKind(other.to_owned())),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MessageRecord {
    pub message_id: MessageId,
    pub conversation_id: String,
    pub sender_id: String,
    pub seq: u64,
    pub sent_at: i64,
    pub kind: MessageKind,
    pub target_id: Option<String>,
    pub body: Option<String>,
    pub attachment_hash: Option<String>,
    pub attachment_name: Option<String>,
    pub attachment_size: Option<u64>,
    pub attachment_mime: Option<String>,
}

impl MessageRecord {
    pub fn new(
        conversation_id: impl Into<String>,
        sender_id: impl Into<String>,
        seq: u64,
        sent_at: i64,
        kind: MessageKind,
        body: Option<String>,
    ) -> Self {
        Self {
            message_id: MessageId::generate(),
            conversation_id: conversation_id.into(),
            sender_id: sender_id.into(),
            seq,
            sent_at,
            kind,
            target_id: None,
            body,
            attachment_hash: None,
            attachment_name: None,
            attachment_size: None,
            attachment_mime: None,
        }
    }

    pub fn with_attachment(mut self, attachment: &AttachmentRef) -> Result<Self, MessageError> {
        self.attachment_hash = Some(attachment.hash.to_hex());
        self.attachment_name = Some(attachment.name.clone());
        self.attachment_size = Some(attachment.size);
        self.attachment_mime = Some(attachment.mime.clone());
        self.kind = attachment.kind();
        self.validate()?;
        Ok(self)
    }

    pub fn attachment_ref(&self) -> Result<Option<AttachmentRef>, MessageError> {
        let Some(hash) = &self.attachment_hash else {
            return Ok(None);
        };
        let hash = hash
            .parse::<AttachmentHash>()
            .map_err(|_| MessageError::InvalidAttachmentHash)?;
        let name = self
            .attachment_name
            .clone()
            .ok_or(MessageError::InvalidAttachment)?;
        let size = self
            .attachment_size
            .ok_or(MessageError::InvalidAttachment)?;
        let mime = self
            .attachment_mime
            .clone()
            .ok_or(MessageError::InvalidAttachment)?;
        let attachment = AttachmentRef {
            hash,
            name,
            size,
            mime,
        };
        attachment
            .validate(crate::attachment::DEFAULT_MAX_ATTACHMENT_SIZE)
            .map_err(MessageError::Attachment)?;
        Ok(Some(attachment))
    }

    pub fn text(
        conversation_id: impl Into<String>,
        sender_id: impl Into<String>,
        seq: u64,
        sent_at: i64,
        body: impl Into<String>,
    ) -> Self {
        Self::new(
            conversation_id,
            sender_id,
            seq,
            sent_at,
            MessageKind::Text,
            Some(body.into()),
        )
    }

    pub fn canonical_bytes(&self) -> Result<Vec<u8>, MessageError> {
        self.validate()?;
        let mut encoder = Encoder::new(MESSAGE_DOMAIN).map_err(MessageError::Canonical)?;
        encoder
            .u16(PROTOCOL_VERSION)
            .map_err(MessageError::Canonical)?;
        encoder
            .bytes(self.message_id.as_uuid().as_bytes())
            .map_err(MessageError::Canonical)?;
        encoder
            .string(&self.conversation_id)
            .map_err(MessageError::Canonical)?;
        encoder
            .string(&self.sender_id)
            .map_err(MessageError::Canonical)?;
        encoder.u64(self.seq).map_err(MessageError::Canonical)?;
        encoder
            .u64(u64::try_from(self.sent_at).map_err(|_| MessageError::InvalidTimestamp)?)
            .map_err(MessageError::Canonical)?;
        encoder
            .u8(self.kind.as_u8())
            .map_err(MessageError::Canonical)?;
        encode_optional(&mut encoder, self.target_id.as_deref())?;
        encode_optional(&mut encoder, self.body.as_deref())?;
        encode_optional(&mut encoder, self.attachment_hash.as_deref())?;
        encode_optional(&mut encoder, self.attachment_name.as_deref())?;
        match self.attachment_size {
            Some(size) => {
                encoder.u8(1).map_err(MessageError::Canonical)?;
                encoder.u64(size).map_err(MessageError::Canonical)?;
            }
            None => encoder.u8(0).map_err(MessageError::Canonical)?,
        }
        encode_optional(&mut encoder, self.attachment_mime.as_deref())?;
        Ok(encoder.finish())
    }

    pub fn signed(&self, identity: &IdentityKeyPair) -> Result<SignedEnvelope, MessageError> {
        let bytes = self.canonical_bytes()?;
        SignedEnvelope::sign(identity, &bytes).map_err(MessageError::Sign)
    }

    pub fn from_canonical(bytes: &[u8]) -> Result<Self, MessageError> {
        let mut decoder = Decoder::new(MESSAGE_DOMAIN, bytes).map_err(MessageError::Canonical)?;
        let version = decoder.u16().map_err(MessageError::Canonical)?;
        if version != PROTOCOL_VERSION {
            return Err(MessageError::UnsupportedVersion(version));
        }
        let message_id = MessageId::from_uuid(Uuid::from_bytes(
            decoder
                .bytes()
                .map_err(MessageError::Canonical)?
                .try_into()
                .map_err(|_| MessageError::Canonical(CanonicalError::Truncated))?,
        ));
        let conversation_id = decoder
            .string()
            .map_err(MessageError::Canonical)?
            .to_owned();
        let sender_id = decoder
            .string()
            .map_err(MessageError::Canonical)?
            .to_owned();
        let seq = decoder.u64().map_err(MessageError::Canonical)?;
        let sent_at = i64::try_from(decoder.u64().map_err(MessageError::Canonical)?)
            .map_err(|_| MessageError::InvalidTimestamp)?;
        let kind = MessageKind::from_u8(decoder.u8().map_err(MessageError::Canonical)?)
            .ok_or(MessageError::UnknownKindNumber)?;
        let target_id = decode_optional(&mut decoder)?;
        let body = decode_optional(&mut decoder)?;
        let attachment_hash = decode_optional(&mut decoder)?;
        let attachment_name = decode_optional(&mut decoder)?;
        let attachment_size = match decoder.u8().map_err(MessageError::Canonical)? {
            0 => None,
            1 => Some(decoder.u64().map_err(MessageError::Canonical)?),
            _ => return Err(MessageError::InvalidOptional),
        };
        let attachment_mime = decode_optional(&mut decoder)?;
        decoder.finish().map_err(MessageError::Canonical)?;

        let record = Self {
            message_id,
            conversation_id,
            sender_id,
            seq,
            sent_at,
            kind,
            target_id,
            body,
            attachment_hash,
            attachment_name,
            attachment_size,
            attachment_mime,
        };
        record.validate()?;
        Ok(record)
    }

    pub fn from_envelope(bytes: &[u8]) -> Result<Self, MessageError> {
        let envelope = SignedEnvelope::decode(bytes).map_err(MessageError::Sign)?;
        Self::from_canonical(envelope.payload())
    }

    pub fn decode_signed(
        bytes: &[u8],
        public_key: &VerifyingKey,
    ) -> Result<(Self, SignedEnvelope), MessageError> {
        let envelope = SignedEnvelope::decode(bytes).map_err(MessageError::Sign)?;
        envelope
            .verify(public_key)
            .map_err(|_| MessageError::InvalidSignature)?;
        let record = Self::from_canonical(envelope.payload())?;
        Ok((record, envelope))
    }

    pub fn validate(&self) -> Result<(), MessageError> {
        NodeId::from_str(&self.sender_id).map_err(MessageError::InvalidSender)?;
        if self.conversation_id.is_empty() || self.conversation_id.len() > MAX_MESSAGE_METADATA {
            return Err(MessageError::InvalidConversation);
        }
        if self.seq == 0 || self.seq > i64::MAX as u64 {
            return Err(MessageError::InvalidSequence);
        }
        if self.sent_at < 0 {
            return Err(MessageError::InvalidTimestamp);
        }
        if let Some(target_id) = &self.target_id
            && MessageId::from_str(target_id).is_err()
        {
            return Err(MessageError::InvalidTarget);
        }
        if let Some(body) = &self.body
            && body.len() > MAX_MESSAGE_BODY
        {
            return Err(MessageError::BodyTooLarge);
        }
        if let Some(hash) = &self.attachment_hash
            && (hash.len() != 64 || !hash.bytes().all(|byte| byte.is_ascii_hexdigit()))
        {
            return Err(MessageError::InvalidAttachmentHash);
        }
        if let Some(name) = &self.attachment_name {
            validate_name(name).map_err(MessageError::Attachment)?;
        }
        if let Some(mime) = &self.attachment_mime {
            validate_mime(mime).map_err(MessageError::Attachment)?;
        }
        match (
            &self.attachment_hash,
            &self.attachment_name,
            self.attachment_size,
            &self.attachment_mime,
        ) {
            (None, None, None, None) => {}
            (Some(_), Some(_), Some(_), Some(_)) => {}
            _ => return Err(MessageError::InvalidAttachment),
        }
        if self.kind == MessageKind::Ack {
            if self.target_id.is_none() {
                return Err(MessageError::InvalidAck);
            }
            match self.body.as_deref() {
                Some("delivered" | "read") => {}
                _ => return Err(MessageError::InvalidAck),
            }
        }
        Ok(())
    }
}

pub fn conversation_id(first: NodeId, second: NodeId) -> String {
    let (lower, higher) = if first <= second {
        (first, second)
    } else {
        (second, first)
    };

    let mut hasher = blake3::Hasher::new();
    hasher.update(CONVERSATION_DOMAIN);
    hasher.update(lower.as_uuid().as_bytes());
    hasher.update(higher.as_uuid().as_bytes());
    let hash = hasher.finalize();
    let mut uuid = [0_u8; 16];
    uuid.copy_from_slice(&hash.as_bytes()[..16]);
    Uuid::from_bytes(uuid).to_string()
}

fn encode_optional(encoder: &mut Encoder, value: Option<&str>) -> Result<(), MessageError> {
    match value {
        Some(value) => {
            encoder.u8(1).map_err(MessageError::Canonical)?;
            encoder.string(value).map_err(MessageError::Canonical)?;
        }
        None => encoder.u8(0).map_err(MessageError::Canonical)?,
    }
    Ok(())
}

fn decode_optional(decoder: &mut Decoder<'_>) -> Result<Option<String>, MessageError> {
    match decoder.u8().map_err(MessageError::Canonical)? {
        0 => Ok(None),
        1 => Ok(Some(
            decoder
                .string()
                .map_err(MessageError::Canonical)?
                .to_owned(),
        )),
        _ => Err(MessageError::InvalidOptional),
    }
}

#[derive(Debug, Error)]
pub enum MessageError {
    #[error(transparent)]
    Canonical(CanonicalError),

    #[error(transparent)]
    Sign(SignError),

    #[error("message signature is invalid")]
    InvalidSignature,

    #[error("message protocol version {0} is not supported")]
    UnsupportedVersion(u16),

    #[error("message kind {0} is not supported")]
    UnknownKind(String),

    #[error("message kind is not recognized")]
    UnknownKindNumber,

    #[error("message sender is not a valid node id")]
    InvalidSender(#[source] IdError),

    #[error("message conversation id is invalid")]
    InvalidConversation,

    #[error("message sequence is invalid")]
    InvalidSequence,

    #[error("message timestamp is invalid")]
    InvalidTimestamp,

    #[error("message target id is invalid")]
    InvalidTarget,

    #[error("message body exceeds {MAX_MESSAGE_BODY} bytes")]
    BodyTooLarge,

    #[error("attachment hash is invalid")]
    InvalidAttachmentHash,

    #[error("attachment reference is incomplete")]
    InvalidAttachment,

    #[error(transparent)]
    Attachment(AttachmentError),

    #[error("ack record is invalid")]
    InvalidAck,

    #[error("optional message field is invalid")]
    InvalidOptional,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn conversation_id_is_order_independent() {
        let first = NodeId::generate();
        let second = NodeId::generate();
        assert_eq!(
            conversation_id(first, second),
            conversation_id(second, first)
        );
        assert_ne!(conversation_id(first, second), first.to_string());
    }

    #[test]
    fn text_record_round_trips_and_verifies() {
        let identity = IdentityKeyPair::from_secret_bytes([71_u8; 32]);
        let sender = NodeId::generate();
        let peer = NodeId::generate();
        let record = MessageRecord::text(
            conversation_id(sender, peer),
            sender.to_string(),
            1,
            1_700_000_000_000,
            "hello",
        );
        let envelope = record.signed(&identity).expect("sign");
        let (decoded, _) =
            MessageRecord::decode_signed(&envelope.encode(), &identity.verifying_key())
                .expect("decode");

        assert_eq!(decoded, record);
    }

    #[test]
    fn tampering_fails_verification() {
        let identity = IdentityKeyPair::from_secret_bytes([72_u8; 32]);
        let sender = NodeId::generate();
        let peer = NodeId::generate();
        let record = MessageRecord::text(
            conversation_id(sender, peer),
            sender.to_string(),
            1,
            1,
            "hello",
        );
        let mut bytes = record.signed(&identity).expect("sign").encode();
        let last = bytes.len() - 1;
        bytes[last] ^= 0xff;

        assert!(matches!(
            MessageRecord::decode_signed(&bytes, &identity.verifying_key()),
            Err(MessageError::Sign(SignError::InvalidSignature))
                | Err(MessageError::InvalidSignature)
                | Err(MessageError::Canonical(_))
        ));
    }

    #[test]
    fn attachment_record_round_trips() {
        let identity = IdentityKeyPair::from_secret_bytes([73_u8; 32]);
        let sender = NodeId::generate();
        let peer = NodeId::generate();
        let attachment =
            AttachmentRef::new(AttachmentHash::of(b"payload"), "photo.png", 7, "image/png")
                .expect("attachment");
        let record = MessageRecord::new(
            conversation_id(sender, peer),
            sender.to_string(),
            1,
            1,
            MessageKind::File,
            None,
        )
        .with_attachment(&attachment)
        .expect("attach");
        let envelope = record.signed(&identity).expect("sign");
        let (decoded, _) =
            MessageRecord::decode_signed(&envelope.encode(), &identity.verifying_key())
                .expect("decode");

        assert_eq!(decoded.attachment_ref().expect("ref"), Some(attachment));
        assert_eq!(decoded.kind, MessageKind::Image);
    }

    #[test]
    fn rejects_incomplete_attachment() {
        let mut record = MessageRecord::new(
            "conversation",
            NodeId::generate().to_string(),
            1,
            1,
            MessageKind::File,
            None,
        );
        record.attachment_hash = Some("a".repeat(64));
        assert!(matches!(
            record.canonical_bytes(),
            Err(MessageError::InvalidAttachment)
        ));
    }

    #[test]
    fn rejects_oversized_body_and_bad_ack() {
        let sender = NodeId::generate();
        let peer = NodeId::generate();
        let mut record = MessageRecord::text(
            conversation_id(sender, peer),
            sender.to_string(),
            1,
            1,
            "x".repeat(MAX_MESSAGE_BODY + 1),
        );
        assert!(matches!(
            record.canonical_bytes(),
            Err(MessageError::BodyTooLarge)
        ));

        record = MessageRecord::new(
            conversation_id(sender, peer),
            sender.to_string(),
            2,
            1,
            MessageKind::Ack,
            Some("delivered".to_owned()),
        );
        assert!(matches!(
            record.canonical_bytes(),
            Err(MessageError::InvalidAck)
        ));
    }
}
