use std::time::{Duration, Instant};

use ticonnect_core::ack::AckKind;
use ticonnect_core::id::MessageId;

pub const RECEIPT_DELAY: Duration = Duration::from_millis(500);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingAck {
    pub peer_id: String,
    pub target_id: MessageId,
    pub kind: AckKind,
    pub queued_at: Instant,
}

#[derive(Debug, Default)]
pub struct ReceiptCoalescer {
    pending: Vec<PendingAck>,
}

impl ReceiptCoalescer {
    pub fn queue(&mut self, peer_id: impl Into<String>, target_id: MessageId, kind: AckKind) {
        let peer_id = peer_id.into();
        if self
            .pending
            .iter()
            .any(|ack| ack.peer_id == peer_id && ack.target_id == target_id && ack.kind == kind)
        {
            return;
        }
        self.pending.push(PendingAck {
            peer_id,
            target_id,
            kind,
            queued_at: Instant::now(),
        });
    }

    pub fn take_due(&mut self, now: Instant) -> Vec<PendingAck> {
        let mut due = Vec::new();
        self.pending.retain(|ack| {
            if now.saturating_duration_since(ack.queued_at) >= RECEIPT_DELAY {
                due.push(ack.clone());
                false
            } else {
                true
            }
        });
        due
    }

    pub fn is_empty(&self) -> bool {
        self.pending.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn coalesces_duplicates_and_releases_after_delay() {
        let mut coalescer = ReceiptCoalescer::default();
        let target = MessageId::generate();
        coalescer.queue("peer", target, AckKind::Delivered);
        coalescer.queue("peer", target, AckKind::Delivered);
        assert_eq!(coalescer.pending.len(), 1);

        let now = Instant::now();
        assert!(coalescer.take_due(now).is_empty());
        assert_eq!(coalescer.take_due(now + RECEIPT_DELAY).len(), 1);
        assert!(coalescer.is_empty());
    }
}
