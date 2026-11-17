use std::collections::HashSet;
use std::io;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::{Arc, Mutex, PoisonError, RwLock};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use quinn::Connection;
use ticonnect_core::fingerprint::Fingerprint;
use ticonnect_core::id::NodeId;
use ticonnect_core::identity::IdentityKeyPair;
use ticonnect_core::profile::{MAX_AVATAR_BYTES, ProfileError, ProfileRecord};
use ticonnect_net::discovery::candidate::Candidate;
use ticonnect_net::discovery::{Discovery, DiscoveryEvent, LanDiscovery};
use ticonnect_net::transport::{DialTarget, Transport, TransportEvent};
use tokio::sync::broadcast;

use crate::storage::{DatabaseHandle, UserProfile};

const SELF_AVATAR: &str = "self.png";
const PROFILE_DIR: &str = "profile";
const REQUEST: &[u8; 5] = b"TICP1";
const IO_TIMEOUT: Duration = Duration::from_secs(5);
const MAX_RESPONSE: usize = 256 * 1024;
const MAX_CONCURRENT_SERVES: usize = 8;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelfProfile {
    pub display_name: String,
    pub avatar: Vec<u8>,
    pub avatar_hash: Option<String>,
}

impl SelfProfile {
    fn record(&self, node_id: NodeId) -> Result<ProfileRecord, ProfileError> {
        ProfileRecord::new(
            node_id.to_string(),
            self.display_name.clone(),
            self.avatar.clone(),
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProfileEvent {
    SelfUpdated,
    UserUpdated { fingerprint: String },
}

pub struct ProfileService {
    dir: PathBuf,
    identity: IdentityKeyPair,
    node_id: NodeId,
    database: DatabaseHandle,
    transport: Arc<Transport>,
    profile: RwLock<SelfProfile>,
    in_flight: Mutex<HashSet<String>>,
    serving: Mutex<usize>,
    events: broadcast::Sender<ProfileEvent>,
}

impl ProfileService {
    pub fn load(
        data_dir: &Path,
        identity: IdentityKeyPair,
        node_id: NodeId,
        display_name: String,
        database: DatabaseHandle,
        transport: Arc<Transport>,
    ) -> Result<Self, ProfileError> {
        let dir = data_dir.join(PROFILE_DIR);
        std::fs::create_dir_all(&dir).map_err(|error| ProfileError::Decode(error.to_string()))?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700));
        }
        let avatar = std::fs::read(dir.join(SELF_AVATAR)).unwrap_or_default();
        let avatar = if valid_avatar(&avatar) {
            avatar
        } else {
            Vec::new()
        };
        let profile = SelfProfile {
            display_name,
            avatar_hash: (!avatar.is_empty()).then(|| blake3::hash(&avatar).to_hex().to_string()),
            avatar,
        };
        let (events, _receiver) = broadcast::channel(64);
        Ok(Self {
            dir,
            identity,
            node_id,
            database,
            transport,
            profile: RwLock::new(profile),
            in_flight: Mutex::new(HashSet::new()),
            serving: Mutex::new(0),
            events,
        })
    }

    pub fn subscribe(&self) -> broadcast::Receiver<ProfileEvent> {
        self.events.subscribe()
    }

    pub fn display_name(&self) -> String {
        self.profile
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .display_name
            .clone()
    }

    pub fn avatar_hash(&self) -> Option<String> {
        self.profile
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .avatar_hash
            .clone()
    }

    pub fn avatar(&self) -> Vec<u8> {
        self.profile
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .avatar
            .clone()
    }

    pub fn set_display_name(&self, display_name: String) {
        let mut profile = self.profile.write().unwrap_or_else(PoisonError::into_inner);
        if profile.display_name != display_name {
            profile.display_name = display_name;
            let _ = self.events.send(ProfileEvent::SelfUpdated);
        }
    }

    pub fn set_avatar(&self, avatar: Vec<u8>) -> Result<(), ProfileError> {
        if !valid_avatar(&avatar) {
            return Err(ProfileError::InvalidAvatar);
        }
        let path = self.dir.join(SELF_AVATAR);
        write_private(&path, &avatar).map_err(|error| ProfileError::Decode(error.to_string()))?;
        let mut profile = self.profile.write().unwrap_or_else(PoisonError::into_inner);
        profile.avatar_hash = Some(blake3::hash(&avatar).to_hex().to_string());
        profile.avatar = avatar;
        drop(profile);
        let _ = self.events.send(ProfileEvent::SelfUpdated);
        Ok(())
    }

