use ticonnect_core::id::NodeId;
use ticonnect_core::wire::{Frame, SyncFrame, sync_frame_from_wire, sync_frame_to_wire};
use ticonnect_net::transport::Session;
use ticonnect_net::transport::session::{read_frame, write_stream_frame};

use super::{MessagingError, MessagingService, outbox, store};

const SYNC_BATCH: u32 = 128;

pub async fn run_initiator(
    service: MessagingService,
    session: Session,
) -> Result<(), MessagingError> {
    let peer_node = session.peer().node_id;
    let peer_id = peer_node.to_string();
    let conversation_id = ensure_conversation(&service, peer_node).await?;
    let local = service.database().peer_state(&peer_id).await?;

    let (mut send, mut recv) = session.open_bi().await?;
    write_stream_frame(
        &mut send,
        &sync_frame_to_wire(&SyncFrame::Hello {
            last_sent_seq: local.last_sent_seq,
            last_recv_seq: local.last_recv_seq,
        })?,
    )
    .await?;

    let remote = expect_hello(read_sync(&mut recv).await?)?;
    let count = request_phase(
        &service,
        &peer_id,
        remote.last_sent_seq,
        &mut send,
        &mut recv,
    )
    .await?;
    serve_phase(&service, &peer_id, &conversation_id, &mut send, &mut recv).await?;
    let _ = send.finish();

    service.mark_sync_ready(&peer_id);
    service.emit_sync_completed(&peer_id, count);
    outbox::flush_peer(&service, &peer_id).await;
    Ok(())
}

pub async fn run_responder(
    service: MessagingService,
    peer_info: ticonnect_net::transport::PeerInfo,
    hello: SyncFrame,
    send: &mut quinn::SendStream,
    recv: &mut quinn::RecvStream,
) -> Result<(), MessagingError> {
    let peer_node = peer_info.node_id;
    let peer_id = peer_node.to_string();
    let conversation_id = ensure_conversation(&service, peer_node).await?;
    let remote = expect_hello(hello)?;
    let local = service.database().peer_state(&peer_id).await?;
    write_stream_frame(
        send,
        &sync_frame_to_wire(&SyncFrame::Hello {
            last_sent_seq: local.last_sent_seq,
            last_recv_seq: local.last_recv_seq,
        })?,
    )
    .await?;

    serve_phase(&service, &peer_id, &conversation_id, send, recv).await?;
    let count = request_phase(&service, &peer_id, remote.last_sent_seq, send, recv).await?;
    let _ = send.finish();

    service.mark_sync_ready(&peer_id);
    service.emit_sync_completed(&peer_id, count);
    outbox::flush_peer(&service, &peer_id).await;
    Ok(())
}

async fn ensure_conversation(
    service: &MessagingService,
    peer_node: NodeId,
) -> Result<String, MessagingError> {
    let conversation = ticonnect_core::message::conversation_id(service.local_node_id(), peer_node);
    service
        .database()
        .ensure_conversation(&conversation, &peer_node.to_string(), super::now_unix_ms())
        .await?;
    Ok(conversation)
}

async fn request_phase(
    service: &MessagingService,
    peer_id: &str,
    remote_last_sent: u64,
    send: &mut quinn::SendStream,
    recv: &mut quinn::RecvStream,
) -> Result<u32, MessagingError> {
    let local = service.database().peer_state(peer_id).await?;
    let mut from = local.last_recv_seq.saturating_add(1);
    let mut inserted = 0_u32;

    while from <= remote_last_sent {
        let to = remote_last_sent.min(from.saturating_add(u64::from(SYNC_BATCH) - 1));
        write_stream_frame(
            send,
            &sync_frame_to_wire(&SyncFrame::Request {
                from_seq: from,
                to_seq: to,
            })?,
        )
        .await?;

        loop {
            match read_sync(recv).await? {
                SyncFrame::Record { envelope } => {
                    if service.ingest_payload(peer_id, &envelope).await? {
                        inserted = inserted.saturating_add(1);
                    }
                }
                SyncFrame::Done { .. } => break,
                SyncFrame::Error { code } => {
                    return Err(MessagingError::SyncRejected(code));
                }
                _ => return Err(MessagingError::UnexpectedSyncFrame),
            }
        }
        service.database().advance_recv_seq(peer_id, to).await?;
        from = to.saturating_add(1);
    }

    write_stream_frame(send, &sync_frame_to_wire(&SyncFrame::Complete)?).await?;
    Ok(inserted)
}

async fn serve_phase(
    service: &MessagingService,
    peer_id: &str,
    conversation_id: &str,
    send: &mut quinn::SendStream,
    recv: &mut quinn::RecvStream,
) -> Result<(), MessagingError> {
    loop {
        match read_sync(recv).await? {
            SyncFrame::Request { from_seq, to_seq } => {
                if to_seq < from_seq {
                    write_stream_frame(send, &sync_frame_to_wire(&SyncFrame::Done { count: 0 })?)
                        .await?;
                    continue;
                }
                let records = service
                    .database()
                    .records_between(conversation_id, from_seq, to_seq, SYNC_BATCH)
                    .await?;
                let mut count = 0_u32;
                for stored in records {
                    let envelope = store::signed_bytes(&stored.record, &stored.signature)?;
                    write_stream_frame(send, &sync_frame_to_wire(&SyncFrame::Record { envelope })?)
                        .await?;
                    count = count.saturating_add(1);
                }
                write_stream_frame(send, &sync_frame_to_wire(&SyncFrame::Done { count })?).await?;
                tracing::trace!(peer_id, count, "served sync batch");
            }
            SyncFrame::Complete => return Ok(()),
            SyncFrame::Error { code } => return Err(MessagingError::SyncRejected(code)),
            _ => return Err(MessagingError::UnexpectedSyncFrame),
        }
    }
}

async fn read_sync(recv: &mut quinn::RecvStream) -> Result<SyncFrame, MessagingError> {
    let frame: Frame = read_frame(recv).await?;
    Ok(sync_frame_from_wire(&frame)?)
}

fn expect_hello(frame: SyncFrame) -> Result<HelloState, MessagingError> {
    match frame {
        SyncFrame::Hello {
            last_sent_seq,
            last_recv_seq: _,
        } => Ok(HelloState { last_sent_seq }),
        _ => Err(MessagingError::UnexpectedSyncFrame),
    }
}

struct HelloState {
    last_sent_seq: u64,
}
