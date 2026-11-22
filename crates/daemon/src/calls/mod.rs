pub mod session;
pub mod state;

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, PoisonError, RwLock};
use std::time::Duration;

use thiserror::Error;
use ticonnect_core::call::{AudioPacket, CallId, CallSignalFrame, CallState};
use ticonnect_core::id::NodeId;
use ticonnect_media::jitter::{JitterBuffer, JitterFrame};
use ticonnect_net::transport::{Session, Transport, TransportError, UserInfo};
use ticonnect_settings::Settings;
use tokio::task::JoinHandle;

use crate::messaging::now_unix_ms;
use crate::storage::{CallRecord, DatabaseHandle, StorageError, UserPolicy};
use crate::trust::{TrustError, TrustStore};

use session::{AudioReceiver, MediaError, MediaSession};
use state::{CallInfo, CallRegistry, CallStateError, CallTransition, EndReason};

const MEDIA_UNAVAILABLE_CODE: u8 = 1;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CallEvent {
    IncomingCall(CallInfo),
    StateChanged(CallInfo),
    Ended {
        call_id: CallId,
        user_id: String,
        reason: String,
    },
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct CallMetrics {
    pub frames_sent: u64,
    pub frames_received: u64,
    pub packets_lost: u64,
    pub bytes_sent: u64,
}

#[derive(Debug, Default)]
pub struct CallMetricsHandle {
    frames_sent: AtomicU64,
    frames_received: AtomicU64,
    packets_lost: AtomicU64,
    bytes_sent: AtomicU64,
}

impl CallMetricsHandle {
    pub fn snapshot(&self) -> CallMetrics {
        CallMetrics {
            frames_sent: self.frames_sent.load(Ordering::Relaxed),
            frames_received: self.frames_received.load(Ordering::Relaxed),
            packets_lost: self.packets_lost.load(Ordering::Relaxed),
            bytes_sent: self.bytes_sent.load(Ordering::Relaxed),
        }
    }

    pub fn note_sent(&self, bytes: usize) {
        self.frames_sent.fetch_add(1, Ordering::Relaxed);
        let bytes = u64::try_from(bytes).expect("usize length always fits u64");
        self.bytes_sent.fetch_add(bytes, Ordering::Relaxed);
    }

    pub fn note_received(&self) {
        self.frames_received.fetch_add(1, Ordering::Relaxed);
    }

    pub fn note_lost(&self) {
        self.packets_lost.fetch_add(1, Ordering::Relaxed);
    }
}

#[derive(Debug)]
pub struct InboundCallSignal {
    pub user: UserInfo,
    pub frame: CallSignalFrame,
}

#[derive(Debug)]
pub struct InboundAudio {
    pub user: UserInfo,
    pub payload: Vec<u8>,
}

#[derive(Clone)]
pub struct CallService {
    inner: Arc<Inner>,
}

struct Inner {
    local: NodeId,
    database: DatabaseHandle,
    trust: Arc<TrustStore>,
    transport: Arc<Transport>,
    settings: RwLock<Settings>,
    events: tokio::sync::broadcast::Sender<CallEvent>,
    registry: Mutex<CallRegistry>,
    media: Mutex<Option<MediaSession>>,
    audio: Mutex<Option<AudioReceiver>>,
    jitter: Mutex<JitterBuffer>,
    media_enabled: std::sync::atomic::AtomicBool,
    metrics: Arc<CallMetricsHandle>,
}

impl CallService {
    pub fn new(
        local: NodeId,
        database: DatabaseHandle,
        trust: Arc<TrustStore>,
        transport: Arc<Transport>,
        settings: Settings,
    ) -> Self {
        let (events, _) = tokio::sync::broadcast::channel(64);
        Self {
            inner: Arc::new(Inner {
                local,
                database,
                trust,
                transport,
                settings: RwLock::new(settings),
                events,
                registry: Mutex::new(CallRegistry::default()),
                media: Mutex::new(None),
                audio: Mutex::new(None),
                jitter: Mutex::new(JitterBuffer::default()),
                media_enabled: std::sync::atomic::AtomicBool::new(true),
                metrics: Arc::new(CallMetricsHandle::default()),
            }),
        }
    }

    pub fn subscribe(&self) -> tokio::sync::broadcast::Receiver<CallEvent> {
        self.inner.events.subscribe()
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

    pub fn set_media_enabled(&self, enabled: bool) {
        self.inner
            .media_enabled
            .store(enabled, std::sync::atomic::Ordering::Relaxed);
    }

    pub fn metrics(&self) -> CallMetrics {
        self.inner.metrics.snapshot()
    }

    pub fn state(&self) -> Option<CallInfo> {
        self.inner
            .registry
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .active()
            .map(state::ActiveCall::info)
    }

    pub async fn start_call(&self, user_id: &str) -> Result<CallInfo, CallError> {
        let user = self.user(user_id).await?;
        if user.policy == UserPolicy::Ignore {
            return Err(CallError::Rejected);
        }
        let user_node = parse_user(user_id)?;
        let session = self
            .inner
            .transport
            .session_by_node_id(user_node)
            .ok_or(CallError::UserOffline)?;
        let call = {
            let mut registry = self
                .inner
                .registry
                .lock()
                .unwrap_or_else(PoisonError::into_inner);
            match registry.start(user_id, now_unix_ms())? {
                CallTransition::Ringing(call) => call,
                _ => return Err(CallError::InvalidState),
            }
        };
        let info = call.info();
        tracing::info!(call_id = %call.id, user_id = %call.user_id, "outgoing call started");
        self.emit(CallEvent::StateChanged(info.clone()));
        if let Err(error) = self
            .send_signal(&session, Endpoint::User, &CallSignalFrame::Offer(call.id))
            .await
        {
            self.fail(call.id).await;
            return Err(error);
        }
        self.spawn_timeout(call.id, call.user_id.clone());
        Ok(info)
    }

    pub async fn accept_call(&self, call_id: CallId) -> Result<CallInfo, CallError> {
        let transition = {
            let mut registry = self
                .inner
                .registry
                .lock()
                .unwrap_or_else(PoisonError::into_inner);
            registry.accept(call_id, now_unix_ms())?
        };
        let CallTransition::InCall(call) = transition else {
            return Err(CallError::InvalidState);
        };
        tracing::info!(call_id = %call.id, user_id = %call.user_id, "call accepted");
        if let Err(error) = self
            .send_signal_for_user(&call.user_id, CallSignalFrame::Accept(call.id))
            .await
        {
            self.fail(call.id).await;
            return Err(error);
        }
        if let Err(error) = self.start_media(&call) {
            let _ = self
                .send_signal_for_user(
                    &call.user_id,
                    CallSignalFrame::Error {
                        call_id: call.id,
                        code: MEDIA_UNAVAILABLE_CODE,
                    },
                )
                .await;
            self.fail(call.id).await;
            return Err(error);
        }
        let info = call.info();
        self.emit(CallEvent::StateChanged(info.clone()));
        Ok(info)
    }

    pub fn set_muted(
        &self,
        call_id: CallId,
        input_muted: bool,
        output_muted: bool,
    ) -> Result<CallInfo, CallError> {
        let info = {
            let mut registry = self
                .inner
                .registry
                .lock()
                .unwrap_or_else(PoisonError::into_inner);
            registry
                .set_muted(call_id, input_muted, output_muted)?
                .info()
        };
        if let Some(media) = self
            .inner
            .media
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .as_mut()
        {
            media.set_input_muted(input_muted);
            media.set_output_muted(output_muted);
        }
        self.emit(CallEvent::StateChanged(info.clone()));
        Ok(info)
    }

    pub async fn decline_call(&self, call_id: CallId) -> Result<(), CallError> {
        let transition = {
            let mut registry = self
                .inner
                .registry
                .lock()
                .unwrap_or_else(PoisonError::into_inner);
            registry.decline(call_id)?
        };
        self.finish(transition, true).await;
        Ok(())
    }

    pub async fn hangup_call(&self, call_id: CallId) -> Result<(), CallError> {
        let transition = {
            let mut registry = self
                .inner
                .registry
                .lock()
                .unwrap_or_else(PoisonError::into_inner);
            registry.hangup(call_id)?
        };
        self.finish(transition, true).await;
        Ok(())
    }

    pub async fn handle_signal(
        &self,
        user: UserInfo,
        frame: CallSignalFrame,
    ) -> Result<(), CallError> {
        let user_id = user.node_id.to_string();
        let stored = self
            .inner
            .trust
            .user_by_fingerprint(&user.fingerprint.to_hex())
            .await?
            .ok_or_else(|| CallError::UnknownUser(user_id.clone()))?;
        if stored.policy == UserPolicy::Ignore {
            return Ok(());
        }
        match frame {
            CallSignalFrame::Offer(call_id) => {
                let transition = {
                    let mut registry = self
                        .inner
                        .registry
                        .lock()
                        .unwrap_or_else(PoisonError::into_inner);
                    registry.incoming_with_id(call_id, &user_id, now_unix_ms())
                };
                let call = match transition {
                    Ok(CallTransition::Ringing(call)) => call,
                    Ok(_) | Err(CallStateError::Busy) => {
                        let _ = self
                            .send_signal_frame(&user, CallSignalFrame::Busy(call_id))
                            .await;
                        return Ok(());
                    }
                    Err(error) => return Err(error.into()),
                };
                let info = call.info();
                tracing::info!(call_id = %call.id, user_id = %user_id, "incoming call received");
                self.emit(CallEvent::IncomingCall(info.clone()));
                let _ = self
                    .send_signal_frame(&user, CallSignalFrame::Ringing(call.id))
                    .await;
                self.spawn_timeout(call.id, user_id);
                Ok(())
            }
            CallSignalFrame::Ringing(call_id) => {
                self.ensure_active(call_id, &user_id)?;
                Ok(())
            }
            CallSignalFrame::Accept(call_id) => {
                let transition = {
                    let mut registry = self
                        .inner
                        .registry
                        .lock()
                        .unwrap_or_else(PoisonError::into_inner);
                    registry.accept(call_id, now_unix_ms())
                };
                let CallTransition::InCall(call) = transition? else {
                    return Err(CallError::InvalidState);
                };
                if let Err(error) = self.start_media(&call) {
                    let _ = self
                        .send_signal_frame(
                            &user,
                            CallSignalFrame::Error {
                                call_id: call.id,
                                code: MEDIA_UNAVAILABLE_CODE,
                            },
                        )
                        .await;
                    self.fail(call.id).await;
                    return Err(error);
                }
                self.emit(CallEvent::StateChanged(call.info()));
                Ok(())
            }
            CallSignalFrame::Decline(call_id) => {
                self.ensure_active(call_id, &user_id)?;
                let transition = {
                    let mut registry = self
                        .inner
                        .registry
                        .lock()
                        .unwrap_or_else(PoisonError::into_inner);
                    registry.decline(call_id)?
                };
                self.finish(transition, false).await;
                Ok(())
            }
            CallSignalFrame::Hangup(call_id) => {
                self.ensure_active(call_id, &user_id)?;
                let transition = {
                    let mut registry = self
                        .inner
                        .registry
                        .lock()
                        .unwrap_or_else(PoisonError::into_inner);
                    registry.hangup(call_id)?
                };
                self.finish(transition, false).await;
                Ok(())
            }
            CallSignalFrame::Busy(call_id) | CallSignalFrame::Error { call_id, .. } => {
                self.ensure_active(call_id, &user_id)?;
                let transition = {
                    let mut registry = self
                        .inner
                        .registry
                        .lock()
                        .unwrap_or_else(PoisonError::into_inner);
                    registry.error(call_id)?
                };
                self.finish(transition, false).await;
                Ok(())
            }
        }
    }

    pub fn handle_audio(&self, user: UserInfo, payload: &[u8]) -> Result<(), CallError> {
        let packet =
            AudioPacket::decode(payload).map_err(|error| CallError::Protocol(error.to_string()))?;
        let call_id = packet.id();
        self.ensure_active(call_id, &user.node_id.to_string())?;
        self.inner.metrics.note_received();
        {
            let mut jitter = self
                .inner
                .jitter
                .lock()
                .unwrap_or_else(PoisonError::into_inner);
            jitter.push(packet);
            let lost_before = jitter.lost();
            while let Some(frame) = jitter.pop() {
                let samples = match frame {
                    JitterFrame::Packet(packet) => {
                        let mut audio = self
                            .inner
                            .audio
                            .lock()
                            .unwrap_or_else(PoisonError::into_inner);
                        match audio.as_mut() {
                            Some(audio) => audio.decode(&packet).unwrap_or_default(),
                            None => Vec::new(),
                        }
                    }
                    JitterFrame::Lost(_) => {
                        let mut audio = self
                            .inner
                            .audio
                            .lock()
                            .unwrap_or_else(PoisonError::into_inner);
                        match audio.as_mut() {
                            Some(audio) => audio.conceal().unwrap_or_default(),
                            None => Vec::new(),
                        }
                    }
                };
                if let Some(media) = self
                    .inner
                    .media
                    .lock()
                    .unwrap_or_else(PoisonError::into_inner)
                    .as_mut()
                {
                    media.push_audio(&samples);
                }
            }
            let lost_after = jitter.lost();
            for _ in lost_before..lost_after {
                self.inner.metrics.note_lost();
            }
        }
        Ok(())
    }

    pub async fn handle_disconnect(&self, user: &UserInfo) {
        let ended = self
            .inner
            .registry
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .disconnected(&user.node_id.to_string());
        if let Some((call, reason)) = ended {
            self.cleanup();
            self.record_call(&call, reason).await;
            self.emit(CallEvent::Ended {
                call_id: call.id,
                user_id: call.user_id.clone(),
                reason: reason.as_str().to_owned(),
            });
        }
    }

    async fn finish(&self, transition: CallTransition, send: bool) {
        let CallTransition::Ended(call, reason) = transition else {
            return;
        };
        tracing::info!(call_id = %call.id, user_id = %call.user_id, reason = reason.as_str(), "call finished");
        self.cleanup();
        self.record_call(&call, reason).await;
        if send {
            let frame = match reason {
                EndReason::Declined => CallSignalFrame::Decline(call.id),
                _ => CallSignalFrame::Hangup(call.id),
            };
            let _ = self.send_signal_for_user(&call.user_id, frame).await;
        }
        self.emit(CallEvent::Ended {
            call_id: call.id,
            user_id: call.user_id.clone(),
            reason: reason.as_str().to_owned(),
        });
    }

    async fn fail(&self, call_id: CallId) {
        let ended = {
            let mut registry = self
                .inner
                .registry
                .lock()
                .unwrap_or_else(PoisonError::into_inner);
            registry.error(call_id).ok()
        };
        self.cleanup();
        if let Some(CallTransition::Ended(call, reason)) = ended {
            tracing::warn!(call_id = %call.id, user_id = %call.user_id, reason = reason.as_str(), "call failed");
            self.record_call(&call, reason).await;
            self.emit(CallEvent::Ended {
                call_id: call.id,
                user_id: call.user_id,
                reason: reason.as_str().to_owned(),
            });
        }
    }

    async fn record_call(&self, call: &state::ActiveCall, reason: EndReason) {
        let ended_at = now_unix_ms();
        let connected = call.connected_at.filter(|connected| *connected > 0);
        let duration_secs = match connected.and_then(|connected| ended_at.checked_sub(connected)) {
            Some(elapsed) if elapsed >= 0 => match u64::try_from(elapsed / 1000) {
                Ok(seconds) => seconds,
                Err(error) => {
                    tracing::error!(%error, "call duration conversion failed");
                    0
                }
            },
            Some(_) => {
                tracing::warn!("call clock moved backwards; storing zero duration");
                0
            }
            None => 0,
        };
        let direction = match call.direction {
            ticonnect_core::call::CallDirection::Incoming => "incoming",
            ticonnect_core::call::CallDirection::Outgoing => "outgoing",
        };
        let record = CallRecord {
            id: call.id.to_string(),
            user_id: call.user_id.clone(),
            direction: direction.to_owned(),
            started_at: call.created_at,
            connected_at: connected,
            ended_at,
            duration_secs,
            reason: reason.as_str().to_owned(),
        };
        if let Err(error) = self.inner.database.insert_call_log(record).await {
            tracing::error!(%error, "failed to persist call log");
        }
    }

    fn cleanup(&self) {
        if let Some(media) = self
            .inner
            .media
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .take()
        {
            media.stop();
        }
        *self
            .inner
            .audio
            .lock()
            .unwrap_or_else(PoisonError::into_inner) = None;
        *self
            .inner
            .jitter
            .lock()
            .unwrap_or_else(PoisonError::into_inner) = JitterBuffer::default();
    }

    fn start_media(&self, call: &state::ActiveCall) -> Result<(), CallError> {
        if !self
            .inner
            .media_enabled
            .load(std::sync::atomic::Ordering::Relaxed)
        {
            return Ok(());
        }
        let session = parse_user(&call.user_id)
            .ok()
            .and_then(|node_id| self.inner.transport.session_by_node_id(node_id))
            .ok_or(CallError::UserOffline)?;
        let settings = self.settings();
        tracing::info!(
            call_id = %call.id,
            user_id = %call.user_id,
            input_device = settings.call_input_device().unwrap_or("system default"),
            output_device = settings.call_output_device().unwrap_or("system default"),
            "starting call media"
        );
        let media = MediaSession::start(
            call.id,
            session,
            settings.call_input_device(),
            settings.call_output_device(),
            call.input_muted,
            call.output_muted,
            self.inner.metrics.clone(),
        )
        .map_err(|error| {
            tracing::warn!(call_id = %call.id, user_id = %call.user_id, %error, "failed to start call media");
            CallError::Media(error)
        })?;
        *self
            .inner
            .media
            .lock()
            .unwrap_or_else(PoisonError::into_inner) = Some(media);
        *self
            .inner
            .audio
            .lock()
            .unwrap_or_else(PoisonError::into_inner) = AudioReceiver::new().ok();
        Ok(())
    }

    fn ensure_active(&self, call_id: CallId, user_id: &str) -> Result<(), CallError> {
        let registry = self
            .inner
            .registry
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        match registry.active() {
            Some(call) if call.id == call_id && call.user_id == user_id => Ok(()),
            Some(_) => Err(CallError::InvalidState),
            None => Err(CallError::UnknownCall(call_id)),
        }
    }

    async fn user(&self, user_id: &str) -> Result<crate::storage::User, CallError> {
        self.inner
            .trust
            .user(user_id)
            .await?
            .ok_or_else(|| CallError::UnknownUser(user_id.to_owned()))
    }

    fn spawn_timeout(&self, call_id: CallId, user_id: String) {
        let service = self.clone();
        let timeout = u64::from(self.settings().call_timeout_secs());
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_secs(timeout)).await;
            let transition = {
                let mut registry = service
                    .inner
                    .registry
                    .lock()
                    .unwrap_or_else(PoisonError::into_inner);
                match registry.active() {
                    Some(call) if call.id == call_id && call.state == CallState::Ringing => {
                        registry.timeout(call_id).ok()
                    }
                    _ => None,
                }
            };
            if let Some(transition) = transition {
                service.finish(transition, true).await;
                tracing::debug!(user_id, "call timed out");
            }
        });
    }

    async fn send_signal_for_user(
        &self,
        user_id: &str,
        frame: CallSignalFrame,
    ) -> Result<(), CallError> {
        let user_id = parse_user(user_id)?;
        let session = self
            .inner
            .transport
            .session_by_node_id(user_id)
            .ok_or(CallError::UserOffline)?;
        self.send_signal(&session, Endpoint::User, &frame).await
    }

    async fn send_signal_frame(
        &self,
        user: &UserInfo,
        frame: CallSignalFrame,
    ) -> Result<(), CallError> {
        let Some(session) = self
            .inner
            .transport
            .session_by_fingerprint(&user.fingerprint)
        else {
            return Err(CallError::UserOffline);
        };
        self.send_signal(&session, Endpoint::User, &frame).await
    }

    async fn send_signal(
        &self,
        session: &Session,
        _endpoint: Endpoint,
        frame: &CallSignalFrame,
    ) -> Result<(), CallError> {
        let mut send = session.open_uni().await?;
        ticonnect_net::transport::session::write_frame(&mut send, &frame.to_wire()?).await?;
        Ok(())
    }

    fn emit(&self, event: CallEvent) {
        let _ = self.inner.events.send(event);
    }

    pub fn local_node(&self) -> NodeId {
        self.inner.local
    }
}

