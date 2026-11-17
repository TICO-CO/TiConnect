pub mod outbox;
pub mod policy;
pub mod receipts;
pub mod store;
pub mod sync;

use std::collections::{HashMap, HashSet};
use std::str::FromStr;
use std::sync::{Arc, Mutex, PoisonError, RwLock};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use thiserror::Error;
use ticonnect_core::ack::AckKind;
use ticonnect_core::attachment::AttachmentRef;
use ticonnect_core::id::{MessageId, NodeId};
use ticonnect_core::identity::IdentityKeyPair;
use ticonnect_core::message::{MessageKind, MessageRecord};
use ticonnect_core::wire::{Frame, FrameKind};
use ticonnect_net::transport::{Session, Transport, TransportEvent, UserInfo};
use ticonnect_settings::Settings;
use tokio::sync::mpsc::UnboundedSender;
use tokio::task::JoinHandle;

use crate::attachments::transfer::InboundTransfer;
use crate::calls::{InboundAudio, InboundCallSignal};
use crate::remote::InboundRemoteFrame;
use crate::storage::{
    DatabaseHandle, InsertOutcome, MessageState, StorageError, StoredMessage, User, UserPolicy,
};
use crate::trust::{TrustError, TrustStore};
use crate::watch_party::InboundWatchPartyFrame;
use receipts::{PendingAck, ReceiptCoalescer};

use ticonnect_net::transport::TransportError;

const RECEIPT_TICK: Duration = Duration::from_millis(100);
const OUTBOX_TICK: Duration = Duration::from_secs(1);

