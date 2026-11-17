use std::collections::{HashMap, HashSet};
use std::net::{IpAddr, SocketAddr};
use std::str::FromStr;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use thiserror::Error;
use ticonnect_core::fingerprint::{Fingerprint, FingerprintError};
use ticonnect_core::id::NodeId;
use ticonnect_net::discovery::candidate::Candidate;
use ticonnect_net::discovery::{Discovery, DiscoveryEvent, LanDiscovery};
use ticonnect_net::transport::{DialTarget, Transport, TransportError, TransportEvent, UserInfo};
use ticonnect_net::trust::{
    EndpointError, InitiatorRequest, ResponderRequest, TrustUser, run_initiator, run_responder,
};
use tokio::task::JoinHandle;
use zbus::object_server::SignalEmitter;

use crate::identity::PublicIdentity;
use crate::service::DaemonService;
use crate::storage::{NewUser, StorageError, User, UserPolicy};
use crate::trust::{TrustError, TrustStore};

#[derive(Debug, Error)]
pub enum TrustServiceError {
    #[error("no discovery candidate found for node {0}")]
    NoCandidate(String),

    #[error("candidate address is invalid")]
    InvalidAddress,

    #[error("candidate port is not available")]
    MissingPort,

    #[error("invalid user identity: {0}")]
    InvalidIdentity(String),

    #[error("trusted user was not found")]
    NotFound,

