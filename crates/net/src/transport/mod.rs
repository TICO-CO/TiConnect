pub mod client;
pub mod server;
pub mod session;

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, PoisonError, RwLock};
use std::time::Duration;

use thiserror::Error;
use ticonnect_core::fingerprint::Fingerprint;
use ticonnect_core::id::NodeId;
use ticonnect_core::identity::IdentityKeyPair;
use ticonnect_core::wire::WireError;
use tokio::sync::broadcast;
use tokio::task::JoinHandle;

use crate::tls::{self, ClientAlpn, IdentityCertificate, TlsError};

pub use session::{PeerInfo, Session};

pub const DEFAULT_TRANSPORT_PORT: u16 = 45871;
pub const SESSION_REJECTED: u32 = 1;
pub const SHUTDOWN: u32 = 2;
pub const RECONNECT_ATTEMPTS: usize = 5;
pub const RECONNECT_BASE_DELAY: Duration = Duration::from_millis(100);

#[derive(Debug, Error)]
pub enum TransportError {
    #[error("failed to bind transport: {0}")]
    Bind(String),

    #[error(transparent)]
    Tls(#[from] TlsError),

    #[error("transport handshake failed: {0}")]
    Handshake(String),

    #[error("peer is not trusted")]
    Untrusted,

    #[error("peer identity does not match the expected identity")]
    IdentityMismatch,

    #[error("transport stream failed: {0}")]
    Stream(String),

    #[error(transparent)]
    Wire(#[from] WireError),

    #[error("transport is not running")]
    Stopped,
}

#[derive(Clone, Debug)]
pub enum TransportEvent {
    PairingRequested {
        fingerprint: Fingerprint,
        address: SocketAddr,
        connection: quinn::Connection,
    },
    PeerConnected(PeerInfo),
    PeerDisconnected(PeerInfo),
}

pub trait TrustLookup: Send + Sync {
    fn is_trusted(&self, fingerprint: &str) -> bool;
}

#[derive(Clone)]
pub struct TransportConfig {
    pub identity: IdentityKeyPair,
    pub node_id: NodeId,
    pub fingerprint: Fingerprint,
    pub bind_addr: SocketAddr,
    pub trust: Arc<dyn TrustLookup>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct TransportMetrics {
    pub handshakes: u64,
    pub failures: u64,
    pub active_sessions: u64,
}

#[derive(Debug, Default)]
pub(crate) struct AtomicMetrics {
    handshakes: AtomicU64,
    failures: AtomicU64,
    active_sessions: AtomicU64,
}

impl AtomicMetrics {
    fn snapshot(&self) -> TransportMetrics {
        TransportMetrics {
            handshakes: self.handshakes.load(Ordering::Relaxed),
            failures: self.failures.load(Ordering::Relaxed),
            active_sessions: self.active_sessions.load(Ordering::Relaxed),
        }
    }

    pub(crate) fn note_handshake(&self) {
        self.handshakes.fetch_add(1, Ordering::Relaxed);
    }

    pub(crate) fn note_failure(&self) {
        self.failures.fetch_add(1, Ordering::Relaxed);
    }

    pub(crate) fn note_connected(&self) {
        self.active_sessions.fetch_add(1, Ordering::Relaxed);
    }

    pub(crate) fn note_disconnected(&self) {
        self.active_sessions.fetch_sub(1, Ordering::Relaxed);
    }
}

#[derive(Clone)]
pub struct Transport {
    endpoint: quinn::Endpoint,
    certificate: IdentityCertificate,
    node_id: NodeId,
    fingerprint: Fingerprint,
    trust: Arc<dyn TrustLookup>,
    sessions: Arc<RwLock<HashMap<String, Session>>>,
    events: broadcast::Sender<TransportEvent>,
    metrics: Arc<AtomicMetrics>,
    accept_task: Arc<Mutex<Option<JoinHandle<()>>>>,
}

impl Transport {
    pub async fn bind(config: TransportConfig) -> Result<Self, TransportError> {
        let certificate = IdentityCertificate::from_identity(&config.identity)?;
        if certificate.fingerprint() != config.fingerprint {
            return Err(TransportError::IdentityMismatch);
        }
        let server = tls::server_config(&certificate)?;
        let endpoint = quinn::Endpoint::server(server, config.bind_addr)
            .map_err(|error| TransportError::Bind(error.to_string()))?;
        let (events, _receiver) = broadcast::channel(64);
        let transport = Self {
            endpoint,
            certificate,
            node_id: config.node_id,
            fingerprint: config.fingerprint,
            trust: config.trust,
            sessions: Arc::new(RwLock::new(HashMap::new())),
            events,
            metrics: Arc::new(AtomicMetrics::default()),
            accept_task: Arc::new(Mutex::new(None)),
        };

        let accept = server::spawn_accept_loop(transport.clone());
        *transport
            .accept_task
            .lock()
            .unwrap_or_else(PoisonError::into_inner) = Some(accept);

        Ok(transport)
    }

    pub fn local_addr(&self) -> Result<SocketAddr, TransportError> {
        self.endpoint
            .local_addr()
            .map_err(|error| TransportError::Bind(error.to_string()))
    }

    pub fn fingerprint(&self) -> Fingerprint {
        self.fingerprint
    }

    pub fn certificate(&self) -> &IdentityCertificate {
        &self.certificate
    }

    pub fn subscribe(&self) -> broadcast::Receiver<TransportEvent> {
        self.events.subscribe()
    }

    pub fn metrics(&self) -> TransportMetrics {
        self.metrics.snapshot()
    }

    pub fn session_count(&self) -> usize {
        self.sessions
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .len()
    }

    pub fn sessions(&self) -> Vec<Session> {
        self.sessions
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .values()
            .cloned()
            .collect()
    }

    pub async fn connect(&self, target: DialTarget) -> Result<Session, TransportError> {
        client::connect(self, target).await
    }

    pub async fn connect_with_retry(&self, target: DialTarget) -> Result<Session, TransportError> {
        client::connect_with_retry(self, target).await
    }

    pub async fn connect_pairing(
        &self,
        target: DialTarget,
    ) -> Result<quinn::Connection, TransportError> {
        client::connect_pairing(self, target).await
    }

    pub async fn connect_pairing_with_retry(
        &self,
        target: DialTarget,
    ) -> Result<quinn::Connection, TransportError> {
        client::connect_pairing_with_retry(self, target).await
    }

    pub async fn shutdown(&self) {
        self.endpoint.close(SHUTDOWN.into(), b"shutdown");
        if let Some(task) = self
            .accept_task
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .take()
        {
            task.abort();
        }
        self.endpoint.wait_idle().await;
    }

    pub(crate) fn endpoint(&self) -> &quinn::Endpoint {
        &self.endpoint
    }

    pub(crate) fn node_id(&self) -> NodeId {
        self.node_id
    }

    pub(crate) fn expected_fingerprint(&self) -> Fingerprint {
        self.fingerprint
    }

    pub(crate) fn trust(&self) -> &Arc<dyn TrustLookup> {
        &self.trust
    }

    pub(crate) fn events(&self) -> &broadcast::Sender<TransportEvent> {
        &self.events
    }

    pub(crate) fn register(&self, session: Session) {
        let key = session.peer().fingerprint.to_hex();
        let previous = self
            .sessions
            .write()
            .unwrap_or_else(PoisonError::into_inner)
            .insert(key, session.clone());
        if let Some(previous) = previous {
            previous.close(SESSION_REJECTED, b"replaced");
            self.metrics.note_disconnected();
            let _ = self
                .events
                .send(TransportEvent::PeerDisconnected(previous.peer().clone()));
        }
        self.metrics.note_connected();
        self.metrics.note_handshake();
        let _ = self
            .events
            .send(TransportEvent::PeerConnected(session.peer().clone()));
    }

    pub(crate) fn unregister(&self, connection_id: usize, peer: &PeerInfo) {
        let mut sessions = self
            .sessions
            .write()
            .unwrap_or_else(PoisonError::into_inner);
        let remove = sessions
            .get(&peer.fingerprint.to_hex())
            .is_some_and(|session| session.connection().stable_id() == connection_id);
        if remove {
            sessions.remove(&peer.fingerprint.to_hex());
            self.metrics.note_disconnected();
            let _ = self
                .events
                .send(TransportEvent::PeerDisconnected(peer.clone()));
        }
    }

    pub(crate) fn note_failure(&self) {
        self.metrics.note_failure();
    }

    pub(crate) fn client_config(
        &self,
        expected: Fingerprint,
        alpn: ClientAlpn,
    ) -> Result<quinn::ClientConfig, TransportError> {
        tls::client_config(&self.certificate, expected, alpn).map_err(Into::into)
    }
}

#[derive(Debug, Clone)]
pub struct DialTarget {
    pub address: SocketAddr,
    pub node_id: Option<NodeId>,
    pub fingerprint: Fingerprint,
}

impl DialTarget {
    pub fn new(address: SocketAddr, fingerprint: Fingerprint) -> Self {
        Self {
            address,
            node_id: None,
            fingerprint,
        }
    }

    pub fn with_node_id(mut self, node_id: NodeId) -> Self {
        self.node_id = Some(node_id);
        self
    }
}
