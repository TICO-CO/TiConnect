use std::net::{IpAddr, Ipv4Addr, SocketAddr, UdpSocket};
use std::str::FromStr;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use thiserror::Error;
use ticonnect_core::fingerprint::{Fingerprint, FingerprintError};
use ticonnect_core::id::NodeId;
use ticonnect_net::discovery::{Discovery, LanDiscovery};
use ticonnect_net::pairing::{
    EndpointError, InitiatorRequest, PIN_ATTEMPTS, PairingMode, PairingPeer, PairingSecret,
    PinError, PinPolicy, QrError, QrPayload, ResponderRequest, run_initiator, run_responder,
};
use ticonnect_net::transport::{DialTarget, PeerInfo, Transport, TransportError, TransportEvent};
use tokio::sync::{Mutex, broadcast};
use tokio::task::JoinHandle;
use zbus::object_server::SignalEmitter;

use crate::identity::PublicIdentity;
use crate::service::DaemonService;
use crate::storage::{NewPeer, Peer, PeerPolicy, StorageError};
use crate::trust::{TrustError, TrustStore};

#[derive(Debug, Error)]
pub enum PairingServiceError {
    #[error("pairing session has expired")]
    Expired,

    #[error("no discovery candidate found for node {0}")]
    NoCandidate(String),

    #[error("candidate address is invalid")]
    InvalidAddress,

    #[error("candidate port is not available")]
    MissingPort,

    #[error("invalid peer identity: {0}")]
    InvalidIdentity(String),

    #[error("trusted peer was not found")]
    NotFound,