    pub fn clear_avatar(&self) -> Result<(), ProfileError> {
        let _ = std::fs::remove_file(self.dir.join(SELF_AVATAR));
        let mut profile = self.profile.write().unwrap_or_else(PoisonError::into_inner);
        profile.avatar.clear();
        profile.avatar_hash = None;
        drop(profile);
        let _ = self.events.send(ProfileEvent::SelfUpdated);
        Ok(())
    }

    pub fn encode(&self) -> Result<Vec<u8>, ProfileError> {
        let profile = self.profile.read().unwrap_or_else(PoisonError::into_inner);
        profile.record(self.node_id)?.encode(&self.identity)
    }

    pub async fn serve(self: Arc<Self>, connection: Connection) {
        {
            let mut serving = self.serving.lock().unwrap_or_else(PoisonError::into_inner);
            if *serving >= MAX_CONCURRENT_SERVES {
                connection.close(0u32.into(), b"profile busy");
                return;
            }
            *serving += 1;
        }
        let result = tokio::time::timeout(IO_TIMEOUT, async {
            let (mut send, mut recv) = connection
                .accept_bi()
                .await
                .map_err(|error| ProfileError::Decode(error.to_string()))?;
            let mut request = [0_u8; REQUEST.len()];
            recv.read_exact(&mut request)
                .await
                .map_err(|error| ProfileError::Decode(error.to_string()))?;
            if &request != REQUEST {
                return Err(ProfileError::Decode("invalid request".to_owned()));
            }
            let payload = self.encode()?;
            let length = u32::try_from(payload.len()).map_err(|_| ProfileError::PayloadTooLarge)?;
            send.write_all(&length.to_be_bytes())
                .await
                .map_err(|error| ProfileError::Decode(error.to_string()))?;
            send.write_all(&payload)
                .await
                .map_err(|error| ProfileError::Decode(error.to_string()))?;
            send.finish()
                .map_err(|error| ProfileError::Decode(error.to_string()))?;
            let _ = tokio::time::timeout(IO_TIMEOUT, send.stopped()).await;
            Ok::<_, ProfileError>(())
        })
        .await;
        match result {
            Ok(Ok(())) => {}
            Ok(Err(error)) => tracing::debug!(%error, "profile request failed"),
            Err(_) => tracing::debug!("profile request timed out"),
        }
        let mut serving = self.serving.lock().unwrap_or_else(PoisonError::into_inner);
        *serving = serving.saturating_sub(1);
    }

    async fn fetch(&self, candidate: Candidate) {
        let Ok(fingerprint) = Fingerprint::from_str(candidate.fingerprint()) else {
            return;
        };
        let node_id = candidate.node_id().parse::<NodeId>().ok();
        let Some(address) = candidate.addresses().next() else {
            return;
        };
        let Ok(address) = address.parse() else {
            return;
        };
        self.fetch_from(
            fingerprint,
            node_id,
            address,
            candidate.avatar_hash(),
            candidate.node_id(),
        )
        .await;
    }

    async fn fetch_from(
        &self,
        fingerprint: Fingerprint,
        node_id: Option<NodeId>,
        address: std::net::SocketAddr,
        expected_avatar_hash: Option<&str>,
        expected_node_id: &str,
    ) {
        let target = DialTarget::new(address, fingerprint);
        let target = match node_id {
            Some(node_id) => target.with_node_id(node_id),
            None => target,
        };
        let Ok((connection, public_key)) = self.transport.connect_profile(target).await else {
            return;
        };
        let result = tokio::time::timeout(IO_TIMEOUT, read_profile(&connection, &public_key)).await;
        connection.close(0u32.into(), b"profile fetched");
        let Ok(Ok(record)) = result else {
            return;
        };
        if record.node_id != expected_node_id {
            return;
        }
        if let Some(expected) = expected_avatar_hash
            && record.avatar_hash_hex().as_deref() != Some(expected)
        {
            return;
        }
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |duration| duration.as_millis() as i64);
        let fingerprint = fingerprint.to_hex();
        let stored = UserProfile {
            fingerprint: fingerprint.clone(),
            node_id: Some(record.node_id.clone()),
            display_name: record.display_name.clone(),
            avatar_hash: record.avatar_hash_hex(),
            avatar_mime: (!record.avatar.is_empty()).then_some(record.avatar_mime.clone()),
            avatar: record.avatar,
            updated_at: now,
        };
        if self.database.upsert_user_profile(stored).await.is_ok() {
            let _ = self.events.send(ProfileEvent::UserUpdated { fingerprint });
        }
    }

    fn maybe_fetch(self: &Arc<Self>, candidate: Candidate) {
        let fingerprint = candidate.fingerprint().to_owned();
        {
            let mut in_flight = self
                .in_flight
                .lock()
                .unwrap_or_else(PoisonError::into_inner);
            if in_flight.len() >= 4 || !in_flight.insert(fingerprint.clone()) {
                return;
            }
        }
        let service = self.clone();
        tokio::spawn(async move {
            service.fetch(candidate).await;
            service
                .in_flight
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .remove(&fingerprint);
        });
    }

    pub async fn refresh_known(self: &Arc<Self>, candidates: Vec<Candidate>) {
        for candidate in candidates {
            let fingerprint = candidate.fingerprint().to_owned();
            let cached = self
                .database
                .get_user_profile(&fingerprint)
                .await
                .ok()
                .flatten();
            let needs = match &cached {
                Some(profile) => candidate
                    .avatar_hash()
                    .is_some_and(|hash| profile.avatar_hash.as_deref() != Some(hash)),
                None => true,
            };
            if needs {
                self.maybe_fetch(candidate);
            }
        }
    }
}

