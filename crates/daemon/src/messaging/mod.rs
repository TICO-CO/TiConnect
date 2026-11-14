pub mod outbox;
pub mod policy;
pub mod receipts;
pub mod store;
pub mod sync;

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex, PoisonError, RwLock};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use thiserror::Error;
use ticonnect_core::ack::AckKind;
use ticonnect_core::attachment::AttachmentRef;
use ticonnect_core::id::{MessageId, NodeId};
use ticonnect_core::identity::IdentityKeyPair;
use ticonnect_core::message::{MessageKind, MessageRecord};
use ticonnect_core::wire::{Frame, FrameKind};
use ticonnect_net::transport::{PeerInfo, Session, Transport, TransportEvent};
use ticonnect_settings::Settings;
use tokio::sync::mpsc::UnboundedSender;
use tokio::task::JoinHandle;

use crate::attachments::transfer::InboundTransfer;
use crate::calls::{InboundAudio, InboundCallSignal};
use crate::remote::InboundRemoteFrame;
use crate::storage::{
    DatabaseHandle, InsertOutcome, MessageState, Peer, PeerPolicy, StorageError, StoredMessage,
};
use crate::trust::{TrustError, TrustStore};
use receipts::{PendingAck, ReceiptCoalescer};

use ticonnect_net::transport::TransportError;

const RECEIPT_TICK: Duration = Duration::from_millis(100);
const OUTBOX_TICK: Duration = Duration::from_secs(1);

#[derive(Debug, Clone)]
#[allow(clippy::large_enum_variant)]
pub enum MessagingEvent {
    MessageReceived {
        peer_id: String,
        message: StoredMessage,
        notify: bool,
    },
    MessageStateChanged {
        message_id: String,
        state: MessageState,
    },
    SyncCompleted {
        peer_id: String,
        count: u32,
    },
    PolicyChanged {
        peer_id: String,
        policy: PeerPolicy,
    },
}

#[derive(Clone)]
pub struct MessagingService {
    inner: Arc<Inner>,
}

struct Inner {
    identity: IdentityKeyPair,
    local: NodeId,
    database: DatabaseHandle,
    trust: Arc<TrustStore>,
    transport: Arc<Transport>,
    settings: RwLock<Settings>,
    events: tokio::sync::broadcast::Sender<MessagingEvent>,
    receipts: Mutex<ReceiptCoalescer>,
    sync_ready: Mutex<HashSet<String>>,
    session_tasks: Mutex<HashMap<String, JoinHandle<()>>>,
    transfer_tx: Mutex<Option<UnboundedSender<InboundTransfer>>>,
    call_tx: Mutex<Option<UnboundedSender<InboundCallSignal>>>,
    audio_tx: Mutex<Option<UnboundedSender<InboundAudio>>>,
    remote_tx: Mutex<Option<UnboundedSender<InboundRemoteFrame>>>,
}

impl MessagingService {
    pub fn new(
        identity: IdentityKeyPair,
        local: NodeId,
        database: DatabaseHandle,
        trust: Arc<TrustStore>,
        transport: Arc<Transport>,
        settings: Settings,
    ) -> Self {
        let (events, _) = tokio::sync::broadcast::channel(256);
        Self {
            inner: Arc::new(Inner {
                identity,
                local,
                database,
                trust,
                transport,
                settings: RwLock::new(settings),
                events,
                receipts: Mutex::new(ReceiptCoalescer::default()),
                sync_ready: Mutex::new(HashSet::new()),
                session_tasks: Mutex::new(HashMap::new()),
                transfer_tx: Mutex::new(None),
                call_tx: Mutex::new(None),
                audio_tx: Mutex::new(None),
                remote_tx: Mutex::new(None),
            }),
        }
    }

    pub fn subscribe(&self) -> tokio::sync::broadcast::Receiver<MessagingEvent> {
        self.inner.events.subscribe()
    }

    pub fn set_transfer_sender(&self, sender: UnboundedSender<InboundTransfer>) {
        *self
            .inner
            .transfer_tx
            .lock()
            .unwrap_or_else(PoisonError::into_inner) = Some(sender);
    }

