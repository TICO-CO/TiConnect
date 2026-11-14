pub mod session;
pub mod state;

use std::sync::{Arc, Mutex, PoisonError, RwLock};
use std::time::Duration;

use thiserror::Error;
use ticonnect_core::call::{AudioPacket, CallId, CallSignalFrame, CallState};
use ticonnect_core::id::NodeId;
use ticonnect_media::jitter::{JitterBuffer, JitterFrame};
use ticonnect_media::ringtone::{RingtoneControl, RingtoneError};
use ticonnect_net::transport::{PeerInfo, Session, Transport, TransportError};
use ticonnect_settings::Settings;
use tokio::task::JoinHandle;

use crate::messaging::now_unix_ms;
use crate::storage::{DatabaseHandle, PeerPolicy, StorageError};
use crate::trust::{TrustError, TrustStore};

use session::{AudioReceiver, MediaError, MediaSession};
use state::{CallInfo, CallRegistry, CallStateError, CallTransition, EndReason};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CallEvent {
    IncomingCall(CallInfo),
    StateChanged(CallInfo),
    Ended {
        call_id: CallId,
        peer_id: String,
        reason: String,
    },
}

#[derive(Debug)]
pub struct InboundCallSignal {
    pub peer: PeerInfo,
    pub frame: CallSignalFrame,
}

#[derive(Debug)]
pub struct InboundAudio {
    pub peer: PeerInfo,
    pub payload: Vec<u8>,
}

#[derive(Clone)]
pub struct CallService {
    inner: Arc<Inner>,
}

struct Inner {
    local: NodeId,
    _database: DatabaseHandle,
    trust: Arc<TrustStore>,
    transport: Arc<Transport>,
    settings: RwLock<Settings>,
    events: tokio::sync::broadcast::Sender<CallEvent>,
    registry: Mutex<CallRegistry>,
    ringtone: Arc<dyn RingtoneControl>,
    media: Mutex<Option<MediaSession>>,
    audio: Mutex<Option<AudioReceiver>>,
    jitter: Mutex<JitterBuffer>,
    media_enabled: std::sync::atomic::AtomicBool,
}