fn valid_avatar(bytes: &[u8]) -> bool {
    !bytes.is_empty() && bytes.len() <= MAX_AVATAR_BYTES && bytes.starts_with(b"\x89PNG\r\n\x1a\n")
}

fn write_private(path: &Path, bytes: &[u8]) -> io::Result<()> {
    #[cfg(unix)]
    {
        use std::io::Write;
        use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
        let mut options = std::fs::OpenOptions::new();
        options.create(true).truncate(true).write(true).mode(0o600);
        let mut file = options.open(path)?;
        file.write_all(bytes)?;
        file.sync_all()?;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))?;
    }
    #[cfg(not(unix))]
    std::fs::write(path, bytes)?;
    Ok(())
}

async fn read_profile(
    connection: &Connection,
    public_key: &[u8; 32],
) -> Result<ProfileRecord, ProfileError> {
    let (mut send, mut recv) = connection
        .open_bi()
        .await
        .map_err(|error| ProfileError::Decode(error.to_string()))?;
    send.write_all(REQUEST)
        .await
        .map_err(|error| ProfileError::Decode(error.to_string()))?;
    send.finish()
        .map_err(|error| ProfileError::Decode(error.to_string()))?;
    let mut length = [0_u8; 4];
    recv.read_exact(&mut length)
        .await
        .map_err(|error| ProfileError::Decode(error.to_string()))?;
    let length = u32::from_be_bytes(length) as usize;
    if length == 0 || length > MAX_RESPONSE {
        return Err(ProfileError::Decode("invalid response length".to_owned()));
    }
    let mut bytes = vec![0_u8; length];
    recv.read_exact(&mut bytes)
        .await
        .map_err(|error| ProfileError::Decode(error.to_string()))?;
    let key = ed25519_dalek::VerifyingKey::from_bytes(public_key)
        .map_err(|error| ProfileError::Decode(error.to_string()))?;
    ProfileRecord::decode(&bytes, &key)
}

