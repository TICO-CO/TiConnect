pub mod consent;
pub mod session;
pub mod state;

use std::sync::{Arc, Mutex, PoisonError, RwLock};

use thiserror::Error;
use ticonnect_core::remote::{InputKind, InputPacket, RemoteSignalFrame};
use ticonnect_desktop::input::{InputEvent, InputRouter};
use ticonnect_net::transport::{PeerInfo, Transport, TransportError};
use ticonnect_settings::Settings;
use tokio::task::JoinHandle;

use crate::messaging::now_unix_ms;
use crate::storage::PeerPolicy;
use crate::trust::{TrustError, TrustStore};

use consent::{generate_pin, verify_pin};
use state::{
    RemoteEvent, RemoteRole, RemoteSession, RemoteSessionId, RemoteState, RemoteStateError,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteSnapshot {
    pub session_id: String,
    pub peer_id: String,
    pub fingerprint: String,
    pub role: String,
    pub state: String,
    pub pin: String,
    pub indicator: bool,
    pub expires_at: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RemoteEventMessage {
    Requested(RemoteSnapshot),
    Started(RemoteSnapshot),
    Ended { session_id: String, reason: String },
    Indicator { session_id: String, visible: bool },
}

#[derive(Debug)]
pub struct InboundRemoteFrame {
    pub peer: PeerInfo,
    pub frame: ticonnect_core::wire::Frame,
}

pub trait RemoteHost: Send + Sync {
    fn start(&self, width: u32, height: u32) -> Result<(), RemoteError>;
    fn stop(&self);
    fn inject(&self, event: InputEvent) -> Result<(), RemoteError>;
}

#[derive(Debug, Default)]
pub struct NoopRemoteHost;

impl RemoteHost for NoopRemoteHost {
    fn start(&self, _width: u32, _height: u32) -> Result<(), RemoteError> {
        Ok(())
    }

    fn stop(&self) {}

    fn inject(&self, event: InputEvent) -> Result<(), RemoteError> {
        event
            .validate()
            .map_err(|error| RemoteError::Input(error.to_string()))
    }
}

pub struct DesktopRemoteHost {
    router: Mutex<Option<InputRouter>>,
}

impl Default for DesktopRemoteHost {
    fn default() -> Self {
        Self {
            router: Mutex::new(None),
        }
    }
}

impl RemoteHost for DesktopRemoteHost {
    fn start(&self, _width: u32, _height: u32) -> Result<(), RemoteError> {
        Ok(())
    }

    fn stop(&self) {
        if let Ok(mut router) = self.router.lock()
            && let Some(router) = router.as_mut()
        {
            router.release();
        }
        if let Ok(mut router) = self.router.lock() {
            *router = None;
        }
    }

    fn inject(&self, event: InputEvent) -> Result<(), RemoteError> {
        let mut router = self.router.lock().unwrap_or_else(PoisonError::into_inner);
        if router.is_none() {
            let backend = ticonnect_desktop::input::xtest::XtestBackend::connect()
                .map_err(|error| RemoteError::Input(error.to_string()))?;
            *router = Some(InputRouter::new(backend));
        }
        router
            .as_mut()
            .expect("router")
            .inject(event)
            .map_err(|error| RemoteError::Input(error.to_string()))
    }
}

#[derive(Clone)]
pub struct RemoteService {
    inner: Arc<Inner>,
}

struct Inner {
    trust: Arc<TrustStore>,
    transport: Arc<Transport>,
    settings: RwLock<Settings>,
    events: tokio::sync::broadcast::Sender<RemoteEventMessage>,
    session: Mutex<Option<RemoteSession>>,
    host: Arc<dyn RemoteHost>,
}

impl RemoteService {
    pub fn new(
        trust: Arc<TrustStore>,
        transport: Arc<Transport>,
        settings: Settings,
        host: Arc<dyn RemoteHost>,
    ) -> Self {
        let (events, _) = tokio::sync::broadcast::channel(64);
        Self {
            inner: Arc::new(Inner {
                trust,
                transport,
                settings: RwLock::new(settings),
                events,
                session: Mutex::new(None),
                host,
            }),
        }
    }

    pub fn subscribe(&self) -> tokio::sync::broadcast::Receiver<RemoteEventMessage> {
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

    pub fn snapshot(&self) -> Option<RemoteSnapshot> {
        self.inner
            .session
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .as_ref()
            .map(snapshot)
    }

    pub async fn request_session(&self, peer_id: &str) -> Result<RemoteSnapshot, RemoteError> {
        let peer = self
            .inner
            .trust
            .peer(peer_id)
            .await?
            .ok_or_else(|| RemoteError::UnknownPeer(peer_id.to_owned()))?;
        if peer.policy == PeerPolicy::Ignore {
            return Err(RemoteError::Rejected);
        }
        let peer_node = peer_id
            .parse()
            .map_err(|_| RemoteError::InvalidPeer(peer_id.to_owned()))?;
        if self.inner.transport.session_by_node_id(peer_node).is_none() {
            return Err(RemoteError::PeerOffline);
        }
        let settings = self.settings();
        let now = now_unix_ms();
        let mut session = RemoteSession {
            id: RemoteSessionId::generate(),
            peer_id: peer_id.to_owned(),
            fingerprint: peer.fingerprint.clone(),
            role: RemoteRole::Viewer,
            state: RemoteState::Idle,
            pin: None,
            attempts: 0,
            max_attempts: settings.remote_pin_attempts(),
            expires_at: now.saturating_add(i64::from(settings.remote_pin_ttl_secs()) * 1000),
            indicator_visible: false,
        };
        session.transition(RemoteEvent::Request)?;
        let frame = RemoteSignalFrame::Request {
            session_id: session.id.as_bytes(),
        };
        self.send_signal(peer_node, &frame).await?;
        let snapshot = snapshot(&session);
        *self
            .inner
            .session
            .lock()
            .unwrap_or_else(PoisonError::into_inner) = Some(session);
        self.emit(RemoteEventMessage::Requested(snapshot.clone()));
        Ok(snapshot)
    }

    pub async fn accept_session(
        &self,
        session_id: &str,
        pin: &str,
    ) -> Result<RemoteSnapshot, RemoteError> {
        let id: RemoteSessionId = session_id.parse()?;
        let (role, snapshot, signal) = {
            let mut guard = self
                .inner
                .session
                .lock()
                .unwrap_or_else(PoisonError::into_inner);
            let session = guard
                .as_mut()
                .filter(|session| session.id == id)
                .ok_or(RemoteError::State(RemoteStateError::InvalidSession))?;
            match session.role {
                RemoteRole::Host => {
                    if session.state != RemoteState::Requested {
                        return Err(RemoteError::State(RemoteStateError::InvalidTransition));
                    }
                    let generated = session.pin.clone().unwrap_or_else(generate_pin);
                    session.pin = Some(generated.clone());
                    session.transition(RemoteEvent::Consent)?;
                    let snapshot = snapshot(session);
                    (
                        RemoteRole::Host,
                        snapshot,
                        Some(RemoteSignalFrame::Consent {
                            session_id: id_bytes(id),
                        }),
                    )
                }
                RemoteRole::Viewer => {
                    if session.state != RemoteState::Requested
                        && session.state != RemoteState::Consented
                    {
                        return Err(RemoteError::State(RemoteStateError::InvalidTransition));
                    }
                    session.pin = Some(pin.to_owned());
                    let snapshot = snapshot(session);
                    (
                        RemoteRole::Viewer,
                        snapshot,
                        Some(RemoteSignalFrame::Pin {
                            session_id: id_bytes(id),
                            pin: pin.to_owned(),
                        }),
                    )
                }
            }
        };
        let peer_node = snapshot
            .peer_id
            .parse()
            .map_err(|_| RemoteError::InvalidPeer(snapshot.peer_id.clone()))?;
        if let Some(signal) = signal {
            self.send_signal(peer_node, &signal).await?;
        }
        self.emit(RemoteEventMessage::Requested(snapshot.clone()));
        let _ = role;
        Ok(snapshot)
    }

    pub async fn deny_session(&self, session_id: &str) -> Result<(), RemoteError> {
        let id: RemoteSessionId = session_id.parse()?;
        let snapshot = {
            let mut guard = self
                .inner
                .session
                .lock()
                .unwrap_or_else(PoisonError::into_inner);
            let session = guard
                .as_mut()
                .filter(|session| session.id == id)
                .ok_or(RemoteError::State(RemoteStateError::InvalidSession))?;
            session.transition(RemoteEvent::Deny)?;
            snapshot(session)
        };
        let peer_node = snapshot
            .peer_id
            .parse()
            .map_err(|_| RemoteError::InvalidPeer(snapshot.peer_id.clone()))?;
        let _ = self
            .send_signal(
                peer_node,
                &RemoteSignalFrame::Deny {
                    session_id: id_bytes(id),
                },
            )
            .await;
        self.cleanup();
        self.emit(RemoteEventMessage::Ended {
            session_id: snapshot.session_id,
            reason: "denied".to_owned(),
        });
        Ok(())
    }

    pub async fn end_session(&self, session_id: &str) -> Result<(), RemoteError> {
        let id: RemoteSessionId = session_id.parse()?;
        let snapshot = {
            let mut guard = self
                .inner
                .session
                .lock()
                .unwrap_or_else(PoisonError::into_inner);
            let session = guard
                .as_mut()
                .filter(|session| session.id == id)
                .ok_or(RemoteError::State(RemoteStateError::InvalidSession))?;
            session.transition(RemoteEvent::KillSwitch)?;
            snapshot(session)
        };
        let peer_node = snapshot
            .peer_id
            .parse()
            .map_err(|_| RemoteError::InvalidPeer(snapshot.peer_id.clone()))?;
        let _ = self
            .send_signal(
                peer_node,
                &RemoteSignalFrame::KillSwitch {
                    session_id: id_bytes(id),
                },
            )
            .await;
        self.cleanup();
        self.emit(RemoteEventMessage::Ended {
            session_id: snapshot.session_id,
            reason: "kill-switch".to_owned(),
        });
        Ok(())
    }

    pub async fn handle_signal(
        &self,
        peer: PeerInfo,
        frame: RemoteSignalFrame,
    ) -> Result<(), RemoteError> {
        let peer_id = peer.node_id.to_string();
        let stored = self
            .inner
            .trust
            .peer_by_fingerprint(&peer.fingerprint.to_hex())
            .await?
            .ok_or_else(|| RemoteError::UnknownPeer(peer_id.clone()))?;
        if stored.policy == PeerPolicy::Ignore {
            return Ok(());
        }
        let id = RemoteSessionId::from_bytes(frame.session_id());
        match frame {
            RemoteSignalFrame::Request { .. } => {
                if self
                    .inner
                    .session
                    .lock()
                    .unwrap_or_else(PoisonError::into_inner)
                    .is_some()
                {
                    let _ = self
                        .send_signal(
                            peer.node_id,
                            &RemoteSignalFrame::Deny {
                                session_id: id_bytes(id),
                            },
                        )
                        .await;
                    return Ok(());
                }
                let settings = self.settings();
                let mut session = RemoteSession {
                    id,
                    peer_id: peer_id.clone(),
                    fingerprint: peer.fingerprint.to_hex(),
                    role: RemoteRole::Host,
                    state: RemoteState::Idle,
                    pin: Some(generate_pin()),
                    attempts: 0,
                    max_attempts: settings.remote_pin_attempts(),
                    expires_at: now_unix_ms()
                        .saturating_add(i64::from(settings.remote_pin_ttl_secs()) * 1000),
                    indicator_visible: false,
                };
                session.transition(RemoteEvent::Request)?;
                let snapshot = snapshot(&session);
                *self
                    .inner
                    .session
                    .lock()
                    .unwrap_or_else(PoisonError::into_inner) = Some(session);
                self.emit(RemoteEventMessage::Requested(snapshot));
                Ok(())
            }
            RemoteSignalFrame::Consent { .. } => {
                let mut guard = self
                    .inner
                    .session
                    .lock()
                    .unwrap_or_else(PoisonError::into_inner);
                let session = guard
                    .as_mut()
                    .filter(|session| session.id == id && session.peer_id == peer_id)
                    .ok_or(RemoteError::State(RemoteStateError::InvalidSession))?;
                if session.state == RemoteState::Requested {
                    session.transition(RemoteEvent::Consent)?;
                    let snapshot = snapshot(session);
                    drop(guard);
                    self.emit(RemoteEventMessage::Requested(snapshot));
                }
                Ok(())
            }
            RemoteSignalFrame::Pin { pin, .. } => {
                let (verified, snapshot, denied) = {
                    let mut guard = self
                        .inner
                        .session
                        .lock()
                        .unwrap_or_else(PoisonError::into_inner);
                    let session = guard
                        .as_mut()
                        .filter(|session| session.id == id && session.peer_id == peer_id)
                        .ok_or(RemoteError::State(RemoteStateError::InvalidSession))?;
                    let verified = verify_pin(session, &pin)?;
                    let snapshot = snapshot(session);
                    (verified.state, snapshot, verified.pin_failed)
                };
                if verified == RemoteState::Active {
                    self.inner.host.start(1920, 1080)?;
                    let _ = self
                        .send_signal(
                            peer.node_id,
                            &RemoteSignalFrame::Start {
                                session_id: id_bytes(id),
                            },
                        )
                        .await;
                    self.emit(RemoteEventMessage::Started(snapshot.clone()));
                    self.emit(RemoteEventMessage::Indicator {
                        session_id: snapshot.session_id,
                        visible: true,
                    });
                } else if denied {
                    let _ = self
                        .send_signal(
                            peer.node_id,
                            &RemoteSignalFrame::Deny {
                                session_id: id_bytes(id),
                            },
                        )
                        .await;
                    self.cleanup();
                    self.emit(RemoteEventMessage::Ended {
                        session_id: snapshot.session_id,
                        reason: "pin".to_owned(),
                    });
                }
                Ok(())
            }
            RemoteSignalFrame::Deny { .. } => {
                self.finish_remote(&peer_id, id, "denied").await;
                Ok(())
            }
            RemoteSignalFrame::Start { .. } => {
                let snapshot = {
                    let mut guard = self
                        .inner
                        .session
                        .lock()
                        .unwrap_or_else(PoisonError::into_inner);
                    let session = guard
                        .as_mut()
                        .filter(|session| session.id == id && session.peer_id == peer_id)
                        .ok_or(RemoteError::State(RemoteStateError::InvalidSession))?;
                    if session.state == RemoteState::Requested {
                        session.transition(RemoteEvent::Consent)?;
                    }
                    session.transition(RemoteEvent::PinOk)?;
                    snapshot(session)
                };
                self.emit(RemoteEventMessage::Started(snapshot.clone()));
                self.emit(RemoteEventMessage::Indicator {
                    session_id: snapshot.session_id,
                    visible: true,
                });
                Ok(())
            }
            RemoteSignalFrame::Hangup { .. } | RemoteSignalFrame::KillSwitch { .. } => {
                self.finish_remote(&peer_id, id, "hangup").await;
                Ok(())
            }
            RemoteSignalFrame::KeyframeRequest { .. } => Ok(()),
        }
    }

    pub fn inject_input(&self, peer: PeerInfo, packet: InputPacket) -> Result<(), RemoteError> {
        let peer_id = peer.node_id.to_string();
        let guard = self
            .inner
            .session
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        let session = guard
            .as_ref()
            .filter(|session| {
                session.peer_id == peer_id
                    && session.state == RemoteState::Active
                    && session.role == RemoteRole::Host
            })
            .ok_or(RemoteError::Inactive)?;
        if packet.session_id != id_bytes(session.id) {
            return Err(RemoteError::Inactive);
        }
        let event = match packet.kind {
            InputKind::Motion => InputEvent::PointerMotion {
                x: packet.x,
                y: packet.y,
            },
            InputKind::Button => InputEvent::PointerButton {
                button: u8::try_from(packet.x).unwrap_or(u8::MAX),
                pressed: packet.pressed,
            },
            InputKind::Axis => InputEvent::PointerAxis {
                horizontal: packet.x,
                vertical: packet.y,
            },
            InputKind::Key => InputEvent::Key {
                keysym: u32::try_from(packet.x).unwrap_or_default(),
                pressed: packet.pressed,
            },
        };
        self.inner.host.inject(event)
    }

    pub fn handle_disconnect(&self, peer: &PeerInfo) {
        let peer_id = peer.node_id.to_string();
        let ended = {
            let mut guard = self
                .inner
                .session
                .lock()
                .unwrap_or_else(PoisonError::into_inner);
            if let Some(session) = guard.as_mut()
                && session.peer_id == peer_id
                && matches!(
                    session.state,
                    RemoteState::Active | RemoteState::Consented | RemoteState::Requested
                )
            {
                let _ = session.transition(RemoteEvent::Disconnect);
                Some(session.id)
            } else {
                None
            }
        };
        if let Some(id) = ended {
            self.cleanup();
            self.emit(RemoteEventMessage::Ended {
                session_id: id.to_string(),
                reason: "disconnected".to_owned(),
            });
        }
    }

    async fn finish_remote(&self, peer_id: &str, id: RemoteSessionId, reason: &str) {
        let matches = self
            .inner
            .session
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .as_ref()
            .is_some_and(|session| session.id == id && session.peer_id == peer_id);
        if !matches {
            return;
        }
        self.cleanup();
        self.emit(RemoteEventMessage::Ended {
            session_id: id.to_string(),
            reason: reason.to_owned(),
        });
    }

    fn cleanup(&self) {
        self.inner.host.stop();
        if let Some(session) = self
            .inner
            .session
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .as_mut()
        {
            session.indicator_visible = false;
            let _ = session.transition(RemoteEvent::CleanupDone);
        }
        *self
            .inner
            .session
            .lock()
            .unwrap_or_else(PoisonError::into_inner) = None;
    }

    async fn send_signal(
        &self,
        peer_id: ticonnect_core::id::NodeId,
        frame: &RemoteSignalFrame,
    ) -> Result<(), RemoteError> {
        let session = self
            .inner
            .transport
            .session_by_node_id(peer_id)
            .ok_or(RemoteError::PeerOffline)?;
        let mut send = session.open_uni().await?;
        ticonnect_net::transport::session::write_frame(&mut send, &frame.to_wire()?).await?;
        Ok(())
    }

    fn emit(&self, event: RemoteEventMessage) {
        let _ = self.inner.events.send(event);
    }
}

pub fn spawn(
    service: RemoteService,
    mut frames: tokio::sync::mpsc::UnboundedReceiver<InboundRemoteFrame>,
) -> JoinHandle<()> {
    let mut transport_events = service.inner.transport.subscribe();
    tokio::spawn(async move {
        loop {
            tokio::select! {
                frame = frames.recv() => {
                    let Some(frame) = frame else { break };
                    let result = match frame.frame.kind() {
                        ticonnect_core::wire::FrameKind::Remote => {
                            match ticonnect_core::remote::RemoteSignalFrame::from_wire(&frame.frame) {
                                Ok(signal) => service.handle_signal(frame.peer, signal).await,
                                Err(error) => Err(RemoteError::Protocol(error.to_string())),
                            }
                        }
                        ticonnect_core::wire::FrameKind::Input => {
                            match ticonnect_core::remote::InputPacket::decode(frame.frame.payload()) {
                                Ok(packet) => service.inject_input(frame.peer, packet),
                                Err(error) => Err(RemoteError::Protocol(error.to_string())),
                            }
                        }
                        _ => Ok(()),
                    };
                    if let Err(error) = result {
                        tracing::debug!(%error, "remote frame rejected");
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

fn id_bytes(id: RemoteSessionId) -> [u8; 16] {
    id.as_bytes()
}

fn snapshot(session: &RemoteSession) -> RemoteSnapshot {
    RemoteSnapshot {
        session_id: session.id.to_string(),
        peer_id: session.peer_id.clone(),
        fingerprint: session.fingerprint.clone(),
        role: match session.role {
            RemoteRole::Host => "host",
            RemoteRole::Viewer => "viewer",
        }
        .to_owned(),
        state: session.state.as_str().to_owned(),
        pin: session.pin.clone().unwrap_or_default(),
        indicator: session.indicator_visible,
        expires_at: session.expires_at,
    }
}

#[derive(Debug, Error)]
pub enum RemoteError {
    #[error(transparent)]
    State(#[from] RemoteStateError),

    #[error(transparent)]
    Trust(#[from] TrustError),

    #[error(transparent)]
    Transport(#[from] TransportError),

    #[error(transparent)]
    Signal(#[from] ticonnect_core::remote::RemoteError),

    #[error("unknown peer: {0}")]
    UnknownPeer(String),

    #[error("invalid peer id: {0}")]
    InvalidPeer(String),

    #[error("peer is offline")]
    PeerOffline,

    #[error("remote access was rejected by policy")]
    Rejected,

    #[error("remote session is not active")]
    Inactive,

    #[error("remote host failed: {0}")]
    Host(String),

    #[error("input failed: {0}")]
    Input(String),

    #[error("remote protocol error: {0}")]
    Protocol(String),
}
