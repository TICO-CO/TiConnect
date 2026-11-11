use std::time::Duration;

use ticonnect_core::id::NodeId;
use ticonnect_core::message::MessageKind;
use ticonnect_core::wire::{Frame, FrameKind};

use super::{MessagingService, now_unix_ms, store};
use crate::storage::MessageState;

const BACKOFF_BASE: Duration = Duration::from_secs(1);

pub async fn flush_peer(service: &MessagingService, peer_id: &str) {
    if !service.is_sync_ready(peer_id) {
        return;
    }
    let Ok(peer_node) = peer_id.parse::<NodeId>() else {
        return;
    };
    let Some(session) = service.transport().session_by_node_id(peer_node) else {
        return;
    };
    let entries = match service.database().due_outbox(peer_id, now_unix_ms()).await {
        Ok(entries) => entries,
        Err(error) => {
            tracing::warn!(%error, "failed to read outbox");
            return;
        }
    };

    for entry in entries {
        let stored = match service.database().get_message(&entry.message_id).await {
            Ok(Some(stored)) => stored,
            Ok(None) => {
                let _ = service.database().remove_outbox(&entry.message_id).await;
                continue;
            }
            Err(error) => {
                tracing::warn!(%error, "failed to read outbox message");
                continue;
            }
        };
        let bytes = match store::signed_bytes(&stored.record, &stored.signature) {
            Ok(bytes) => bytes,
            Err(error) => {
                tracing::warn!(%error, "failed to encode outbox message");
                let _ = service.database().remove_outbox(&entry.message_id).await;
                continue;
            }
        };
        let kind = if stored.record.kind == MessageKind::Ack {
            FrameKind::Ack
        } else {
            FrameKind::Message
        };
        let frame = match Frame::new(kind, bytes) {
            Ok(frame) => frame,
            Err(error) => {
                tracing::warn!(%error, "outbox message exceeds frame limit");
                let _ = service
                    .database()
                    .set_message_state(&entry.message_id, MessageState::Failed)
                    .await;
                let _ = service.database().remove_outbox(&entry.message_id).await;
                continue;
            }
        };

        let mut send = match session.open_uni().await {
            Ok(send) => send,
            Err(error) => {
                tracing::debug!(%error, "outbox stream open failed");
                defer(service, &entry.message_id, entry.attempts).await;
                return;
            }
        };
        if let Err(error) = ticonnect_net::transport::session::write_frame(&mut send, &frame).await
        {
            tracing::debug!(%error, "outbox stream write failed");
            defer(service, &entry.message_id, entry.attempts).await;
            return;
        }

        if service
            .database()
            .set_message_state(&entry.message_id, MessageState::Sent)
            .await
            .unwrap_or(false)
        {
            service.emit_state_changed(&entry.message_id, MessageState::Sent);
        }
        let _ = service.database().remove_outbox(&entry.message_id).await;
    }
}

async fn defer(service: &MessagingService, message_id: &str, attempts: u32) {
    let factor = 1_u64 << attempts.min(20);
    let delay = BACKOFF_BASE
        .checked_mul(u32::try_from(factor).unwrap_or(u32::MAX))
        .map_or(Duration::from_secs(60), |delay| delay)
        .min(Duration::from_secs(3600));
    let next_try =
        now_unix_ms().saturating_add(i64::try_from(delay.as_millis()).unwrap_or(i64::MAX));
    let _ = service.database().defer_outbox(message_id, next_try).await;
}