    #[error(transparent)]
    Transport(#[from] TransportError),

    #[error(transparent)]
    Endpoint(#[from] EndpointError),

    #[error(transparent)]
    Trust(#[from] TrustError),

    #[error(transparent)]
    Storage(#[from] StorageError),
}

#[derive(Default)]
struct AutoTrustState {
    inflight: HashSet<String>,
    last_attempt: HashMap<String, Instant>,
}

const AUTO_TRUST_RETRY: Duration = Duration::from_secs(15);
const AUTO_TRUST_SCAN: Duration = Duration::from_secs(2);

pub struct TrustService {
    identity: PublicIdentity,
    trust: Arc<TrustStore>,
    transport: Arc<Transport>,
    discovery: Arc<LanDiscovery>,
    auto_trust: std::sync::Mutex<AutoTrustState>,
}

impl TrustService {
    pub fn new(
        identity: PublicIdentity,
        trust: Arc<TrustStore>,
        transport: Arc<Transport>,
        discovery: Arc<LanDiscovery>,
    ) -> Self {
        Self {
            identity,
            trust,
            transport,
            discovery,
            auto_trust: std::sync::Mutex::new(AutoTrustState::default()),
        }
    }

    fn queue_auto_trust(self: &Arc<Self>, candidate: Candidate) {
        let fingerprint = match Fingerprint::from_str(candidate.fingerprint()) {
            Ok(fingerprint) => fingerprint,
            Err(_) => return,
        };
        let node_id = match NodeId::from_str(candidate.node_id()) {
            Ok(node_id) => node_id,
            Err(_) => return,
        };
        if fingerprint == self.identity.fingerprint()
            || node_id == self.identity.node_id()
            || self.trust.is_trusted(&fingerprint.to_hex())
            || self.trust.is_blocked(&fingerprint.to_hex())
        {
            return;
        }

        let key = fingerprint.to_hex();
        {
            let mut state = self
                .auto_trust
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            if state.inflight.contains(&key) {
                return;
            }
            if state
                .last_attempt
                .get(&key)
                .is_some_and(|attempt| attempt.elapsed() < AUTO_TRUST_RETRY)
            {
                return;
            }
            state.inflight.insert(key.clone());
            state.last_attempt.insert(key.clone(), Instant::now());
        }

        let service = self.clone();
        tokio::spawn(async move {
            match service.trust_candidate(candidate).await {
                Ok(user) => {
                    tracing::info!(user = %user.id, "LAN user trusted automatically");
                }
                Err(error) => {
                    tracing::debug!(%error, "automatic trust failed");
                }
            }
            service
                .auto_trust
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .inflight
                .remove(&key);
        });
    }

    fn scan_auto_trust(self: &Arc<Self>) {
        for candidate in self.discovery.snapshot() {
            self.queue_auto_trust(candidate);
        }
    }

    async fn trust_candidate(&self, candidate: Candidate) -> Result<User, TrustServiceError> {
        let node_id = NodeId::from_str(candidate.node_id())
            .map_err(|error| TrustServiceError::InvalidIdentity(error.to_string()))?;
        let fingerprint =
            Fingerprint::from_str(candidate.fingerprint()).map_err(fingerprint_error)?;
        if self.trust.is_blocked(&fingerprint.to_hex()) {
            return Err(TrustServiceError::Trust(TrustError::NotFound));
        }
        let address = candidate
            .addresses()
            .next()
            .ok_or(TrustServiceError::InvalidAddress)?
            .parse::<IpAddr>()
            .map_err(|_| TrustServiceError::InvalidAddress)?;
        if candidate.port() == 0 {
            return Err(TrustServiceError::MissingPort);
        }
        let expected = TrustUser {
            node_id,
            fingerprint,
            certificate_fingerprint: fingerprint,
        };
        let display_name =
            (!candidate.display_name().is_empty()).then(|| candidate.display_name().to_owned());
        let token = lan_trust_token(self.identity.fingerprint(), fingerprint);
        self.trust_as_initiator(
            SocketAddr::new(address, candidate.port()),
            expected,
            token,
            display_name,
        )
        .await
    }

    async fn trust_as_initiator(
        &self,
        address: SocketAddr,
        expected: TrustUser,
        token: [u8; 32],
        display_name: Option<String>,
    ) -> Result<User, TrustServiceError> {
        let local = self.local_user();
        let target = DialTarget::new(address, expected.fingerprint).with_node_id(expected.node_id);
        let connection = self
            .transport
            .connect_trust_with_retry(target.clone())
            .await?;
        let outcome = run_initiator(
            &connection,
            InitiatorRequest {
                local,
                expected: expected.clone(),
                token,
            },
        )
        .await;
        connection.close(0u32.into(), b"trusted");
        let outcome = outcome?;
        let user = self.commit_user(outcome.user.clone(), display_name).await?;
        if self.identity.node_id() < outcome.user.node_id
            && let Err(error) = self.transport.connect(target).await
        {
            tracing::warn!(%error, "failed to open session after trust");
        }
        Ok(user)
    }

    pub async fn forget_user(&self, user_id: &str) -> Result<User, TrustServiceError> {
        let user = self.trust.forget(user_id).await?;
        for session in self.transport.sessions() {
            if session.user().fingerprint.to_hex() == user.fingerprint {
                session.close(0, b"forgotten");
            }
        }
        Ok(user)
    }

    pub async fn list_users(&self) -> Result<Vec<User>, TrustServiceError> {
        self.trust.list().await.map_err(Into::into)
    }

    pub async fn list_blocked_users(
        &self,
    ) -> Result<Vec<crate::storage::BlockedUser>, TrustServiceError> {
        self.trust.blocked_users().await.map_err(Into::into)
    }

    pub async fn unblock_user(&self, fingerprint: &str) -> Result<bool, TrustServiceError> {
        self.trust.unblock(fingerprint).await.map_err(Into::into)
    }

    async fn handle_trust_request(&self, connection: quinn::Connection, fingerprint: Fingerprint) {
        if self.trust.is_blocked(&fingerprint.to_hex()) {
            connection.close(0u32.into(), b"blocked");
            return;
        }

        let token = lan_trust_token(self.identity.fingerprint(), fingerprint);
        let result = run_responder(
            &connection,
            ResponderRequest {
                local: self.local_user(),
                token,
                certificate_fingerprint: fingerprint,
            },
        )
        .await;

        let outcome = match result {
            Ok(outcome) => outcome,
            Err(error) => {
                tracing::debug!(%error, "incoming automatic trust failed");
                return;
            }
        };

        let user = outcome.user;
        let display_name = self
            .discovery
            .snapshot()
            .into_iter()
            .find(|candidate| candidate.fingerprint() == user.fingerprint.to_hex())
            .map(|candidate| candidate.display_name().to_owned())
            .filter(|name| !name.is_empty());
        let target = DialTarget::new(connection.remote_address(), user.fingerprint)
            .with_node_id(user.node_id);
        match self.commit_user(user.clone(), display_name).await {
            Ok(_) => {
                if self.identity.node_id() < user.node_id
                    && let Err(error) = self.transport.connect(target).await
                {
                    tracing::warn!(%error, "failed to open session after trust");
                }
            }
            Err(error) => {
                tracing::warn!(%error, "failed to commit trusted user");
            }
        }
    }

    async fn commit_user(
        &self,
        user: TrustUser,
        display_name: Option<String>,
    ) -> Result<User, TrustServiceError> {
        let new_user = NewUser {
            id: user.node_id.to_string(),
            fingerprint: user.fingerprint.to_hex(),
            public_key: None,
            display_name,
            policy: UserPolicy::Accept,
            trusted_at: i64::try_from(now_unix()).unwrap_or(i64::MAX),
            last_seen: None,
        };

        match self.trust.commit_auto(new_user).await {
            Ok(user) => Ok(user),
            Err(TrustError::Storage(StorageError::DuplicateFingerprint(_))) => self
                .trust
                .list()
                .await
                .map_err(TrustServiceError::from)?
                .into_iter()
                .find(|existing| existing.fingerprint == user.fingerprint.to_hex())
                .ok_or(TrustServiceError::NotFound),
            Err(error) => Err(error.into()),
        }
    }

    fn local_user(&self) -> TrustUser {
        TrustUser {
            node_id: self.identity.node_id(),
            fingerprint: self.identity.fingerprint(),
            certificate_fingerprint: self.identity.fingerprint(),
        }
    }
}

fn lan_trust_token(local: Fingerprint, remote: Fingerprint) -> [u8; 32] {
    let mut fingerprints = [local.to_hex(), remote.to_hex()];
    fingerprints.sort();
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"ticonnect lan trust v1");
    hasher.update(fingerprints[0].as_bytes());
    hasher.update(fingerprints[1].as_bytes());
    *hasher.finalize().as_bytes()
}

pub fn spawn(connection: zbus::Connection, service: Arc<TrustService>) -> JoinHandle<()> {
    let transport = service.transport.clone();
    let mut transport_events = transport.subscribe();
    let mut discovery_events = service.discovery.subscribe();
    let mut auto_scan = tokio::time::interval(AUTO_TRUST_SCAN);
    auto_scan.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);

    tokio::spawn(async move {
        loop {
            tokio::select! {
                event = discovery_events.recv() => {
                    if let Ok(DiscoveryEvent::Found(candidate)) = event {
                        service.clone().queue_auto_trust(candidate);
                    }
                }
                _ = auto_scan.tick() => {
                    service.clone().scan_auto_trust();
                }
                event = transport_events.recv() => {
                    match event {
                        Ok(TransportEvent::TrustRequested { fingerprint, connection: trust_connection, .. }) => {
                            let service = service.clone();
                            tokio::spawn(async move {
                                service.handle_trust_request(trust_connection, fingerprint).await;
                            });
                        }
                        Ok(TransportEvent::ProfileRequested { .. }) => {}
                        Ok(TransportEvent::UserConnected(user)) => {
                            let emitter = match SignalEmitter::new(&connection, ticonnect_core::ipc::DAEMON_PATH) {
                                Ok(emitter) => emitter,
                                Err(error) => {
                                    tracing::warn!(%error, "failed to create user signal emitter");
                                    continue;
                                }
                            };
                            if let Err(error) = DaemonService::user_connected(emitter, user_variant(&user)).await {
                                tracing::warn!(%error, "failed to emit UserConnected");
                            }
                        }
                        Ok(TransportEvent::UserDisconnected(user)) => {
                            let emitter = match SignalEmitter::new(&connection, ticonnect_core::ipc::DAEMON_PATH) {
                                Ok(emitter) => emitter,
                                Err(error) => {
                                    tracing::warn!(%error, "failed to create user signal emitter");
                                    continue;
                                }
                            };
                            if let Err(error) = DaemonService::user_disconnected(emitter, user_variant(&user)).await {
                                tracing::warn!(%error, "failed to emit UserDisconnected");
                            }
                        }
                        Err(tokio::sync::broadcast::error::RecvError::Lagged(skipped)) => {
                            tracing::warn!(skipped, "transport event stream lagged");
                        }
                        Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                    }
                }
            }
        }
    })
}

fn user_variant(user: &UserInfo) -> std::collections::HashMap<String, zbus::OwnedVariant> {
    use ticonnect_core::ipc;
    std::collections::HashMap::from([
        (
            ipc::USER_ID.to_owned(),
            zbus::OwnedVariant::from(zbus::Str::from(user.node_id.to_string())),
        ),
        (
            ipc::USER_FINGERPRINT.to_owned(),
            zbus::OwnedVariant::from(zbus::Str::from(user.fingerprint.to_hex())),
        ),
        (
            ipc::USER_ADDRESS.to_owned(),
            zbus::OwnedVariant::from(zbus::Str::from(user.address.to_string())),
        ),
    ])
}

fn fingerprint_error(_: FingerprintError) -> TrustServiceError {
    TrustServiceError::InvalidIdentity("fingerprint is invalid".to_owned())
}

fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_secs())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lan_trust_token_is_symmetric_and_user_specific() {
        let first = Fingerprint::from_bytes([1; 32]);
        let second = Fingerprint::from_bytes([2; 32]);
        let third = Fingerprint::from_bytes([3; 32]);

        assert_eq!(
            lan_trust_token(first, second),
            lan_trust_token(second, first)
        );
        assert_ne!(
            lan_trust_token(first, second),
            lan_trust_token(first, third)
        );
    }
}