    #[error(transparent)]
    Pin(#[from] PinError),

    #[error(transparent)]
    Qr(#[from] QrError),

    #[error(transparent)]
    Transport(#[from] TransportError),

    #[error(transparent)]
    Endpoint(#[from] EndpointError),

    #[error(transparent)]
    Trust(#[from] TrustError),

    #[error(transparent)]
    Storage(#[from] StorageError),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StartPairingResponse {
    pub pin: String,
    pub qr: String,
    pub expires_at: u64,
    pub node_id: String,
    pub port: u16,
}

#[derive(Debug, Clone)]
pub enum PairingEvent {
    Started(StartPairingResponse),
    Completed {
        peer_id: String,
        fingerprint: String,
    },
    Failed(String),
}

struct ActivePairing {
    pin: PinPolicy,
    token: [u8; 32],
    expires_at: u64,
    connection: Option<quinn::Connection>,
    failures: u8,
}

pub struct PairingService {
    identity: PublicIdentity,
    trust: Arc<TrustStore>,
    transport: Arc<Transport>,
    discovery: Arc<LanDiscovery>,
    active: Mutex<Option<ActivePairing>>,
    events: broadcast::Sender<PairingEvent>,
}

impl PairingService {
    pub fn new(
        identity: PublicIdentity,
        trust: Arc<TrustStore>,
        transport: Arc<Transport>,
        discovery: Arc<LanDiscovery>,
    ) -> Self {
        let (events, _receiver) = broadcast::channel(32);
        Self {
            identity,
            trust,
            transport,
            discovery,
            active: Mutex::new(None),
            events,
        }
    }

    pub fn subscribe(&self) -> broadcast::Receiver<PairingEvent> {
        self.events.subscribe()
    }

    pub async fn start_pairing(&self) -> Result<StartPairingResponse, PairingServiceError> {
        let port = self.transport.local_addr()?.port();
        let address = local_ipv4();
        let pin = PinPolicy::generate();
        let qr = QrPayload::generate(
            self.identity.node_id(),
            self.identity.fingerprint(),
            IpAddr::V4(address),
            port,
        )?;
        let token = *qr.token();
        let expires_at = qr.expires_at_unix();
        let response = StartPairingResponse {
            pin: pin.value().to_owned(),
            qr: qr.encode(),
            expires_at,
            node_id: self.identity.node_id().to_string(),
            port,
        };

        let previous = self.active.lock().await.replace(ActivePairing {
            pin,
            token,
            expires_at,
            connection: None,
            failures: 0,
        });
        if let Some(previous) = previous
            && let Some(connection) = previous.connection
        {
            connection.close(0u32.into(), b"pairing restarted");
        }
        let _ = self.events.send(PairingEvent::Started(response.clone()));
        Ok(response)
    }

    pub async fn pair_with_pin(
        &self,
        node_id: &str,
        pin: &str,
    ) -> Result<Peer, PairingServiceError> {
        PinPolicy::from_value(pin)?;
        let node_id = NodeId::from_str(node_id)
            .map_err(|error| PairingServiceError::InvalidIdentity(error.to_string()))?;
        let candidate = self
            .discovery
            .snapshot()
            .into_iter()
            .find(|candidate| candidate.node_id() == node_id.to_string())
            .ok_or_else(|| PairingServiceError::NoCandidate(node_id.to_string()))?;
        let address = candidate
            .addresses()
            .next()
            .ok_or(PairingServiceError::InvalidAddress)?
            .parse::<IpAddr>()
            .map_err(|_| PairingServiceError::InvalidAddress)?;
        if candidate.port() == 0 {
            return Err(PairingServiceError::MissingPort);
        }
        let fingerprint =
            Fingerprint::from_str(candidate.fingerprint()).map_err(fingerprint_error)?;
        let expected = PairingPeer {
            node_id,
            fingerprint,
            certificate_fingerprint: fingerprint,
        };
        let display_name =
            (!candidate.display_name().is_empty()).then(|| candidate.display_name().to_owned());

        self.pair_as_initiator(
            SocketAddr::new(address, candidate.port()),
            expected,
            PairingMode::Pin,
            PairingSecret::Pin(pin.to_owned()),
            None,
            display_name,
        )
        .await
    }

    pub async fn pair_with_qr(&self, payload: &str) -> Result<Peer, PairingServiceError> {
        let payload = QrPayload::decode(payload)?;
        if payload.is_expired(now_unix()) {
            return Err(PairingServiceError::Expired);
        }
        let expected = PairingPeer {
            node_id: payload.node_id(),
            fingerprint: payload.fingerprint(),
            certificate_fingerprint: payload.fingerprint(),
        };
        let token = *payload.token();
        self.pair_as_initiator(
            SocketAddr::new(payload.address(), payload.port()),
            expected,
            PairingMode::Qr,
            PairingSecret::Token(token),
            Some(token),
            None,
        )
        .await
    }

    async fn pair_as_initiator(
        &self,
        address: SocketAddr,
        expected: PairingPeer,
        mode: PairingMode,
        secret: PairingSecret,
        token: Option<[u8; 32]>,
        display_name: Option<String>,
    ) -> Result<Peer, PairingServiceError> {
        let local = self.local_peer();
        let target = DialTarget::new(address, expected.fingerprint).with_node_id(expected.node_id);
        let connection = self
            .transport
            .connect_pairing_with_retry(target.clone())
            .await?;
        let outcome = run_initiator(
            &connection,
            InitiatorRequest {
                local,
                expected: expected.clone(),
                mode,
                secret,
                token,
            },
        )
        .await;
        connection.close(0u32.into(), b"paired");
        let outcome = outcome?;
        let peer = self.commit_peer(outcome.peer, display_name).await?;
        if let Err(error) = self.transport.connect(target).await {
            tracing::warn!(%error, "failed to open session after pairing");
        }
        Ok(peer)
    }

    pub async fn cancel_pairing(&self) {
        let active = self.active.lock().await.take();
        if let Some(active) = active
            && let Some(connection) = active.connection
        {
            connection.close(0u32.into(), b"pairing cancelled");
        }
    }

    pub async fn forget_peer(&self, peer_id: &str) -> Result<Peer, PairingServiceError> {
        let peer = self.trust.forget(peer_id).await?;
        for session in self.transport.sessions() {
            if session.peer().fingerprint.to_hex() == peer.fingerprint {
                session.close(0, b"forgotten");
            }
        }
        Ok(peer)
    }

    pub async fn list_trusted_peers(&self) -> Result<Vec<Peer>, PairingServiceError> {
        self.trust.list().await.map_err(Into::into)
    }

    async fn handle_pairing_request(
        &self,
        connection: quinn::Connection,
        fingerprint: Fingerprint,
    ) {
        let (pin, token) = {
            let mut active = self.active.lock().await;
            let Some(session) = active.as_mut() else {
                connection.close(0u32.into(), b"no pairing session");
                return;
            };
            if session.expires_at <= now_unix() {
                *active = None;
                connection.close(0u32.into(), b"pairing expired");
                return;
            }
            session.connection = Some(connection.clone());
            (session.pin.value().to_owned(), session.token)
        };

        let result = run_responder(
            &connection,
            ResponderRequest {
                local: self.local_peer(),
                pin_secret: Some(PairingSecret::Pin(pin)),
                qr_secret: Some(PairingSecret::Token(token)),
                token: Some(token),
                certificate_fingerprint: fingerprint,
            },
        )
        .await;

        match result {
            Ok(outcome) => {
                *self.active.lock().await = None;
                let peer = outcome.peer;
                let target = DialTarget::new(connection.remote_address(), peer.fingerprint)
                    .with_node_id(peer.node_id);
                match self.commit_peer(peer, None).await {
                    Ok(_) => {
                        if let Err(error) = self.transport.connect(target).await {
                            tracing::warn!(%error, "failed to open session after pairing");
                        }
                    }
                    Err(error) => {
                        tracing::warn!(%error, "failed to commit paired peer");
                        let _ = self.events.send(PairingEvent::Failed(error.to_string()));
                    }
                }
            }
            Err(error) => {
                let mut active = self.active.lock().await;
                if let Some(session) = active.as_mut() {
                    session.connection = None;
                    session.failures = session.failures.saturating_add(1);
                    if session.failures >= PIN_ATTEMPTS {
                        *active = None;
                    }
                }
                drop(active);
                let _ = self.events.send(PairingEvent::Failed(error.to_string()));
                tracing::warn!(%error, "pairing failed");
            }
        }
    }

    async fn commit_peer(
        &self,
        peer: PairingPeer,
        display_name: Option<String>,
    ) -> Result<Peer, PairingServiceError> {
        let new_peer = NewPeer {
            id: peer.node_id.to_string(),
            fingerprint: peer.fingerprint.to_hex(),
            display_name,
            policy: PeerPolicy::Accept,
            trusted_at: i64::try_from(now_unix()).unwrap_or(i64::MAX),
            last_seen: None,
        };

        match self.trust.commit(new_peer).await {
            Ok(peer) => {
                let _ = self.events.send(PairingEvent::Completed {
                    peer_id: peer.id.clone(),
                    fingerprint: peer.fingerprint.clone(),
                });
                Ok(peer)
            }
            Err(TrustError::Storage(StorageError::DuplicateFingerprint(_))) => {
                let existing = self
                    .trust
                    .list()
                    .await
                    .map_err(PairingServiceError::from)?
                    .into_iter()
                    .find(|existing| existing.fingerprint == peer.fingerprint.to_hex())
                    .ok_or(PairingServiceError::NotFound)?;
                let _ = self.events.send(PairingEvent::Completed {
                    peer_id: existing.id.clone(),
                    fingerprint: existing.fingerprint.clone(),
                });
                Ok(existing)
            }
            Err(error) => Err(error.into()),
        }
    }

    fn local_peer(&self) -> PairingPeer {
        PairingPeer {
            node_id: self.identity.node_id(),
            fingerprint: self.identity.fingerprint(),
            certificate_fingerprint: self.identity.fingerprint(),
        }
    }
}

pub fn spawn(connection: zbus::Connection, service: Arc<PairingService>) -> JoinHandle<()> {
    let transport = service.transport.clone();
    let mut transport_events = transport.subscribe();
    let mut pairing_events = service.subscribe();

    tokio::spawn(async move {
        loop {
            tokio::select! {
                event = transport_events.recv() => {
                    match event {
                        Ok(TransportEvent::PairingRequested { fingerprint, connection: pairing_connection, .. }) => {
                            let service = service.clone();
                            tokio::spawn(async move {
                                service.handle_pairing_request(pairing_connection, fingerprint).await;
                            });
                        }
                        Ok(TransportEvent::PeerConnected(peer)) => {
                            let emitter = match SignalEmitter::new(&connection, ticonnect_core::ipc::DAEMON_PATH) {
                                Ok(emitter) => emitter,
                                Err(error) => {
                                    tracing::warn!(%error, "failed to create peer signal emitter");
                                    continue;
                                }
                            };
                            if let Err(error) = DaemonService::peer_connected(emitter, peer_variant(&peer)).await {
                                tracing::warn!(%error, "failed to emit PeerConnected");
                            }
                        }
                        Ok(TransportEvent::PeerDisconnected(peer)) => {
                            let emitter = match SignalEmitter::new(&connection, ticonnect_core::ipc::DAEMON_PATH) {
                                Ok(emitter) => emitter,
                                Err(error) => {
                                    tracing::warn!(%error, "failed to create peer signal emitter");
                                    continue;
                                }
                            };
                            if let Err(error) = DaemonService::peer_disconnected(emitter, peer_variant(&peer)).await {
                                tracing::warn!(%error, "failed to emit PeerDisconnected");
                            }
                        }
                        Err(broadcast::error::RecvError::Lagged(skipped)) => {
                            tracing::warn!(skipped, "transport event stream lagged");
                        }
                        Err(broadcast::error::RecvError::Closed) => break,
                    }
                }
                event = pairing_events.recv() => {
                    let emitter = match SignalEmitter::new(&connection, ticonnect_core::ipc::DAEMON_PATH) {
                        Ok(emitter) => emitter,
                        Err(error) => {
                            tracing::warn!(%error, "failed to create pairing signal emitter");
                            continue;
                        }
                    };
                    match event {
                        Ok(PairingEvent::Started(started)) => {
                            if let Err(error) = DaemonService::pairing_started(emitter, start_variant(&started)).await {
                                tracing::warn!(%error, "failed to emit PairingStarted");
                            }
                        }
                        Ok(PairingEvent::Completed { peer_id, fingerprint }) => {
                            if let Err(error) = DaemonService::pairing_completed(emitter, &peer_id, &fingerprint).await {
                                tracing::warn!(%error, "failed to emit PairingCompleted");
                            }
                        }
                        Ok(PairingEvent::Failed(reason)) => {
                            if let Err(error) = DaemonService::pairing_failed(emitter, &reason).await {
                                tracing::warn!(%error, "failed to emit PairingFailed");
                            }
                        }
                        Err(broadcast::error::RecvError::Lagged(skipped)) => {
                            tracing::warn!(skipped, "pairing event stream lagged");
                        }
                        Err(broadcast::error::RecvError::Closed) => break,
                    }
                }
            }
        }
    })
}

fn start_variant(
    started: &StartPairingResponse,
) -> std::collections::HashMap<String, zbus::OwnedVariant> {
    use ticonnect_core::ipc;
    std::collections::HashMap::from([
        (
            ipc::PAIRING_PIN.to_owned(),
            zbus::OwnedVariant::from(zbus::Str::from(started.pin.clone())),
        ),
        (
            ipc::PAIRING_QR.to_owned(),
            zbus::OwnedVariant::from(zbus::Str::from(started.qr.clone())),
        ),
        (
            ipc::PAIRING_EXPIRES_AT.to_owned(),
            zbus::OwnedVariant::from(started.expires_at),
        ),
        (
            ipc::PAIRING_NODE_ID.to_owned(),
            zbus::OwnedVariant::from(zbus::Str::from(started.node_id.clone())),
        ),
        (
            ipc::PAIRING_PORT.to_owned(),
            zbus::OwnedVariant::from(u32::from(started.port)),
        ),
    ])
}

fn peer_variant(peer: &PeerInfo) -> std::collections::HashMap<String, zbus::OwnedVariant> {
    use ticonnect_core::ipc;
    std::collections::HashMap::from([
        (
            ipc::PEER_ID.to_owned(),
            zbus::OwnedVariant::from(zbus::Str::from(peer.node_id.to_string())),
        ),
        (
            ipc::PEER_FINGERPRINT.to_owned(),
            zbus::OwnedVariant::from(zbus::Str::from(peer.fingerprint.to_hex())),
        ),
        (
            ipc::PEER_ADDRESS.to_owned(),
            zbus::OwnedVariant::from(zbus::Str::from(peer.address.to_string())),
        ),
    ])
}

fn fingerprint_error(_: FingerprintError) -> PairingServiceError {
    PairingServiceError::InvalidIdentity("fingerprint is invalid".to_owned())
}

fn local_ipv4() -> Ipv4Addr {
    UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0))
        .and_then(|socket| {
            socket.connect((Ipv4Addr::new(192, 0, 2, 1), 9))?;
            socket.local_addr()
        })
        .ok()
        .and_then(|address| match address.ip() {
            IpAddr::V4(address) if !address.is_loopback() => Some(address),
            _ => None,
        })
        .unwrap_or(Ipv4Addr::LOCALHOST)
}

fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_secs())
}