pub fn spawn(
    connection: zbus::Connection,
    discovery: Arc<LanDiscovery>,
    profile: Arc<ProfileService>,
    transport: Arc<Transport>,
) -> tokio::task::JoinHandle<()> {
    let mut discovery_events = discovery.subscribe();
    let mut transport_events = transport.subscribe();
    let mut profile_events = profile.subscribe();
    let mut ticker = tokio::time::interval(Duration::from_secs(15));
    tokio::spawn(async move {
        loop {
            tokio::select! {
                _ = ticker.tick() => {
                    profile.refresh_known(discovery.snapshot()).await;
                }
                event = discovery_events.recv() => {
                    match event {
                        Ok(DiscoveryEvent::Found(candidate)) => profile.maybe_fetch(candidate),
                        Ok(DiscoveryEvent::Lost(_)) => {}
                        Err(broadcast::error::RecvError::Lagged(_)) => {}
                        Err(broadcast::error::RecvError::Closed) => break,
                    }
                }
                event = transport_events.recv() => {
                    match event {
                        Ok(TransportEvent::ProfileRequested { connection, .. }) => {
                            let service = profile.clone();
                            tokio::spawn(async move { service.serve(connection).await; });
                        }
                        Ok(TransportEvent::UserConnected(_)) => {
                            let service = profile.clone();
                            let candidates = discovery.snapshot();
                            tokio::spawn(async move { service.refresh_known(candidates).await; });
                        }
                        _ => {}
                    }
                }
                event = profile_events.recv() => {
                    match event {
                        Ok(ProfileEvent::UserUpdated { fingerprint }) => {
                            if let Err(error) = crate::service::emit_profile_event(&connection, &fingerprint).await {
                                tracing::debug!(%error, "failed to emit profile signal");
                            }
                        }
                        Ok(ProfileEvent::SelfUpdated) => {
                            if let Err(error) = crate::service::emit_self_profile_event(&connection).await {
                                tracing::debug!(%error, "failed to emit self profile signal");
                            }
                        }
                        Err(broadcast::error::RecvError::Lagged(_)) => {}
                        Err(broadcast::error::RecvError::Closed) => break,
                    }
                }
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::{Ipv4Addr, SocketAddr};

    use ticonnect_net::transport::{TransportConfig, TrustLookup};

    struct NoTrust;

    impl TrustLookup for NoTrust {
        fn is_trusted(&self, _fingerprint: &str) -> bool {
            false
        }
    }

    async fn bind_transport(
        key: IdentityKeyPair,
        node_id: NodeId,
        fingerprint: Fingerprint,
    ) -> Arc<Transport> {
        Arc::new(
            Transport::bind(TransportConfig {
                identity: key,
                node_id,
                fingerprint,
                bind_addr: SocketAddr::from((Ipv4Addr::LOCALHOST, 0)),
                trust: Arc::new(NoTrust),
            })
            .await
            .expect("transport"),
        )
    }

    #[tokio::test]
    async fn profile_is_fetched_without_trust() {
        let root = std::env::temp_dir().join(format!(
            "ticonnect-profile-test-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_or(0, |duration| duration.as_millis())
        ));
        let remote_dir = root.join("remote");
        let local_dir = root.join("local");
        std::fs::create_dir_all(&remote_dir).expect("remote dir");
        std::fs::create_dir_all(&local_dir).expect("local dir");

        let remote_key = IdentityKeyPair::from_secret_bytes([71_u8; 32]);
        let remote_node = NodeId::generate();
        let remote_fp = Fingerprint::of(&remote_key.verifying_key());
        let remote_transport = bind_transport(remote_key.clone(), remote_node, remote_fp).await;
        let remote_db = DatabaseHandle::spawn(remote_dir.join("remote.db"))
            .await
            .expect("remote db");
        let remote_service = Arc::new(
            ProfileService::load(
                &remote_dir,
                remote_key,
                remote_node,
                "Ada".to_owned(),
                remote_db,
                remote_transport.clone(),
            )
            .expect("remote profile"),
        );
        let avatar = b"\x89PNG\r\n\x1a\nprofile".to_vec();
        remote_service.set_avatar(avatar.clone()).expect("avatar");

        let mut events = remote_transport.subscribe();
        let serving = remote_service.clone();
        let server = tokio::spawn(async move {
            while let Ok(event) = events.recv().await {
                if let TransportEvent::ProfileRequested { connection, .. } = event {
                    let serving = serving.clone();
                    tokio::spawn(async move {
                        serving.serve(connection).await;
                    });
                }
            }
        });

        let local_key = IdentityKeyPair::from_secret_bytes([72_u8; 32]);
        let local_node = NodeId::generate();
        let local_fp = Fingerprint::of(&local_key.verifying_key());
        let local_transport = bind_transport(local_key.clone(), local_node, local_fp).await;
        let local_db = DatabaseHandle::spawn(local_dir.join("local.db"))
            .await
            .expect("local db");
        let local_service = ProfileService::load(
            &local_dir,
            local_key,
            local_node,
            "Local".to_owned(),
            local_db.clone(),
            local_transport.clone(),
        )
        .expect("local profile");

        local_service
            .fetch_from(
                remote_fp,
                Some(remote_node),
                remote_transport.local_addr().expect("remote addr"),
                None,
                &remote_node.to_string(),
            )
            .await;

        let stored = local_db
            .get_user_profile(&remote_fp.to_hex())
            .await
            .expect("profile")
            .expect("stored profile");
        assert_eq!(stored.display_name, "Ada");
        assert_eq!(stored.avatar, avatar);

        server.abort();
        local_transport.shutdown().await;
        remote_transport.shutdown().await;
        let _ = std::fs::remove_dir_all(root);
    }
}