enum Endpoint {
    User,
}

pub fn spawn(
    service: CallService,
    mut signals: tokio::sync::mpsc::UnboundedReceiver<InboundCallSignal>,
    mut audio: tokio::sync::mpsc::UnboundedReceiver<InboundAudio>,
) -> JoinHandle<()> {
    let mut transport_events = service.inner.transport.subscribe();
    tokio::spawn(async move {
        loop {
            tokio::select! {
                signal = signals.recv() => {
                    let Some(signal) = signal else { break };
                    if let Err(error) = service.handle_signal(signal.user, signal.frame).await {
                        tracing::debug!(%error, "call signal rejected");
                    }
                }
                packet = audio.recv() => {
                    let Some(packet) = packet else { break };
                    if let Err(error) = service.handle_audio(packet.user, &packet.payload) {
                        tracing::debug!(%error, "call audio rejected");
                    }
                }
                event = transport_events.recv() => {
                    match event {
                        Ok(ticonnect_net::transport::TransportEvent::UserDisconnected(user)) => {
                            service.handle_disconnect(&user).await;
                        }
                        Ok(_) => {}
                        Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {}
                        Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                    }
                }
            }
        }
    })
}

fn parse_user(user_id: &str) -> Result<NodeId, CallError> {
    user_id
        .parse()
        .map_err(|_| CallError::InvalidUser(user_id.to_owned()))
}

#[derive(Debug, Error)]
pub enum CallError {
    #[error(transparent)]
    State(#[from] CallStateError),

    #[error(transparent)]
    Trust(#[from] TrustError),

    #[error(transparent)]
    Storage(#[from] StorageError),

    #[error(transparent)]
    Transport(#[from] TransportError),

    #[error(transparent)]
    Media(#[from] MediaError),

    #[error(transparent)]
    Signal(#[from] ticonnect_core::call::CallError),

    #[error("unknown user: {0}")]
    UnknownUser(String),

    #[error("invalid user id: {0}")]
    InvalidUser(String),

    #[error("call was rejected by policy")]
    Rejected,

    #[error("user is offline")]
    UserOffline,

    #[error("unknown call: {0}")]
    UnknownCall(CallId),

    #[error("call state is invalid")]
    InvalidState,

    #[error("call protocol error: {0}")]
    Protocol(String),
}