impl CallService {
    pub fn new(
        local: NodeId,
        database: DatabaseHandle,
        trust: Arc<TrustStore>,
        transport: Arc<Transport>,
        settings: Settings,
        ringtone: Arc<dyn RingtoneControl>,
    ) -> Self {
        let (events, _) = tokio::sync::broadcast::channel(64);
        Self {
            inner: Arc::new(Inner {
                local,
                _database: database,
                trust,
                transport,
                settings: RwLock::new(settings),
                events,
                registry: Mutex::new(CallRegistry::default()),
                ringtone,
                media: Mutex::new(None),
                audio: Mutex::new(None),
                jitter: Mutex::new(JitterBuffer::default()),
                media_enabled: std::sync::atomic::AtomicBool::new(true),
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

    pub fn state(&self) -> Option<CallInfo> {
        self.inner
            .registry
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .active()
            .map(state::ActiveCall::info)
    }

    pub async fn start_call(&self, peer_id: &str) -> Result<CallInfo, CallError> {
        let peer = self.peer(peer_id).await?;
        if peer.policy == PeerPolicy::Ignore {
            return Err(CallError::Rejected);
        }
        let peer_node = parse_peer(peer_id)?;
        let session = self
            .inner
            .transport
            .session_by_node_id(peer_node)
            .ok_or(CallError::PeerOffline)?;
        let call = {
            let mut registry = self
                .inner
                .registry
                .lock()
                .unwrap_or_else(PoisonError::into_inner);
            match registry.start(peer_id, now_unix_ms())? {
                CallTransition::Ringing(call) => call,
                _ => return Err(CallError::InvalidState),
            }
        };
        let info = call.info();
        self.emit(CallEvent::StateChanged(info.clone()));
        if let Err(error) = self
            .send_signal(&session, Endpoint::Peer, &CallSignalFrame::Offer(call.id))
            .await
        {
            self.fail(call.id);
            return Err(error);
        }
        self.spawn_timeout(call.id, call.peer_id.clone());
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
        self.inner.ringtone.stop();
        self.inner.ringtone.duck(false);
        if let Err(error) = self
            .send_signal_for_peer(&call.peer_id, CallSignalFrame::Accept(call.id))
            .await
        {
            self.fail(call.id);
            return Err(error);
        }
        if !self.start_media(&call) {
            self.fail(call.id);
            return Err(CallError::MediaUnavailable);
        }
        let info = call.info();
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
        peer: PeerInfo,
        frame: CallSignalFrame,
    ) -> Result<(), CallError> {
        let peer_id = peer.node_id.to_string();
        let stored = self
            .inner
            .trust
            .peer_by_fingerprint(&peer.fingerprint.to_hex())
            .await?
            .ok_or_else(|| CallError::UnknownPeer(peer_id.clone()))?;
        if stored.policy == PeerPolicy::Ignore {
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
                    registry.incoming(&peer_id, now_unix_ms())
                };
                let call = match transition {
                    Ok(CallTransition::Ringing(call)) => call,
                    Ok(_) | Err(CallStateError::Busy) => {
                        let _ = self
                            .send_signal_frame(&peer, CallSignalFrame::Busy(call_id))
                            .await;
                        return Ok(());
                    }
                    Err(error) => return Err(error.into()),
                };
                let info = call.info();
                self.emit(CallEvent::IncomingCall(info.clone()));
                let settings = self.settings();
                if !settings.dnd_enabled()
                    && let Err(error) = self.inner.ringtone.start(settings.ringtone_path())
                {
                    tracing::warn!(%error, "failed to start ringtone");
                }
                let _ = self
                    .send_signal_frame(&peer, CallSignalFrame::Ringing(call.id))
                    .await;
                self.spawn_timeout(call.id, peer_id);
                Ok(())
            }
            CallSignalFrame::Ringing(call_id) => {
                self.ensure_active(call_id, &peer_id)?;
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
                self.inner.ringtone.stop();
                if !self.start_media(&call) {
                    self.fail(call.id);
                    return Err(CallError::MediaUnavailable);
                }
                self.emit(CallEvent::StateChanged(call.info()));
                Ok(())
            }
            CallSignalFrame::Decline(call_id) => {
                self.ensure_active(call_id, &peer_id)?;
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
                self.ensure_active(call_id, &peer_id)?;
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
                self.ensure_active(call_id, &peer_id)?;
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

    pub fn handle_audio(&self, peer: PeerInfo, payload: &[u8]) -> Result<(), CallError> {
        let packet =
            AudioPacket::decode(payload).map_err(|error| CallError::Protocol(error.to_string()))?;
        let call_id = packet.id();
        self.ensure_active(call_id, &peer.node_id.to_string())?;
        {
            let mut jitter = self
                .inner
                .jitter
                .lock()
                .unwrap_or_else(PoisonError::into_inner);
            jitter.push(packet);
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
        }
        Ok(())
    }

    pub fn handle_disconnect(&self, peer: &PeerInfo) {
        let ended = self
            .inner
            .registry
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .disconnected(&peer.node_id.to_string());
        if let Some((call, reason)) = ended {
            self.cleanup();
            self.emit(CallEvent::Ended {
                call_id: call.id,
                peer_id: call.peer_id.clone(),
                reason: reason.as_str().to_owned(),
            });
        }
    }

    async fn finish(&self, transition: CallTransition, send: bool) {
        let CallTransition::Ended(call, reason) = transition else {
            return;
        };
        self.cleanup();
        if send {
            let frame = match reason {
                EndReason::Declined => CallSignalFrame::Decline(call.id),
                _ => CallSignalFrame::Hangup(call.id),
            };
            let _ = self.send_signal_for_peer(&call.peer_id, frame).await;
        }
        self.emit(CallEvent::Ended {
            call_id: call.id,
            peer_id: call.peer_id.clone(),
            reason: reason.as_str().to_owned(),
        });
    }

    fn fail(&self, call_id: CallId) {
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
            self.emit(CallEvent::Ended {
                call_id: call.id,
                peer_id: call.peer_id,
                reason: reason.as_str().to_owned(),
            });
        }
    }

    fn cleanup(&self) {
        self.inner.ringtone.stop();
        self.inner.ringtone.duck(false);
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

    fn start_media(&self, call: &state::ActiveCall) -> bool {
        if !self
            .inner
            .media_enabled
            .load(std::sync::atomic::Ordering::Relaxed)
        {
            return true;
        }
        let Some(session) = parse_peer(&call.peer_id)
            .ok()
            .and_then(|node_id| self.inner.transport.session_by_node_id(node_id))
        else {
            tracing::warn!(peer_id = %call.peer_id, "call media session is unavailable");
            return false;
        };
        let settings = self.settings();
        match MediaSession::start(
            call.id,
            session,
            settings.call_input_device(),
            settings.call_output_device(),
        ) {
            Ok(media) => {
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
                self.inner.ringtone.duck(true);
                true
            }
            Err(error) => {
                tracing::warn!(%error, "failed to start call audio");
                false
            }
        }
    }

    fn ensure_active(&self, call_id: CallId, peer_id: &str) -> Result<(), CallError> {
        let registry = self
            .inner
            .registry
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        match registry.active() {
            Some(call) if call.id == call_id && call.peer_id == peer_id => Ok(()),
            Some(_) => Err(CallError::InvalidState),
            None => Err(CallError::UnknownCall(call_id)),
        }
    }

    async fn peer(&self, peer_id: &str) -> Result<crate::storage::Peer, CallError> {
        self.inner
            .trust
            .peer(peer_id)
            .await?
            .ok_or_else(|| CallError::UnknownPeer(peer_id.to_owned()))
    }

    fn spawn_timeout(&self, call_id: CallId, peer_id: String) {
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
                tracing::debug!(peer_id, "call timed out");
            }
        });
    }

    async fn send_signal_for_peer(
        &self,
        peer_id: &str,
        frame: CallSignalFrame,
    ) -> Result<(), CallError> {
        let peer_id = parse_peer(peer_id)?;
        let session = self
            .inner
            .transport
            .session_by_node_id(peer_id)
            .ok_or(CallError::PeerOffline)?;
        self.send_signal(&session, Endpoint::Peer, &frame).await
    }

    async fn send_signal_frame(
        &self,
        peer: &PeerInfo,
        frame: CallSignalFrame,
    ) -> Result<(), CallError> {
        let Some(session) = self
            .inner
            .transport
            .session_by_fingerprint(&peer.fingerprint)
        else {
            return Err(CallError::PeerOffline);
        };
        self.send_signal(&session, Endpoint::Peer, &frame).await
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
    Peer,
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
                    if let Err(error) = service.handle_signal(signal.peer, signal.frame).await {
                        tracing::debug!(%error, "call signal rejected");
                    }
                }
                packet = audio.recv() => {
                    let Some(packet) = packet else { break };
                    if let Err(error) = service.handle_audio(packet.peer, &packet.payload) {
                        tracing::debug!(%error, "call audio rejected");
                    }
                }
                event = transport_events.recv() => {
                    match event {
                        Ok(ticonnect_net::transport::TransportEvent::PeerDisconnected(peer)) => {
                            service.handle_disconnect(&peer);
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

fn parse_peer(peer_id: &str) -> Result<NodeId, CallError> {
    peer_id
        .parse()
        .map_err(|_| CallError::InvalidPeer(peer_id.to_owned()))
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
    Ringtone(#[from] RingtoneError),

    #[error(transparent)]
    Media(#[from] MediaError),

    #[error(transparent)]
    Signal(#[from] ticonnect_core::call::CallError),

    #[error("unknown peer: {0}")]
    UnknownPeer(String),

    #[error("invalid peer id: {0}")]
    InvalidPeer(String),

    #[error("call was rejected by policy")]
    Rejected,

    #[error("peer is offline")]
    PeerOffline,

    #[error("unknown call: {0}")]
    UnknownCall(CallId),

    #[error("call state is invalid")]
    InvalidState,

    #[error("call audio devices are unavailable")]
    MediaUnavailable,

    #[error("call protocol error: {0}")]
    Protocol(String),
}
