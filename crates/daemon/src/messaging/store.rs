use ed25519_dalek::VerifyingKey;
use ticonnect_core::ack::AckKind;
use ticonnect_core::id::{MessageId, NodeId};
use ticonnect_core::message::{MessageKind, MessageRecord, conversation_id};
use ticonnect_core::sign::SignedEnvelope;

use super::MessagingError;
use crate::storage::Peer;

pub fn text(local: NodeId, peer: NodeId, seq: u64, sent_at: i64, body: String) -> MessageRecord {
    MessageRecord::text(
        conversation_id(local, peer),
        local.to_string(),
        seq,
        sent_at,
        body,
    )
}

pub fn attachment(
    local: NodeId,
    peer: NodeId,
    seq: u64,
    sent_at: i64,
    attachment: &ticonnect_core::attachment::AttachmentRef,
) -> Result<MessageRecord, MessagingError> {
    MessageRecord::new(
        conversation_id(local, peer),
        local.to_string(),
        seq,
        sent_at,
        attachment.kind(),
        None,
    )
    .with_attachment(attachment)
    .map_err(MessagingError::Message)
}

pub fn ack(
    local: NodeId,
    peer: NodeId,
    seq: u64,
    sent_at: i64,
    kind: AckKind,
    target: MessageId,
) -> Result<MessageRecord, MessagingError> {
    Ok(ticonnect_core::ack::record(
        kind,
        conversation_id(local, peer),
        local.to_string(),
        seq,
        sent_at,
        target,
    )?)
}

pub fn verified(
    bytes: &[u8],
    peer: &Peer,
    local: NodeId,
) -> Result<(MessageRecord, Vec<u8>), MessagingError> {
    let public_key = peer
        .public_key
        .ok_or_else(|| MessagingError::MissingPeerKey(peer.id.clone()))?;
    let public_key = VerifyingKey::from_bytes(&public_key)
        .map_err(|_| MessagingError::InvalidPeerKey(peer.id.clone()))?;
    let (record, envelope) = MessageRecord::decode_signed(bytes, &public_key)?;
    if record.sender_id != peer.id {
        return Err(MessagingError::SenderMismatch);
    }
    let peer_id = peer_node(peer)?;
    if record.conversation_id != conversation_id(local, peer_id) {
        return Err(MessagingError::ConversationMismatch);
    }
    Ok((record, envelope.signature().to_vec()))
}

pub fn signed_bytes(record: &MessageRecord, signature: &[u8]) -> Result<Vec<u8>, MessagingError> {
    let payload = record.canonical_bytes()?;
    let signature: [u8; 64] = signature
        .try_into()
        .map_err(|_| MessagingError::InvalidSignature)?;
    Ok(SignedEnvelope::from_parts(&payload, signature)?.encode())
}

pub fn is_ack(record: &MessageRecord) -> bool {
    record.kind == MessageKind::Ack
}

fn peer_node(peer: &Peer) -> Result<NodeId, MessagingError> {
    peer.id
        .parse()
        .map_err(|_| MessagingError::InvalidPeerId(peer.id.clone()))
}
