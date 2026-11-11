use std::str::FromStr;

use thiserror::Error;

use crate::id::MessageId;
use crate::message::{MessageError, MessageKind, MessageRecord};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AckKind {
    Delivered,
    Read,
}

impl AckKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Delivered => "delivered",
            Self::Read => "read",
        }
    }
}

impl FromStr for AckKind {
    type Err = AckError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "delivered" => Ok(Self::Delivered),
            "read" => Ok(Self::Read),
            other => Err(AckError::UnknownKind(other.to_owned())),
        }
    }
}

pub fn record(
    kind: AckKind,
    conversation_id: impl Into<String>,
    sender_id: impl Into<String>,
    seq: u64,
    sent_at: i64,
    target_id: MessageId,
) -> Result<MessageRecord, AckError> {
    let record = MessageRecord::new(
        conversation_id,
        sender_id,
        seq,
        sent_at,
        MessageKind::Ack,
        Some(kind.as_str().to_owned()),
    );
    let mut record = record;
    record.target_id = Some(target_id.to_string());
    record.validate().map_err(AckError::Message)?;
    Ok(record)
}

pub fn kind_of(record: &MessageRecord) -> Result<AckKind, AckError> {
    if record.kind != MessageKind::Ack {
        return Err(AckError::NotAnAck);
    }
    record
        .body
        .as_deref()
        .ok_or(AckError::NotAnAck)
        .and_then(AckKind::from_str)
}

#[derive(Debug, Error)]
pub enum AckError {
    #[error("ack kind {0} is not supported")]
    UnknownKind(String),

    #[error("record is not a valid ack")]
    NotAnAck,

    #[error(transparent)]
    Message(MessageError),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::id::NodeId;
    use crate::message::conversation_id;

    #[test]
    fn ack_records_carry_target_and_kind() {
        let sender = NodeId::generate();
        let peer = NodeId::generate();
        let target = MessageId::generate();
        let record = record(
            AckKind::Read,
            conversation_id(sender, peer),
            sender.to_string(),
            3,
            42,
            target,
        )
        .expect("ack");

        assert_eq!(kind_of(&record).expect("kind"), AckKind::Read);
        assert_eq!(record.target_id, Some(target.to_string()));
        assert_eq!(record.body.as_deref(), Some("read"));
    }
}