    pub fn set_call_channels(
        &self,
        call_sender: UnboundedSender<InboundCallSignal>,
        audio_sender: UnboundedSender<InboundAudio>,
    ) {
        *self
            .inner
            .call_tx
            .lock()
            .unwrap_or_else(PoisonError::into_inner) = Some(call_sender);
        *self
            .inner
            .audio_tx
            .lock()
            .unwrap_or_else(PoisonError::into_inner) = Some(audio_sender);
    }

    pub fn set_remote_sender(&self, sender: UnboundedSender<InboundRemoteFrame>) {
        *self
            .inner
            .remote_tx
            .lock()
            .unwrap_or_else(PoisonError::into_inner) = Some(sender);
    }

    fn remote_sender(&self) -> Option<UnboundedSender<InboundRemoteFrame>> {
        self.inner
            .remote_tx
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    fn call_sender(&self) -> Option<UnboundedSender<InboundCallSignal>> {
        self.inner
            .call_tx
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    fn audio_sender(&self) -> Option<UnboundedSender<InboundAudio>> {
        self.inner
            .audio_tx
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    fn transfer_sender(&self) -> Option<UnboundedSender<InboundTransfer>> {
        self.inner
            .transfer_tx
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    pub fn local_node(&self) -> NodeId {
        self.inner.local
    }

    pub fn apply_settings(&self, settings: Settings) {
        *self
            .inner
            .settings
            .write()
            .unwrap_or_else(PoisonError::into_inner) = settings;
    }

    pub fn settings(&self) -> Settings {
        self.inner
            .settings
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    pub async fn peer(&self, peer_id: &str) -> Result<Peer, MessagingError> {
        self.inner
            .trust
            .peer(peer_id)
            .await?
            .ok_or_else(|| MessagingError::UnknownPeer(peer_id.to_owned()))
    }

    pub async fn conversation_id(&self, peer_id: &str) -> Result<String, MessagingError> {
        let peer = self.peer(peer_id).await?;
        let peer_node: NodeId = peer
            .id
            .parse()
            .map_err(|_| MessagingError::InvalidPeerId(peer.id.clone()))?;
        Ok(ticonnect_core::message::conversation_id(
            self.inner.local,
            peer_node,
        ))
    }

    pub async fn send_message(
        &self,
        peer_id: &str,
        body: &str,
    ) -> Result<MessageId, MessagingError> {
        if body.is_empty() {
            return Err(MessagingError::EmptyBody);
        }
        if body.len() > ticonnect_core::message::MAX_MESSAGE_BODY {
            return Err(MessagingError::BodyTooLarge);
        }

        let peer = self.peer(peer_id).await?;
        let peer_node: NodeId = peer
            .id
            .parse()
            .map_err(|_| MessagingError::InvalidPeerId(peer.id.clone()))?;
        let now = now_unix_ms();
        self.inner
            .database
            .ensure_conversation(
                &ticonnect_core::message::conversation_id(self.inner.local, peer_node),
                peer_id,
                now,
            )
            .await?;
        let seq = self.inner.database.allocate_seq(peer_id).await?;
        let record = store::text(self.inner.local, peer_node, seq, now, body.to_owned());
        let envelope = record.signed(&self.inner.identity)?;
        let state = MessageState::Queued;
        self.inner
            .database
            .insert_message(
                record.clone(),
                now,
                envelope.signature().to_vec(),
                Some(state),
            )
            .await?;

        let ttl_days = u64::from(self.settings().outbox_ttl_days());
        let expires_at =
            now.saturating_add(i64::try_from(ttl_days * 86_400_000).unwrap_or(i64::MAX));
        self.inner
            .database
            .enqueue_outbox(&record.message_id.to_string(), now, expires_at)
            .await?;
        self.emit(MessagingEvent::MessageStateChanged {
            message_id: record.message_id.to_string(),
            state,
        });
        outbox::flush_peer(self, peer_id).await;
        Ok(record.message_id)
    }

    pub async fn send_attachment(
        &self,
        peer_id: &str,
        attachment: &AttachmentRef,
    ) -> Result<MessageId, MessagingError> {
        attachment
            .validate(ticonnect_core::attachment::DEFAULT_MAX_ATTACHMENT_SIZE)
            .map_err(|error| MessagingError::InvalidAttachment(error.to_string()))?;
        let peer = self.peer(peer_id).await?;
        let peer_node: NodeId = peer
            .id
            .parse()
            .map_err(|_| MessagingError::InvalidPeerId(peer.id.clone()))?;
        let now = now_unix_ms();
        self.inner
            .database
            .ensure_conversation(
                &ticonnect_core::message::conversation_id(self.inner.local, peer_node),
                peer_id,
                now,
            )
            .await?;
        let seq = self.inner.database.allocate_seq(peer_id).await?;
        let record = store::attachment(self.inner.local, peer_node, seq, now, attachment)?;
        let envelope = record.signed(&self.inner.identity)?;
        let state = MessageState::Queued;
        self.inner
            .database
            .insert_message(
                record.clone(),
                now,
                envelope.signature().to_vec(),
                Some(state),
            )
            .await?;
        let ttl_days = u64::from(self.settings().outbox_ttl_days());
        let expires_at =
            now.saturating_add(i64::try_from(ttl_days * 86_400_000).unwrap_or(i64::MAX));
        self.inner
            .database
            .enqueue_outbox(&record.message_id.to_string(), now, expires_at)
            .await?;
        self.emit(MessagingEvent::MessageStateChanged {
            message_id: record.message_id.to_string(),
            state,
        });
        outbox::flush_peer(self, peer_id).await;
        Ok(record.message_id)
    }

    pub async fn list_messages(
        &self,
        conversation_id: &str,
        limit: u32,
        offset: u32,
    ) -> Result<Vec<StoredMessage>, MessagingError> {
        let limit = limit.clamp(1, 500);
        Ok(self
            .inner
            .database
            .list_history(conversation_id, limit, offset)
            .await?)
    }

    pub async fn count_messages(&self, conversation_id: &str) -> Result<u32, MessagingError> {
        Ok(self.inner.database.count_history(conversation_id).await?)
    }

    pub async fn mark_read(
        &self,
        conversation_id: &str,
        message_ids: &[String],
    ) -> Result<(), MessagingError> {
        let peer_id = self.conversation_peer(conversation_id).await?;
        let peer = self.peer(&peer_id).await?;
        if !policy::read_receipt(peer.policy, self.settings().read_receipts_enabled()) {
            return Ok(());
        }

        for message_id in message_ids {
            let target: MessageId = message_id
                .parse()
                .map_err(|_| MessagingError::InvalidMessageId(message_id.clone()))?;
            let Some(stored) = self.inner.database.get_message(message_id).await? else {
                continue;
            };
            if stored.record.conversation_id != conversation_id
                || stored.record.sender_id == self.inner.local.to_string()
            {
                continue;
            }
            self.queue_receipt(&peer_id, target, AckKind::Read);
        }
        Ok(())
    }

    pub async fn message_state(&self, message_id: &str) -> Result<MessageState, MessagingError> {
        self.inner
            .database
            .message_state(message_id)
            .await?
            .ok_or_else(|| MessagingError::UnknownMessage(message_id.to_owned()))
    }

    pub async fn set_peer_policy(
        &self,
        peer_id: &str,
        policy: PeerPolicy,
    ) -> Result<(), MessagingError> {
        if !self
            .inner
            .database
            .update_peer_policy(peer_id, policy)
            .await?
        {
            return Err(MessagingError::UnknownPeer(peer_id.to_owned()));
        }
        self.emit(MessagingEvent::PolicyChanged {
            peer_id: peer_id.to_owned(),
            policy,
        });
        Ok(())
    }

    pub async fn set_dnd(&self, enabled: bool) -> Result<(), MessagingError> {
        let mut settings = self.settings();
        settings.set_dnd_enabled(enabled);
        settings.save()?;
        self.apply_settings(settings);
        Ok(())
    }

    pub fn dnd_enabled(&self) -> bool {
        self.settings().dnd_enabled()
    }

    pub async fn on_peer_connected(&self, peer: PeerInfo) {
        let fingerprint = peer.fingerprint.to_hex();
        if let Ok(Some(stored)) = self.inner.trust.peer_by_fingerprint(&fingerprint).await
            && stored.public_key.is_none()
        {
            let _ = self
                .inner
                .database
                .update_peer_public_key(&stored.id, peer.public_key)
                .await;
        }

        let Some(session) = self
            .inner
            .transport
            .session_by_fingerprint(&peer.fingerprint)
        else {
            return;
        };

        {
            let mut tasks = self
                .inner
                .session_tasks
                .lock()
                .unwrap_or_else(PoisonError::into_inner);
            if let Some(previous) = tasks.remove(&fingerprint) {
                previous.abort();
            }
            let service = self.clone();
            let loop_session = session.clone();
            let task = tokio::spawn(async move {
                service.session_loop(loop_session).await;
            });
            tasks.insert(fingerprint, task);
        }

        let service = self.clone();
        let datagram_session = session.clone();
        tokio::spawn(async move {
            loop {
                match datagram_session.read_datagram().await {
                    Ok(payload) => {
                        if let Some(sender) = service.audio_sender() {
                            let _ = sender.send(InboundAudio {
                                peer: datagram_session.peer().clone(),
                                payload: payload.to_vec(),
                            });
                        }
                    }
                    Err(error) => {
                        tracing::debug!(%error, "session datagram reader stopped");
                        break;
                    }
                }
            }
        });

        let service = self.clone();
        tokio::spawn(async move {
            if let Err(error) = sync::run_initiator(service.clone(), session).await {
                tracing::debug!(%error, "gap sync failed");
            }
        });
    }

    pub fn on_peer_disconnected(&self, peer: &PeerInfo) {
        let fingerprint = peer.fingerprint.to_hex();
        self.clear_sync_ready(&peer.node_id.to_string());
        if let Some(task) = self
            .inner
            .session_tasks
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .remove(&fingerprint)
        {
            task.abort();
        }
    }

    async fn session_loop(&self, session: Session) {
        let peer = session.peer().clone();
        loop {
            tokio::select! {
                result = session.accept_uni() => {
                    match result {
                        Ok(mut recv) => {
                            let service = self.clone();
                            let peer = peer.clone();
                            tokio::spawn(async move {
                                match ticonnect_net::transport::session::read_frame(&mut recv).await {
                                    Ok(frame) => {
                                        if let Err(error) = service.handle_frame(&peer, frame).await {
                                            tracing::debug!(%error, "inbound message frame rejected");
                                        }
                                    }
                                    Err(error) => tracing::debug!(%error, "inbound stream read failed"),
                                }
                            });
                        }
                        Err(error) => {
                            tracing::debug!(%error, "accept uni stream failed");
                            break;
                        }
                    }
                }
                result = session.accept_bi() => {
                    match result {
                        Ok((mut send, mut recv)) => {
                            let service = self.clone();
                            let peer = peer.clone();
                            tokio::spawn(async move {
                                let first = match ticonnect_net::transport::session::read_frame(&mut recv).await {
                                    Ok(frame) => frame,
                                    Err(error) => {
                                        tracing::debug!(%error, "sync stream read failed");
                                        return;
                                    }
                                };
                                if first.kind() == FrameKind::Transfer {
                                    let frame = match ticonnect_net::transfer::TransferFrame::from_wire(&first) {
                                        Ok(frame) => frame,
                                        Err(error) => {
                                            tracing::debug!(%error, "transfer frame rejected");
                                            return;
                                        }
                                    };
                                    let Some(sender) = service.transfer_sender() else {
                                        return;
                                    };
                                    let _ = sender.send(InboundTransfer {
                                        peer,
                                        send,
                                        recv,
                                        frame,
                                    });
                                    return;
                                }
                                if first.kind() != FrameKind::Sync {
                                    return;
                                }
                                let hello = match ticonnect_core::wire::sync_frame_from_wire(&first) {
                                    Ok(frame) => frame,
                                    Err(error) => {
                                        tracing::debug!(%error, "sync hello rejected");
                                        return;
                                    }
                                };
                                if let Err(error) = sync::run_responder(service, peer, hello, &mut send, &mut recv).await {
                                    tracing::debug!(%error, "sync responder failed");
                                }
                            });
                        }
                        Err(error) => {
                            tracing::debug!(%error, "accept bi stream failed");
                            break;
                        }
                    }
                }
                _ = session.connection().closed() => break,
            }
        }
    }

    async fn handle_frame(&self, peer: &PeerInfo, frame: Frame) -> Result<(), MessagingError> {
        match frame.kind() {
            FrameKind::Message | FrameKind::Ack => {}
            FrameKind::Call => {
                let frame = ticonnect_core::call::CallSignalFrame::from_wire(&frame)
                    .map_err(|error| MessagingError::InvalidAttachment(error.to_string()))?;
                if let Some(sender) = self.call_sender() {
                    let _ = sender.send(InboundCallSignal {
                        peer: peer.clone(),
                        frame,
                    });
                }
                return Ok(());
            }
            FrameKind::Remote | FrameKind::Input => {
                if let Some(sender) = self.remote_sender() {
                    let _ = sender.send(InboundRemoteFrame {
                        peer: peer.clone(),
                        frame,
                    });
                }
                return Ok(());
            }
            other => return Err(MessagingError::UnexpectedFrame(other.as_u8())),
        }

        let stored = self
            .inner
            .trust
            .peer_by_fingerprint(&peer.fingerprint.to_hex())
            .await?
            .ok_or_else(|| MessagingError::UnknownPeer(peer.node_id.to_string()))?;
        if stored.policy == PeerPolicy::Ignore {
            return Ok(());
        }
        if stored.public_key.is_none() {
            self.inner
                .database
                .update_peer_public_key(&stored.id, peer.public_key)
                .await?;
        }
        self.ingest_payload(&stored.id, frame.payload()).await?;
        Ok(())
    }

    pub(crate) async fn ingest_payload(
        &self,
        peer_id: &str,
        bytes: &[u8],
    ) -> Result<bool, MessagingError> {
        let peer = self.peer(peer_id).await?;
        if peer.policy == PeerPolicy::Ignore {
            return Ok(false);
        }
        let (record, signature) = store::verified(bytes, &peer, self.inner.local)?;
        match record.kind {
            MessageKind::Ack => {
                self.ingest_ack(&peer, record, signature).await?;
                Ok(true)
            }
            MessageKind::Text | MessageKind::Image | MessageKind::Audio | MessageKind::File => {
                if record.kind != MessageKind::Text {
                    record
                        .attachment_ref()
                        .map_err(|error| MessagingError::InvalidAttachment(error.to_string()))?
                        .ok_or_else(|| {
                            MessagingError::InvalidAttachment(
                                "attachment message has no reference".to_owned(),
                            )
                        })?;
                }
                self.ingest_text(&peer, record, signature).await?;
                Ok(true)
            }
        }
    }

    async fn ingest_text(
        &self,
        peer: &Peer,
        record: MessageRecord,
        signature: Vec<u8>,
    ) -> Result<(), MessagingError> {
        let decision = policy::inbound(
            peer.policy,
            self.settings().dnd_enabled(),
            self.settings().read_receipts_enabled(),
        );
        let policy::InboundDecision::Store {
            notify,
            delivered_receipt,
        } = decision
        else {
            return Ok(());
        };

        let received_at = now_unix_ms();
        let outcome = self
            .inner
            .database
            .insert_inbound(record.clone(), received_at, signature, &peer.id)
            .await?;
        if outcome == InsertOutcome::Inserted {
            let stored = self
                .inner
                .database
                .get_message(&record.message_id.to_string())
                .await?
                .ok_or_else(|| MessagingError::UnknownMessage(record.message_id.to_string()))?;
            self.emit(MessagingEvent::MessageReceived {
                peer_id: peer.id.clone(),
                message: stored,
                notify,
            });
        }
        if delivered_receipt {
            self.queue_receipt(&peer.id, record.message_id, AckKind::Delivered);
        }
        Ok(())
    }

    async fn ingest_ack(
        &self,
        peer: &Peer,
        record: MessageRecord,
        signature: Vec<u8>,
    ) -> Result<(), MessagingError> {
        let kind = ticonnect_core::ack::kind_of(&record)?;
        let target = record.target_id.clone().ok_or(MessagingError::InvalidAck)?;
        let target_id: MessageId = target
            .parse()
            .map_err(|_| MessagingError::InvalidMessageId(target.clone()))?;

        let received_at = now_unix_ms();
        self.inner
            .database
            .insert_inbound(record, received_at, signature, &peer.id)
            .await?;

        let Some(stored) = self.inner.database.get_message(&target).await? else {
            return Err(MessagingError::UnknownMessage(target.to_owned()));
        };
        if stored.record.sender_id != self.inner.local.to_string() {
            return Err(MessagingError::InvalidAck);
        }
        let state = match kind {
            AckKind::Delivered => MessageState::Delivered,
            AckKind::Read => MessageState::Read,
        };
        if self
            .inner
            .database
            .set_message_state(&target, state)
            .await?
        {
            self.emit(MessagingEvent::MessageStateChanged {
                message_id: target_id.to_string(),
                state,
            });
        }
        Ok(())
    }

    fn queue_receipt(&self, peer_id: &str, target: MessageId, kind: AckKind) {
        self.inner
            .receipts
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .queue(peer_id, target, kind);
    }

    pub async fn flush_receipts(&self) {
        let due = self
            .inner
            .receipts
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .take_due(Instant::now());
        for ack in due {
            if let Err(error) = self.create_ack(&ack).await {
                tracing::debug!(%error, "failed to queue receipt");
            }
        }
    }

    async fn create_ack(&self, ack: &PendingAck) -> Result<(), MessagingError> {
        let peer = self.peer(&ack.peer_id).await?;
        let peer_node: NodeId = peer
            .id
            .parse()
            .map_err(|_| MessagingError::InvalidPeerId(peer.id.clone()))?;
        let conversation = ticonnect_core::message::conversation_id(self.inner.local, peer_node);
        self.inner
            .database
            .ensure_conversation(&conversation, &peer.id, now_unix_ms())
            .await?;
        let seq = self.inner.database.allocate_seq(&peer.id).await?;
        let record = store::ack(
            self.inner.local,
            peer_node,
            seq,
            now_unix_ms(),
            ack.kind,
            ack.target_id,
        )?;
        let envelope = record.signed(&self.inner.identity)?;
        self.inner
            .database
            .insert_message(
                record.clone(),
                now_unix_ms(),
                envelope.signature().to_vec(),
                Some(MessageState::Queued),
            )
            .await?;
        let ttl_days = u64::from(self.settings().outbox_ttl_days());
        let expires_at =
            now_unix_ms().saturating_add(i64::try_from(ttl_days * 86_400_000).unwrap_or(i64::MAX));
        self.inner
            .database
            .enqueue_outbox(&record.message_id.to_string(), now_unix_ms(), expires_at)
            .await?;
        outbox::flush_peer(self, &ack.peer_id).await;
        Ok(())
    }

    pub async fn expire_outbox(&self) {
        match self.inner.database.expire_outbox(now_unix_ms()).await {
            Ok(expired) => {
                for message_id in expired {
                    self.emit(MessagingEvent::MessageStateChanged {
                        message_id,
                        state: MessageState::Expired,
                    });
                }
            }
            Err(error) => tracing::warn!(%error, "failed to expire outbox"),
        }
    }

    pub async fn flush_due(&self) {
        let peers = match self.inner.database.list_peers().await {
            Ok(peers) => peers,
            Err(error) => {
                tracing::warn!(%error, "failed to list peers for outbox");
                return;
            }
        };
        for peer in peers {
            outbox::flush_peer(self, &peer.id).await;
        }
    }

    fn emit(&self, event: MessagingEvent) {
        let _ = self.inner.events.send(event);
    }

    pub(crate) fn local_node_id(&self) -> NodeId {
        self.inner.local
    }

    pub(crate) fn database(&self) -> &DatabaseHandle {
        &self.inner.database
    }

    pub fn sync_ready(&self, peer_id: &str) -> bool {
        self.is_sync_ready(peer_id)
    }

    pub(crate) fn is_sync_ready(&self, peer_id: &str) -> bool {
        self.inner
            .sync_ready
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .contains(peer_id)
    }

    pub(crate) fn mark_sync_ready(&self, peer_id: &str) {
        self.inner
            .sync_ready
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .insert(peer_id.to_owned());
    }

    pub(crate) fn clear_sync_ready(&self, peer_id: &str) {
        self.inner
            .sync_ready
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .remove(peer_id);
    }

    pub(crate) fn transport(&self) -> &Transport {
        &self.inner.transport
    }

    pub(crate) fn emit_sync_completed(&self, peer_id: &str, count: u32) {
        self.emit(MessagingEvent::SyncCompleted {
            peer_id: peer_id.to_owned(),
            count,
        });
    }

    pub(crate) fn emit_state_changed(&self, message_id: &str, state: MessageState) {
        self.emit(MessagingEvent::MessageStateChanged {
            message_id: message_id.to_owned(),
            state,
        });
    }

    async fn conversation_peer(&self, conversation_id: &str) -> Result<String, MessagingError> {
        let peers = self.inner.database.list_peers().await?;
        for peer in peers {
            let peer_node: NodeId = peer
                .id
                .parse()
                .map_err(|_| MessagingError::InvalidPeerId(peer.id.clone()))?;
            if ticonnect_core::message::conversation_id(self.inner.local, peer_node)
                == conversation_id
            {
                return Ok(peer.id);
            }
        }
        Err(MessagingError::UnknownConversation(
            conversation_id.to_owned(),
        ))
    }
}

pub fn spawn(connection: zbus::Connection, service: MessagingService) -> JoinHandle<()> {
    let mut transport_events = service.inner.transport.subscribe();
    let mut events = service.subscribe();
    tokio::spawn(async move {
        let mut receipt_tick = tokio::time::interval(RECEIPT_TICK);
        receipt_tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        let mut outbox_tick = tokio::time::interval(OUTBOX_TICK);
        outbox_tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);

        loop {
            tokio::select! {
                event = transport_events.recv() => {
                    match event {
                        Ok(TransportEvent::PeerConnected(peer)) => service.on_peer_connected(peer).await,
                        Ok(TransportEvent::PeerDisconnected(peer)) => service.on_peer_disconnected(&peer),
                        Ok(TransportEvent::PairingRequested { .. }) => {}
                        Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {}
                        Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                    }
                }
                event = events.recv() => {
                    match event {
                        Ok(event) => {
                            if let Err(error) = crate::service::emit_messaging_event(&connection, event).await {
                                tracing::warn!(%error, "failed to emit messaging signal");
                            }
                        }
                        Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {}
                        Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                    }
                }
                _ = receipt_tick.tick() => service.flush_receipts().await,
                _ = outbox_tick.tick() => {
                    service.expire_outbox().await;
                    service.flush_due().await;
                }
            }
        }
    })
}

pub fn now_unix_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| {
            i64::try_from(duration.as_millis()).unwrap_or(i64::MAX)
        })
}

#[derive(Debug, Error)]
pub enum MessagingError {
    #[error(transparent)]
    Storage(#[from] StorageError),

    #[error(transparent)]
    Trust(#[from] TrustError),

    #[error(transparent)]
    Transport(#[from] TransportError),

    #[error(transparent)]
    Message(#[from] ticonnect_core::message::MessageError),

    #[error(transparent)]
    Sign(#[from] ticonnect_core::sign::SignError),

    #[error(transparent)]
    Ack(#[from] ticonnect_core::ack::AckError),

    #[error(transparent)]
    Wire(#[from] ticonnect_core::wire::WireError),

    #[error(transparent)]
    Settings(#[from] ticonnect_settings::Error),

    #[error("unknown peer: {0}")]
    UnknownPeer(String),

    #[error("unknown conversation: {0}")]
    UnknownConversation(String),

    #[error("unknown message: {0}")]
    UnknownMessage(String),

    #[error("invalid message id: {0}")]
    InvalidMessageId(String),

    #[error("invalid peer id: {0}")]
    InvalidPeerId(String),

    #[error("peer {0} has no stored signing key")]
    MissingPeerKey(String),

    #[error("peer {0} has an invalid signing key")]
    InvalidPeerKey(String),

    #[error("message sender does not match the peer")]
    SenderMismatch,

    #[error("message conversation does not match the peer")]
    ConversationMismatch,

    #[error("message body is empty")]
    EmptyBody,

    #[error("message body is too large")]
    BodyTooLarge,

    #[error("message kind {0} is not supported in this stage")]
    UnsupportedKind(String),

    #[error("attachment is invalid: {0}")]
    InvalidAttachment(String),

    #[error("message signature is invalid")]
    InvalidSignature,

    #[error("ack record is invalid")]
    InvalidAck,

    #[error("unexpected frame kind {0}")]
    UnexpectedFrame(u8),

    #[error("sync peer rejected the request with code {0}")]
    SyncRejected(u8),

    #[error("unexpected sync frame")]
    UnexpectedSyncFrame,
}
