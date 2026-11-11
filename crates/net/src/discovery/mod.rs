pub mod broadcast;
pub mod candidate;
pub mod mdns;

use std::collections::BTreeSet;
use std::net::{IpAddr, SocketAddr};
use std::str::FromStr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, Instant};

use thiserror::Error;
use ticonnect_core::id::NodeId;
use tokio::sync::{broadcast as event_bus, watch};
use tokio::task::JoinHandle;

use self::candidate::{Candidate, CandidateCache, CandidateChange};

pub const SERVICE_TYPE: &str = "_ticonnect._udp.local.";
pub const PROTOCOL_VERSION: u16 = 1;
pub const BEACON_PORT: u16 = 45870;
pub const BEACON_INTERVAL: Duration = Duration::from_secs(5);
pub const MAX_PACKET_SIZE: usize = 1024;
pub const MAX_CANDIDATES: usize = 256;
pub const MAX_ADDRESSES: usize = 8;
pub const MAX_DISPLAY_NAME: usize = 64;
pub const MAX_ADDRESS_LENGTH: usize = 45;

#[derive(Debug, Error)]
pub enum DiscoveryError {
    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error("mDNS error: {0}")]
    Mdns(String),

    #[error("invalid discovery record: {0}")]
    InvalidRecord(String),

    #[error("unsupported discovery protocol version: {0}")]
    ProtocolVersion(u16),
}

#[derive(Debug, Clone)]
pub struct DiscoveryConfig {
    pub enabled: bool,
    pub mdns: bool,
    pub broadcast: bool,
    pub beacon_interval: Duration,
    pub display_name: String,
    pub node_id: String,
    pub fingerprint: String,
    pub beacon_port: u16,
    pub beacon_targets: Option<Vec<SocketAddr>>,
}

impl DiscoveryConfig {
    pub fn new(
        display_name: impl Into<String>,
        node_id: impl Into<String>,
        fingerprint: impl Into<String>,
    ) -> Self {
        Self {
            enabled: true,
            mdns: true,
            broadcast: true,
            beacon_interval: BEACON_INTERVAL,
            display_name: display_name.into(),
            node_id: node_id.into(),
            fingerprint: fingerprint.into(),
            beacon_port: BEACON_PORT,
            beacon_targets: None,
        }
    }

    pub fn with_beacon_override(
        mut self,
        port: u16,
        targets: impl IntoIterator<Item = SocketAddr>,
    ) -> Self {
        self.beacon_port = port;
        self.beacon_targets = Some(targets.into_iter().collect());
        self
    }
}

