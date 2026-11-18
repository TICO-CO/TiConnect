pub mod session;
pub mod state;

use std::sync::{Arc, Mutex, PoisonError, RwLock};

use thiserror::Error;
use ticonnect_core::remote::{InputKind, InputPacket, RemoteSignalFrame};
use ticonnect_desktop::input::{InputEvent, InputRouter};
use ticonnect_net::transport::{Transport, TransportError, UserInfo};
use ticonnect_settings::Settings;
use tokio::task::JoinHandle;

use crate::messaging::now_unix_ms;
use crate::storage::UserPolicy;
use crate::trust::{TrustError, TrustStore};

use state::{
    RemoteEvent, RemoteRole, RemoteSession, RemoteSessionId, RemoteState, RemoteStateError,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteSnapshot {
    pub session_id: String,
    pub user_id: String,
    pub fingerprint: String,
    pub role: String,
    pub state: String,
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
    pub user: UserInfo,
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
    portal_stop: Mutex<Option<std::sync::mpsc::Sender<()>>>,
}

impl Default for DesktopRemoteHost {
    fn default() -> Self {
        Self {
            router: Mutex::new(None),
            portal_stop: Mutex::new(None),
        }
    }
}

impl RemoteHost for DesktopRemoteHost {
    fn start(&self, width: u32, height: u32) -> Result<(), RemoteError> {
        let (sender, receiver) = std::sync::mpsc::channel();
        *self
            .portal_stop
            .lock()
            .unwrap_or_else(PoisonError::into_inner) = Some(sender);
        std::thread::Builder::new()
            .name("ticonnect-portal".to_owned())
            .spawn(move || {
                let runtime = match tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                {
                    Ok(runtime) => runtime,
                    Err(error) => {
                        tracing::warn!(%error, "failed to create portal runtime");
                        return;
                    }
                };
                runtime.block_on(async move {
                    match ticonnect_desktop::portal::PortalSession::open().await {
                        Ok(mut portal) => {
                            let config = ticonnect_desktop::capture::CaptureConfig {
                                width: width.min(1920),
                                height: height.min(1080),
                                fps: 30,
                            };
                            let capture = ticonnect_desktop::capture::PortalCapture::start(
                                &mut portal,
                                config,
                            );
                            if let Err(error) = &capture {
                                tracing::warn!(%error, "portal capture did not start");
                            }
                            tracing::debug!("remote desktop portal session started");
                            let _ = tokio::task::spawn_blocking(move || receiver.recv()).await;
                            drop(capture);
                            if let Err(error) = portal.close().await {
                                tracing::debug!(%error, "portal session close failed");
                            }
                            tracing::debug!("remote desktop portal session closed");
                        }
                        Err(error) => tracing::warn!(%error, "remote desktop portal denied"),
                    }
                });
            })
            .map_err(|error| RemoteError::Host(error.to_string()))?;
        Ok(())
    }

    fn stop(&self) {
        if let Some(sender) = self
            .portal_stop
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .take()
        {
            let _ = sender.send(());
        }
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
    clock: Arc<dyn Fn() -> i64 + Send + Sync>,
}

impl RemoteService {
    pub fn new(
        trust: Arc<TrustStore>,
        transport: Arc<Transport>,
        settings: Settings,
        host: Arc<dyn RemoteHost>,
    ) -> Self {
        Self::with_clock(trust, transport, settings, host, Arc::new(now_unix_ms))
    }

    pub fn with_clock(
        trust: Arc<TrustStore>,
        transport: Arc<Transport>,
        settings: Settings,
        host: Arc<dyn RemoteHost>,
        clock: Arc<dyn Fn() -> i64 + Send + Sync>,
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
                clock,
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

    pub async fn request_session(&self, user_id: &str) -> Result<RemoteSnapshot, RemoteError> {
        let user = self
            .inner
            .trust
            .user(user_id)
            .await?
            .ok_or_else(|| RemoteError::UnknownUser(user_id.to_owned()))?;
        if user.policy == UserPolicy::Ignore {
            return Err(RemoteError::Rejected);
        }
        let user_node = user_id
            .parse()
            .map_err(|_| RemoteError::InvalidUser(user_id.to_owned()))?;
        if self.inner.transport.session_by_node_id(user_node).is_none() {
            return Err(RemoteError::UserOffline);
        }
        let settings = self.settings();
        let now = (self.inner.clock)();
        let mut session = RemoteSession {
            id: RemoteSessionId::generate(),
            user_id: user_id.to_owned(),
            fingerprint: user.fingerprint.clone(),
            role: RemoteRole::Viewer,
            state: RemoteState::Idle,
            expires_at: now
                .saturating_add(i64::from(settings.remote_request_timeout_secs()) * 1000),
            indicator_visible: false,
        };
        session.transition(RemoteEvent::Request)?;
        let frame = RemoteSignalFrame::Request {
            session_id: session.id.as_bytes(),
        };
        self.send_signal(user_node, &frame).await?;
        let snapshot = snapshot(&session);
        *self
            .inner
            .session
            .lock()
            .unwrap_or_else(PoisonError::into_inner) = Some(session);
        self.emit(RemoteEventMessage::Requested(snapshot.clone()));
        Ok(snapshot)
    }

    pub async fn accept_session(&self, session_id: &str) -> Result<RemoteSnapshot, RemoteError> {
        let id: RemoteSessionId = session_id.parse()?;
        let expired = {
            let mut guard = self
                .inner
                .session
                .lock()
                .unwrap_or_else(PoisonError::into_inner);
            let session = guard
                .as_mut()
                .filter(|session| session.id == id)
                .ok_or(RemoteError::State(RemoteStateError::InvalidSession))?;
            if (self.inner.clock)() > session.expires_at {
                Some((snapshot(session), session.role, session.user_id.clone()))
            } else {
                None
            }
        };
        if let Some((snapshot, role, user_id)) = expired {
            if role == RemoteRole::Host
                && let Ok(user) = user_id.parse()
            {
                let _ = self
                    .send_signal(
                        user,
                        &RemoteSignalFrame::Deny {
                            session_id: id_bytes(id),
                        },
                    )
                    .await;
            }
            self.cleanup();
            self.emit(RemoteEventMessage::Ended {
                session_id: snapshot.session_id,
                reason: "expired".to_owned(),
            });
            return Err(RemoteError::Expired);
        }
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
            if session.role != RemoteRole::Host {
                return Err(RemoteError::State(RemoteStateError::InvalidTransition));
            }
            if session.state != RemoteState::Requested {
                return Err(RemoteError::State(RemoteStateError::InvalidTransition));
            }
            session.transition(RemoteEvent::Consent)?;
            snapshot(session)
        };
        self.inner.host.start(1920, 1080)?;
        let user_node = snapshot
            .user_id
            .parse()
            .map_err(|_| RemoteError::InvalidUser(snapshot.user_id.clone()))?;
        let _ = self
            .send_signal(
                user_node,
                &RemoteSignalFrame::Start {
                    session_id: id_bytes(id),
                },
            )
            .await;
        self.emit(RemoteEventMessage::Started(snapshot.clone()));
        self.emit(RemoteEventMessage::Indicator {
            session_id: snapshot.session_id.clone(),
            visible: true,
        });
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
        let user_node = snapshot
            .user_id
            .parse()
            .map_err(|_| RemoteError::InvalidUser(snapshot.user_id.clone()))?;
        let _ = self
            .send_signal(
                user_node,
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
        let user_node = snapshot
            .user_id
            .parse()
            .map_err(|_| RemoteError::InvalidUser(snapshot.user_id.clone()))?;
        let _ = self
            .send_signal(
                user_node,
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
        user: UserInfo,
        frame: RemoteSignalFrame,
    ) -> Result<(), RemoteError> {
        let user_id = user.node_id.to_string();
        let stored = self
            .inner
            .trust
            .user_by_fingerprint(&user.fingerprint.to_hex())
            .await?
            .ok_or_else(|| RemoteError::UnknownUser(user_id.clone()))?;
        if stored.policy == UserPolicy::Ignore {
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
                            user.node_id,
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
                    user_id: user_id.clone(),
                    fingerprint: user.fingerprint.to_hex(),
                    role: RemoteRole::Host,
                    state: RemoteState::Idle,
                    expires_at: (self.inner.clock)()
                        .saturating_add(i64::from(settings.remote_request_timeout_secs()) * 1000),
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
            RemoteSignalFrame::Deny { .. } => {
                self.finish_remote(&user_id, id, "denied").await;
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
                        .filter(|session| {
                            session.id == id
                                && session.user_id == user_id
                                && session.role == RemoteRole::Viewer
                        })
                        .ok_or(RemoteError::State(RemoteStateError::InvalidSession))?;
                    if session.state != RemoteState::Requested {
                        return Err(RemoteError::State(RemoteStateError::InvalidTransition));
                    }
                    session.transition(RemoteEvent::Consent)?;
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
                self.finish_remote(&user_id, id, "hangup").await;
                Ok(())
            }
            RemoteSignalFrame::KeyframeRequest { .. } => Ok(()),
        }
    }

    pub fn inject_input(&self, user: UserInfo, packet: InputPacket) -> Result<(), RemoteError> {
        let user_id = user.node_id.to_string();
        let guard = self
            .inner
            .session
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        let session = guard
            .as_ref()
            .filter(|session| {
                session.user_id == user_id
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

    pub fn handle_disconnect(&self, user: &UserInfo) {
        let user_id = user.node_id.to_string();
        let ended = {
            let mut guard = self
                .inner
                .session
                .lock()
                .unwrap_or_else(PoisonError::into_inner);
            if let Some(session) = guard.as_mut()
                && session.user_id == user_id
                && matches!(session.state, RemoteState::Active | RemoteState::Requested)
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

    async fn finish_remote(&self, user_id: &str, id: RemoteSessionId, reason: &str) {
        let matches = self
            .inner
            .session
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .as_ref()
            .is_some_and(|session| session.id == id && session.user_id == user_id);
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
        user_id: ticonnect_core::id::NodeId,
        frame: &RemoteSignalFrame,
    ) -> Result<(), RemoteError> {
        let session = self
            .inner
            .transport
            .session_by_node_id(user_id)
            .ok_or(RemoteError::UserOffline)?;
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
                                Ok(signal) => service.handle_signal(frame.user, signal).await,
                                Err(error) => Err(RemoteError::Protocol(error.to_string())),
                            }
                        }
                        ticonnect_core::wire::FrameKind::Input => {
                            match ticonnect_core::remote::InputPacket::decode(frame.frame.payload()) {
                                Ok(packet) => service.inject_input(frame.user, packet),
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
                        Ok(ticonnect_net::transport::TransportEvent::UserDisconnected(user)) => {
                            service.handle_disconnect(&user);
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
        user_id: session.user_id.clone(),
        fingerprint: session.fingerprint.clone(),
        role: match session.role {
            RemoteRole::Host => "host",
            RemoteRole::Viewer => "viewer",
        }
        .to_owned(),
        state: session.state.as_str().to_owned(),
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

    #[error("unknown user: {0}")]
    UnknownUser(String),

    #[error("invalid user id: {0}")]
    InvalidUser(String),

    #[error("user is offline")]
    UserOffline,

    #[error("remote access was rejected by policy")]
    Rejected,

    #[error("remote session has expired")]
    Expired,

    #[error("remote session is not active")]
    Inactive,

    #[error("remote host failed: {0}")]
    Host(String),

    #[error("input failed: {0}")]
    Input(String),

    #[error("remote protocol error: {0}")]
    Protocol(String),
}