#[derive(Debug, Clone)]
#[allow(clippy::large_enum_variant)]
pub enum MessagingEvent {
    MessageReceived {
        user_id: String,
        message: StoredMessage,
        notify: bool,
    },
    MessageStateChanged {
        message_id: String,
        state: MessageState,
    },
    SyncCompleted {
        user_id: String,
        count: u32,
    },
    PolicyChanged {
        user_id: String,
        policy: UserPolicy,
    },
    TypingChanged {
        user_id: String,
        typing: bool,
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
    watch_party_tx: Mutex<Option<UnboundedSender<InboundWatchPartyFrame>>>,
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
                watch_party_tx: Mutex::new(None),
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

    pub fn set_watch_party_sender(&self, sender: UnboundedSender<InboundWatchPartyFrame>) {
        *self
            .inner
            .watch_party_tx
            .lock()
            .unwrap_or_else(PoisonError::into_inner) = Some(sender);
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

    fn watch_party_sender(&self) -> Option<UnboundedSender<InboundWatchPartyFrame>> {
        self.inner
            .watch_party_tx
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

    pub async fn user(&self, user_id: &str) -> Result<User, MessagingError> {
        self.inner
            .trust
            .user(user_id)
            .await?
            .ok_or_else(|| MessagingError::UnknownUser(user_id.to_owned()))
    }

    pub async fn conversation_id(&self, user_id: &str) -> Result<String, MessagingError> {
        let user = self.user(user_id).await?;
        let user_node: NodeId = user
            .id
            .parse()
            .map_err(|_| MessagingError::InvalidUserId(user.id.clone()))?;
        Ok(ticonnect_core::message::conversation_id(
            self.inner.local,
            user_node,
        ))
    }

    pub async fn send_message(
        &self,
        user_id: &str,
        body: &str,
    ) -> Result<MessageId, MessagingError> {
        if body.is_empty() {
            return Err(MessagingError::EmptyBody);
        }
        if body.len() > ticonnect_core::message::MAX_MESSAGE_BODY {
            return Err(MessagingError::BodyTooLarge);
        }

        let user = self.user(user_id).await?;
        let user_node: NodeId = user
            .id
            .parse()
            .map_err(|_| MessagingError::InvalidUserId(user.id.clone()))?;
        let now = now_unix_ms();
        self.inner
            .database
            .ensure_conversation(
                &ticonnect_core::message::conversation_id(self.inner.local, user_node),
                user_id,
                now,
            )
            .await?;
        let seq = self.inner.database.allocate_seq(user_id).await?;
        let record = store::text(self.inner.local, user_node, seq, now, body.to_owned());
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
        outbox::flush_user(self, user_id).await;
        Ok(record.message_id)
    }

    pub async fn send_attachment(
        &self,
        user_id: &str,
        attachment: &AttachmentRef,
    ) -> Result<MessageId, MessagingError> {
        attachment
            .validate(ticonnect_core::attachment::DEFAULT_MAX_ATTACHMENT_SIZE)
            .map_err(|error| MessagingError::InvalidAttachment(error.to_string()))?;
        let user = self.user(user_id).await?;
        let user_node: NodeId = user
            .id
            .parse()
            .map_err(|_| MessagingError::InvalidUserId(user.id.clone()))?;
        let now = now_unix_ms();
        self.inner
            .database
            .ensure_conversation(
                &ticonnect_core::message::conversation_id(self.inner.local, user_node),
                user_id,
                now,
            )
            .await?;
        let seq = self.inner.database.allocate_seq(user_id).await?;
        let record = store::attachment(self.inner.local, user_node, seq, now, attachment)?;
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
        outbox::flush_user(self, user_id).await;
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
        let user_id = self.conversation_user(conversation_id).await?;
        let user = self.user(&user_id).await?;
        self.inner
            .database
            .mark_messages_read(conversation_id, message_ids, now_unix_ms())
            .await?;
        if !policy::read_receipt(user.policy, self.settings().read_receipts_enabled()) {
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
            self.queue_receipt(&user_id, target, AckKind::Read);
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

    pub async fn set_typing(&self, user_id: &str, typing: bool) -> Result<(), MessagingError> {
        if !self.settings().typing_enabled() {
            return Ok(());
        }
        let user = self.user(user_id).await?;
        if user.policy == UserPolicy::Ignore {
            return Ok(());
        }
        let node_id = NodeId::from_str(user_id)
            .map_err(|_| MessagingError::UnknownUser(user_id.to_owned()))?;
        let Some(session) = self.inner.transport.session_by_node_id(node_id) else {
            return Ok(());
        };
        let frame = Frame::new(
            FrameKind::Typing,
            ticonnect_core::wire::TypingFrame::new(typing).encode(),
        )?;
        let mut send = session.open_uni().await?;
        ticonnect_net::transport::session::write_frame(&mut send, &frame).await?;
        Ok(())
    }

    pub async fn set_user_policy(
        &self,
        user_id: &str,
        policy: UserPolicy,
    ) -> Result<(), MessagingError> {
        if !self
            .inner
            .database
            .update_user_policy(user_id, policy)
            .await?
        {
            return Err(MessagingError::UnknownUser(user_id.to_owned()));
        }
        self.emit(MessagingEvent::PolicyChanged {
            user_id: user_id.to_owned(),
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

    pub async fn on_user_connected(&self, user: UserInfo) {
        let fingerprint = user.fingerprint.to_hex();
        if let Ok(Some(stored)) = self.inner.trust.user_by_fingerprint(&fingerprint).await
            && stored.public_key.is_none()
        {
            let _ = self
                .inner
                .database
                .update_user_public_key(&stored.id, user.public_key)
                .await;
        }

        let Some(session) = self
            .inner
            .transport
            .session_by_fingerprint(&user.fingerprint)
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
                                user: datagram_session.user().clone(),
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

    pub fn on_user_disconnected(&self, user: &UserInfo) {
        let fingerprint = user.fingerprint.to_hex();
        self.clear_sync_ready(&user.node_id.to_string());
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
        let user = session.user().clone();
        loop {
            tokio::select! {
                result = session.accept_uni() => {
                    match result {
                        Ok(mut recv) => {
                            let service = self.clone();
                            let user = user.clone();
                            tokio::spawn(async move {
                                match ticonnect_net::transport::session::read_frame(&mut recv).await {
                                    Ok(frame) => {
                                        if let Err(error) = service.handle_frame(&user, frame).await {
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
                            let user = user.clone();
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
                                        user,
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
                                if let Err(error) = sync::run_responder(service, user, hello, &mut send, &mut recv).await {
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

    async fn handle_frame(&self, user: &UserInfo, frame: Frame) -> Result<(), MessagingError> {
        match frame.kind() {
            FrameKind::Message | FrameKind::Ack => {}
            FrameKind::Typing => {
                let typing = ticonnect_core::wire::TypingFrame::decode(frame.payload())?;
                let stored = self
                    .inner
                    .trust
                    .user_by_fingerprint(&user.fingerprint.to_hex())
                    .await?
                    .ok_or_else(|| MessagingError::UnknownUser(user.node_id.to_string()))?;
                if stored.policy != UserPolicy::Ignore && self.settings().typing_enabled() {
                    self.emit(MessagingEvent::TypingChanged {
                        user_id: stored.id,
                        typing: typing.typing,
                    });
                }
                return Ok(());
            }
            FrameKind::Call => {
                let frame = ticonnect_core::call::CallSignalFrame::from_wire(&frame)
                    .map_err(|error| MessagingError::InvalidAttachment(error.to_string()))?;
                if let Some(sender) = self.call_sender() {
                    let _ = sender.send(InboundCallSignal {
                        user: user.clone(),
                        frame,
                    });
                }
                return Ok(());
            }
            FrameKind::WatchParty => {
                let frame = ticonnect_core::watch_party::WatchPartyFrame::from_wire(&frame)
                    .map_err(|error| MessagingError::InvalidAttachment(error.to_string()))?;
                if let Some(sender) = self.watch_party_sender() {
                    let _ = sender.send(InboundWatchPartyFrame {
                        user: user.clone(),
                        frame,
                    });
                }
                return Ok(());
            }
            FrameKind::Remote | FrameKind::Input => {
                if let Some(sender) = self.remote_sender() {
                    let _ = sender.send(InboundRemoteFrame {
                        user: user.clone(),
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
            .user_by_fingerprint(&user.fingerprint.to_hex())
            .await?
            .ok_or_else(|| MessagingError::UnknownUser(user.node_id.to_string()))?;
        if stored.policy == UserPolicy::Ignore {
            return Ok(());
        }
        if stored.public_key.is_none() {
            self.inner
                .database
                .update_user_public_key(&stored.id, user.public_key)
                .await?;
        }
        self.ingest_payload(&stored.id, frame.payload()).await?;
        Ok(())
    }

    pub(crate) async fn ingest_payload(
        &self,
        user_id: &str,
        bytes: &[u8],
    ) -> Result<bool, MessagingError> {
        let user = self.user(user_id).await?;
        if user.policy == UserPolicy::Ignore {
            return Ok(false);
        }
        let (record, signature) = store::verified(bytes, &user, self.inner.local)?;
        match record.kind {
            MessageKind::Ack => {
                self.ingest_ack(&user, record, signature).await?;
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
                self.ingest_text(&user, record, signature).await?;
                Ok(true)
            }
        }
    }

    async fn ingest_text(
        &self,
        user: &User,
        record: MessageRecord,
        signature: Vec<u8>,
    ) -> Result<(), MessagingError> {
        let decision = policy::inbound(
            user.policy,
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
            .insert_inbound(record.clone(), received_at, signature, &user.id)
            .await?;
        if outcome == InsertOutcome::Inserted {
            let stored = self
                .inner
                .database
                .get_message(&record.message_id.to_string())
                .await?
                .ok_or_else(|| MessagingError::UnknownMessage(record.message_id.to_string()))?;
            self.emit(MessagingEvent::MessageReceived {
                user_id: user.id.clone(),
                message: stored,
                notify,
            });
        }
        if delivered_receipt {
            self.queue_receipt(&user.id, record.message_id, AckKind::Delivered);
        }
        Ok(())
    }

    async fn ingest_ack(
        &self,
        user: &User,
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
            .insert_inbound(record, received_at, signature, &user.id)
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

    fn queue_receipt(&self, user_id: &str, target: MessageId, kind: AckKind) {
        self.inner
            .receipts
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .queue(user_id, target, kind);
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
        let user = self.user(&ack.user_id).await?;
        let user_node: NodeId = user
            .id
            .parse()
            .map_err(|_| MessagingError::InvalidUserId(user.id.clone()))?;
        let conversation = ticonnect_core::message::conversation_id(self.inner.local, user_node);
        self.inner
            .database
            .ensure_conversation(&conversation, &user.id, now_unix_ms())
            .await?;
        let seq = self.inner.database.allocate_seq(&user.id).await?;
        let record = store::ack(
            self.inner.local,
            user_node,
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
        outbox::flush_user(self, &ack.user_id).await;
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
        let users = match self.inner.database.list_users().await {
            Ok(users) => users,
            Err(error) => {
                tracing::warn!(%error, "failed to list users for outbox");
                return;
            }
        };
        for user in users {
            outbox::flush_user(self, &user.id).await;
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

    pub fn sync_ready(&self, user_id: &str) -> bool {
        self.is_sync_ready(user_id)
    }

    pub(crate) fn is_sync_ready(&self, user_id: &str) -> bool {
        self.inner
            .sync_ready
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .contains(user_id)
    }

    pub(crate) fn mark_sync_ready(&self, user_id: &str) {
        self.inner
            .sync_ready
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .insert(user_id.to_owned());
    }

    pub(crate) fn clear_sync_ready(&self, user_id: &str) {
        self.inner
            .sync_ready
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .remove(user_id);
    }

    pub(crate) fn transport(&self) -> &Transport {
        &self.inner.transport
    }

    pub(crate) fn emit_sync_completed(&self, user_id: &str, count: u32) {
        self.emit(MessagingEvent::SyncCompleted {
            user_id: user_id.to_owned(),
            count,
        });
    }

    pub(crate) fn emit_state_changed(&self, message_id: &str, state: MessageState) {
        self.emit(MessagingEvent::MessageStateChanged {
            message_id: message_id.to_owned(),
            state,
        });
    }

    async fn conversation_user(&self, conversation_id: &str) -> Result<String, MessagingError> {
        let users = self.inner.database.list_users().await?;
        for user in users {
            let user_node: NodeId = user
                .id
                .parse()
                .map_err(|_| MessagingError::InvalidUserId(user.id.clone()))?;
            if ticonnect_core::message::conversation_id(self.inner.local, user_node)
                == conversation_id
            {
                return Ok(user.id);
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
                        Ok(TransportEvent::UserConnected(user)) => service.on_user_connected(user).await,
                        Ok(TransportEvent::UserDisconnected(user)) => service.on_user_disconnected(&user),
                        Ok(TransportEvent::TrustRequested { .. }) => {}
                        Ok(TransportEvent::ProfileRequested { .. }) => {}
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

    #[error("unknown user: {0}")]
    UnknownUser(String),

    #[error("unknown conversation: {0}")]
    UnknownConversation(String),

    #[error("unknown message: {0}")]
    UnknownMessage(String),

    #[error("invalid message id: {0}")]
    InvalidMessageId(String),

    #[error("invalid user id: {0}")]
    InvalidUserId(String),

    #[error("user {0} has no stored signing key")]
    MissingUserKey(String),

    #[error("user {0} has an invalid signing key")]
    InvalidUserKey(String),

    #[error("message sender does not match the user")]
    SenderMismatch,

    #[error("message conversation does not match the user")]
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

    #[error("sync user rejected the request with code {0}")]
    SyncRejected(u8),

    #[error("unexpected sync frame")]
    UnexpectedSyncFrame,
}