impl Default for DiscoveryConfig {
    fn default() -> Self {
        Self::new(String::new(), String::new(), String::new())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DiscoveryStatus {
    pub enabled: bool,
    pub mdns_enabled: bool,
    pub broadcast_enabled: bool,
    pub mdns_active: bool,
    pub broadcast_active: bool,
    pub candidate_count: usize,
    pub dropped_packets: u64,
}

#[derive(Debug, Clone)]
pub enum DiscoveryEvent {
    Found(Candidate),
    Lost(String),
}

pub trait Discovery: Send + Sync {
    fn start(&self) -> Result<(), DiscoveryError>;
    fn stop(&self);
    fn snapshot(&self) -> Vec<Candidate>;
    fn subscribe(&self) -> event_bus::Receiver<DiscoveryEvent>;
    fn status(&self) -> DiscoveryStatus;
}

pub struct LanDiscovery {
    config: Mutex<DiscoveryConfig>,
    cache: Arc<Mutex<CandidateCache>>,
    events: event_bus::Sender<DiscoveryEvent>,
    state: Mutex<Option<RuntimeState>>,
    active: Arc<AtomicBool>,
    mdns_active: Arc<AtomicBool>,
    broadcast_active: Arc<AtomicBool>,
}

struct RuntimeState {
    shutdown: watch::Sender<bool>,
    tasks: Vec<JoinHandle<()>>,
}

impl LanDiscovery {
    pub fn new(config: DiscoveryConfig) -> Self {
        let (events, _receiver) = event_bus::channel(64);
        Self {
            config: Mutex::new(config),
            cache: Arc::new(Mutex::new(CandidateCache::new(MAX_CANDIDATES))),
            events,
            state: Mutex::new(None),
            active: Arc::new(AtomicBool::new(false)),
            mdns_active: Arc::new(AtomicBool::new(false)),
            broadcast_active: Arc::new(AtomicBool::new(false)),
        }
    }

    pub async fn stop_and_wait(&self) {
        self.active.store(false, Ordering::SeqCst);
        let state = self
            .state
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .take();

        if let Some(state) = state {
            let _ = state.shutdown.send(true);
            for task in state.tasks {
                let _ = task.await;
            }
        }

        self.mdns_active.store(false, Ordering::SeqCst);
        self.broadcast_active.store(false, Ordering::SeqCst);
        self.lose_all();
    }

    pub async fn reconfigure(&self, config: DiscoveryConfig) -> Result<(), DiscoveryError> {
        self.stop_and_wait().await;
        *self.config.lock().unwrap_or_else(PoisonError::into_inner) = config;
        self.start()
    }

    fn lose_all(&self) {
        let removed = self
            .cache
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clear();
        for candidate in removed {
            let _ = self
                .events
                .send(DiscoveryEvent::Lost(candidate.fingerprint().to_owned()));
        }
    }
}

impl Discovery for LanDiscovery {
    fn start(&self) -> Result<(), DiscoveryError> {
        if self
            .state
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .is_some()
        {
            return Ok(());
        }

        let config = self
            .config
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone();

        if !config.enabled {
            self.mdns_active.store(false, Ordering::SeqCst);
            self.broadcast_active.store(false, Ordering::SeqCst);
            return Ok(());
        }

        let filter = Filter::new(&config);
        let sink = Sink {
            cache: self.cache.clone(),
            events: self.events.clone(),
            filter,
            active: self.active.clone(),
            mdns_active: self.mdns_active.clone(),
            broadcast_active: self.broadcast_active.clone(),
        };
        let (shutdown, shutdown_rx) = watch::channel(false);
        let mut tasks = Vec::new();

        if config.mdns {
            match mdns::spawn(config.clone(), sink.clone(), shutdown_rx.clone()) {
                Ok(task) => tasks.push(task),
                Err(error) => {
                    tracing::warn!(%error, "mDNS discovery unavailable; continuing without it");
                }
            }
        }

        if config.broadcast {
            match broadcast::spawn(config.clone(), sink.clone(), shutdown_rx.clone()) {
                Ok(task) => tasks.push(task),
                Err(error) => {
                    tracing::warn!(%error, "broadcast discovery unavailable; continuing without it");
                }
            }
        }

        let ttl = config.beacon_interval.saturating_mul(3);
        tasks.push(spawn_expiry(
            self.cache.clone(),
            self.events.clone(),
            shutdown_rx,
            ttl,
        ));

        self.active.store(true, Ordering::SeqCst);
        *self.state.lock().unwrap_or_else(PoisonError::into_inner) =
            Some(RuntimeState { shutdown, tasks });

        Ok(())
    }

    fn stop(&self) {
        self.active.store(false, Ordering::SeqCst);
        if let Some(state) = self
            .state
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .take()
        {
            let _ = state.shutdown.send(true);
        }
        self.mdns_active.store(false, Ordering::SeqCst);
        self.broadcast_active.store(false, Ordering::SeqCst);
        self.lose_all();
    }

    fn snapshot(&self) -> Vec<Candidate> {
        self.cache
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .snapshot()
    }

    fn subscribe(&self) -> event_bus::Receiver<DiscoveryEvent> {
        self.events.subscribe()
    }

    fn status(&self) -> DiscoveryStatus {
        let config = self
            .config
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone();
        let cache = self.cache.lock().unwrap_or_else(PoisonError::into_inner);

        DiscoveryStatus {
            enabled: config.enabled,
            mdns_enabled: config.mdns,
            broadcast_enabled: config.broadcast,
            mdns_active: self.mdns_active.load(Ordering::SeqCst),
            broadcast_active: self.broadcast_active.load(Ordering::SeqCst),
            candidate_count: cache.len(),
            dropped_packets: cache.dropped(),
        }
    }
}

impl Drop for LanDiscovery {
    fn drop(&mut self) {
        self.active.store(false, Ordering::SeqCst);
        if let Some(state) = self
            .state
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .take()
        {
            let _ = state.shutdown.send(true);
        }
    }
}

#[derive(Clone)]
pub(crate) struct Sink {
    cache: Arc<Mutex<CandidateCache>>,
    events: event_bus::Sender<DiscoveryEvent>,
    filter: Filter,
    active: Arc<AtomicBool>,
    mdns_active: Arc<AtomicBool>,
    broadcast_active: Arc<AtomicBool>,
}

impl Sink {
    pub(crate) fn offer(&self, candidate: Candidate) {
        if !self.active.load(Ordering::SeqCst) {
            return;
        }

        let candidate = match self.filter.apply(candidate) {
            Ok(candidate) => candidate,
            Err(FilterError::SelfCandidate) => return,
            Err(FilterError::Invalid(error)) => {
                tracing::debug!(%error, "discarded invalid discovery candidate");
                self.note_dropped();
                return;
            }
        };

        let change = self
            .cache
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .upsert(candidate);

        if let CandidateChange::Found(candidate) = change {
            let _ = self.events.send(DiscoveryEvent::Found(candidate));
        }
    }

    pub(crate) fn remove(&self, fingerprint: &str) {
        if !self.active.load(Ordering::SeqCst) {
            return;
        }
        let removed = self
            .cache
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .remove(fingerprint);
        if removed.is_some() {
            let _ = self
                .events
                .send(DiscoveryEvent::Lost(fingerprint.to_owned()));
        }
    }

    pub(crate) fn note_dropped(&self) {
        self.cache
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .note_dropped();
    }

    pub(crate) fn set_mdns_active(&self, active: bool) {
        self.mdns_active.store(active, Ordering::SeqCst);
    }

    pub(crate) fn set_broadcast_active(&self, active: bool) {
        self.broadcast_active.store(active, Ordering::SeqCst);
    }
}

#[derive(Clone)]
struct Filter {
    node_id: String,
    fingerprint: String,
    protocol_version: u16,
}

impl Filter {
    fn new(config: &DiscoveryConfig) -> Self {
        Self {
            node_id: config.node_id.clone(),
            fingerprint: config.fingerprint.to_ascii_lowercase(),
            protocol_version: PROTOCOL_VERSION,
        }
    }

    fn apply(&self, candidate: Candidate) -> Result<Candidate, FilterError> {
        if candidate.protocol_version() != self.protocol_version {
            return Err(FilterError::Invalid(format!(
                "protocol version {} is not supported",
                candidate.protocol_version()
            )));
        }

        let node_id = NodeId::from_str(candidate.node_id())
            .map_err(|_| FilterError::Invalid("invalid node id".to_owned()))?
            .to_string();
        let fingerprint = validate_fingerprint(candidate.fingerprint())?;

        if node_id == self.node_id || fingerprint == self.fingerprint {
            return Err(FilterError::SelfCandidate);
        }

        if candidate.display_name().len() > MAX_DISPLAY_NAME {
            return Err(FilterError::Invalid("display name is too long".to_owned()));
        }
        if candidate
            .display_name()
            .chars()
            .any(|character| character.is_control())
        {
            return Err(FilterError::Invalid(
                "display name contains control characters".to_owned(),
            ));
        }

        let mut addresses = BTreeSet::new();
        for address in candidate.addresses().take(MAX_ADDRESSES + 1) {
            if address.len() > MAX_ADDRESS_LENGTH {
                return Err(FilterError::Invalid("address is too long".to_owned()));
            }
            let parsed = IpAddr::from_str(address)
                .map_err(|_| FilterError::Invalid("invalid IP address".to_owned()))?;
            if parsed.is_unspecified() || parsed.is_multicast() {
                return Err(FilterError::Invalid("unusable IP address".to_owned()));
            }
            addresses.insert(parsed.to_string());
        }
        if addresses.is_empty() {
            return Err(FilterError::Invalid(
                "candidate has no addresses".to_owned(),
            ));
        }
        if addresses.len() > MAX_ADDRESSES {
            return Err(FilterError::Invalid("too many addresses".to_owned()));
        }

        Ok(Candidate::new(
            node_id,
            fingerprint,
            candidate.display_name().to_owned(),
            addresses,
            candidate.port(),
            candidate.protocol_version(),
            candidate.source(),
            Instant::now(),
        ))
    }
}

enum FilterError {
    SelfCandidate,
    Invalid(String),
}

fn validate_fingerprint(value: &str) -> Result<String, FilterError> {
    if value.len() != 64 || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(FilterError::Invalid("invalid fingerprint".to_owned()));
    }
    Ok(value.to_ascii_lowercase())
}

fn spawn_expiry(
    cache: Arc<Mutex<CandidateCache>>,
    events: event_bus::Sender<DiscoveryEvent>,
    mut shutdown: watch::Receiver<bool>,
    ttl: Duration,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        let mut ticker = tokio::time::interval(Duration::from_secs(1));
        loop {
            tokio::select! {
                changed = shutdown.changed() => {
                    if changed.is_err() || *shutdown.borrow() {
                        break;
                    }
                }
                _ = ticker.tick() => {
                    let expired = cache
                        .lock()
                        .unwrap_or_else(PoisonError::into_inner)
                        .expire(Instant::now(), ttl);
                    for candidate in expired {
                        let _ = events.send(DiscoveryEvent::Lost(
                            candidate.fingerprint().to_owned(),
                        ));
                    }
                }
            }
        }
    })
}
