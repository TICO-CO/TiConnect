use std::collections::HashMap;
use std::sync::RwLock;
use std::time::{Duration, Instant};

use ticonnect_core::ipc;
use ticonnect_net::discovery::candidate::Candidate;
use ticonnect_net::discovery::{Discovery, DiscoveryConfig, LanDiscovery};
use ticonnect_settings::{SettingValue, Settings};
use zbus::interface;
use zbus::object_server::SignalEmitter;
use zbus::{DBusError, OwnedVariant, Str};

use std::sync::Arc;

use crate::attachments::{AttachmentService, AttachmentsError, TransferEvent, TransferSnapshot};
use crate::calls::{CallEvent, CallService};
use crate::identity::PublicIdentity;
use crate::messaging::{MessagingEvent, MessagingService};
use crate::pairing_service::{PairingService, StartPairingResponse};
use crate::remote::{RemoteEventMessage, RemoteService, RemoteSnapshot};
use crate::runtime::Shutdown;
use crate::storage::{DatabaseHandle, Peer, PeerPolicy, StorageError, StoredMessage};
use crate::trust::TrustStore;
use ticonnect_net::transport::Transport;

#[derive(Debug, DBusError)]
#[zbus(prefix = "org.ticonnect.Daemon.Error")]
pub enum ServiceError {
    Failed(String),
    NotFound(String),
    #[zbus(error)]
    Zbus(zbus::Error),
}

pub struct DaemonState {
    started_at: Instant,
    shutdown: Arc<Shutdown>,
    identity: PublicIdentity,
    settings: RwLock<Settings>,
    database: DatabaseHandle,
    discovery: Arc<LanDiscovery>,
    trust: Arc<TrustStore>,
    transport: Arc<Transport>,
    pairing: Arc<PairingService>,
    messaging: Arc<MessagingService>,
    attachments: Arc<AttachmentService>,
    calls: Arc<CallService>,
    remote: Arc<RemoteService>,
}

#[derive(Debug, thiserror::Error)]
pub enum DaemonInitError {
    #[error(transparent)]
    Trust(#[from] crate::trust::TrustError),

    #[error(transparent)]
    Transport(#[from] ticonnect_net::transport::TransportError),

    #[error(transparent)]
    Attachments(#[from] AttachmentsError),
}

impl DaemonState {
    pub async fn initialize(
        shutdown: Arc<Shutdown>,
        identity: crate::identity::Identity,
        settings: Settings,
        database: DatabaseHandle,
    ) -> Result<Self, DaemonInitError> {
        let public = identity.public();
        let trust = Arc::new(TrustStore::load(database.clone()).await?);
        let port = settings.transport_port();
        let discovery = Arc::new(LanDiscovery::new(discovery_config(
            &settings, &public, port,
        )));
        let transport = Arc::new(
            Transport::bind(ticonnect_net::transport::TransportConfig {
                identity: identity.key_pair().clone(),
                node_id: public.node_id(),
                fingerprint: public.fingerprint(),
                bind_addr: std::net::SocketAddr::from(([0, 0, 0, 0], port)),
                trust: trust.clone(),
            })
            .await?,
        );
        let pairing = Arc::new(PairingService::new(
            public.clone(),
            trust.clone(),
            transport.clone(),
            discovery.clone(),
        ));
        let messaging = Arc::new(MessagingService::new(
            identity.key_pair().clone(),
            public.node_id(),
            database.clone(),
            trust.clone(),
            transport.clone(),
            settings.clone(),
        ));
        let attachments = Arc::new(
            AttachmentService::new(
                database.clone(),
                settings
                    .data_dir()
                    .map_err(|error| {
                        DaemonInitError::Attachments(AttachmentsError::Invalid(error.to_string()))
                    })?
                    .join("attachments"),
                transport.clone(),
                settings.clone(),
            )
            .map_err(DaemonInitError::Attachments)?,
        );
        let calls = Arc::new(CallService::new(
            public.node_id(),
            database.clone(),
            trust.clone(),
            transport.clone(),
            settings.clone(),
            Arc::new(ticonnect_media::ringtone::RodioRingtone::new()),
        ));
        let remote = Arc::new(RemoteService::new(
            trust.clone(),
            transport.clone(),
            settings.clone(),
            Arc::new(crate::remote::DesktopRemoteHost::default()),
        ));
        Ok(Self::new(
            shutdown,
            public,
            settings,
            database,
            discovery,
            trust,
            transport,
            pairing,
            messaging,
            attachments,
            calls,
            remote,
        ))
    }

    #[allow(clippy::too_many_arguments)]
    pub fn new(
        shutdown: Arc<Shutdown>,
        identity: PublicIdentity,
        settings: Settings,
        database: DatabaseHandle,
        discovery: Arc<LanDiscovery>,
        trust: Arc<TrustStore>,
        transport: Arc<Transport>,
        pairing: Arc<PairingService>,
        messaging: Arc<MessagingService>,
        attachments: Arc<AttachmentService>,
        calls: Arc<CallService>,
        remote: Arc<RemoteService>,
    ) -> Self {
        Self {
            started_at: Instant::now(),
            shutdown,
            identity,
            settings: RwLock::new(settings),
            database,
            discovery,
            trust,
            transport,
            pairing,
            messaging,
            attachments,
            calls,
            remote,
        }
    }

    pub fn shutdown(&self) -> &Arc<Shutdown> {
        &self.shutdown
    }

    pub fn identity(&self) -> &PublicIdentity {
        &self.identity
    }

    pub fn database(&self) -> &DatabaseHandle {
        &self.database
    }

    pub fn discovery(&self) -> &Arc<LanDiscovery> {
        &self.discovery
    }

    pub fn trust(&self) -> &Arc<TrustStore> {
        &self.trust
    }

    pub fn transport(&self) -> &Arc<Transport> {
        &self.transport
    }

    pub fn pairing(&self) -> &Arc<PairingService> {
        &self.pairing
    }

    pub fn messaging(&self) -> &Arc<MessagingService> {
        &self.messaging
    }

    pub fn attachments(&self) -> &Arc<AttachmentService> {
        &self.attachments
    }

    pub fn calls(&self) -> &Arc<CallService> {
        &self.calls
    }

    pub fn remote(&self) -> &Arc<RemoteService> {
        &self.remote
    }

    pub fn settings(&self) -> Settings {
        self.settings
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }

    pub fn replace_settings(&self, settings: Settings) {
        self.messaging.apply_settings(settings.clone());
        self.attachments.apply_settings(settings.clone());
        self.calls.apply_settings(settings.clone());
        self.remote.apply_settings(settings.clone());
        *self
            .settings
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = settings;
    }

    pub async fn reconfigure_discovery(&self) {
        let settings = self.settings();
        let config = discovery_config(&settings, &self.identity, settings.transport_port());
        if let Err(error) = self.discovery.reconfigure(config).await {
            tracing::warn!(%error, "failed to reconfigure discovery");
        }
    }
}

pub struct DaemonService {
    state: Arc<DaemonState>,
}

impl DaemonService {
    pub fn new(state: Arc<DaemonState>) -> Self {
        Self { state }
    }

    async fn status(&self) -> Result<HashMap<String, OwnedVariant>, ServiceError> {
        let peer_count = self
            .state
            .database()
            .count_peers()
            .await
            .map_err(database_error)?;
        let outbox_depth = self
            .state
            .database()
            .outbox_depth()
            .await
            .map_err(database_error)?;

        let sessions = self.state.transport().session_count();
        let transport_state = if sessions > 0 {
            ipc::TRANSPORT_ONLINE
        } else {
            ipc::TRANSPORT_IDLE
        };

        let mut status = HashMap::new();
        status.insert(
            ipc::STATUS_UPTIME_SECONDS.to_owned(),
            OwnedVariant::from(self.state.started_at.elapsed().as_secs()),
        );
        status.insert(
            ipc::STATUS_PEER_COUNT.to_owned(),
            OwnedVariant::from(peer_count),
        );
        status.insert(
            ipc::STATUS_OUTBOX_DEPTH.to_owned(),
            OwnedVariant::from(outbox_depth),
        );
        status.insert(
            ipc::STATUS_TRANSPORT_STATE.to_owned(),
            OwnedVariant::from(Str::from(transport_state)),
        );
        status.insert(
            ipc::STATUS_ACTIVE_SESSIONS.to_owned(),
            OwnedVariant::from(u32::try_from(sessions).unwrap_or_default()),
        );
        Ok(status)
    }

    fn identity(&self) -> HashMap<String, OwnedVariant> {
        let settings = self.state.settings();
        let identity = self.state.identity();
        HashMap::from([
            (
                ipc::IDENTITY_NODE_ID.to_owned(),
                string_variant(identity.node_id().to_string()),
            ),
            (
                ipc::IDENTITY_FINGERPRINT.to_owned(),
                string_variant(identity.fingerprint().to_hex()),
            ),
            (
                ipc::IDENTITY_DISPLAY_NAME.to_owned(),
                string_variant(settings.identity_display_name().to_owned()),
            ),
        ])
    }
}

#[interface(name = "org.ticonnect.Daemon")]
impl DaemonService {
    fn ping(&self) -> &'static str {
        "pong"
    }

    fn get_version(&self) -> &'static str {
        ipc::VERSION
    }

    async fn get_status(&self) -> Result<HashMap<String, OwnedVariant>, ServiceError> {
        self.status().await
    }

    async fn get_identity(&self) -> HashMap<String, OwnedVariant> {
        self.identity()
    }

    async fn get_setting(&self, key: &str) -> Result<OwnedVariant, ServiceError> {
        let settings = self.state.settings();
        match settings.get_setting(key) {
            Ok(value) => Ok(setting_variant(value)),
            Err(ticonnect_settings::Error::UnknownSetting(_)) => {
                Err(ServiceError::NotFound(format!("unknown setting: {key}")))
            }
            Err(error) => Err(ServiceError::Failed(error.to_string())),
        }
    }

    async fn list_settings(&self) -> Result<HashMap<String, OwnedVariant>, ServiceError> {
        let settings = self.state.settings();
        settings
            .list_settings()
            .map(|values| {
                values
                    .into_iter()
                    .map(|(key, value)| (key, setting_variant(value)))
                    .collect()
            })
            .map_err(|error| ServiceError::Failed(error.to_string()))
    }

    async fn list_peers(&self) -> Result<Vec<HashMap<String, OwnedVariant>>, ServiceError> {
        let peers = self
            .state
            .database()
            .list_peers()
            .await
            .map_err(database_error)?;
        Ok(peers
            .into_iter()
            .map(|peer| {
                let connected = peer_is_connected(&self.state, &peer.id);
                peer_variant(peer, connected)
            })
            .collect())
    }

    async fn list_candidates(&self) -> Vec<HashMap<String, OwnedVariant>> {
        self.state
            .discovery()
            .snapshot()
            .into_iter()
            .map(candidate_variant)
            .collect()
    }

    async fn get_discovery_status(&self) -> HashMap<String, OwnedVariant> {
        let status = self.state.discovery().status();
        HashMap::from([
            (
                ipc::DISCOVERY_ENABLED.to_owned(),
                OwnedVariant::from(status.enabled),
            ),
            (
                ipc::DISCOVERY_MDNS_ENABLED.to_owned(),
                OwnedVariant::from(status.mdns_enabled),
            ),
            (
                ipc::DISCOVERY_BROADCAST_ENABLED.to_owned(),
                OwnedVariant::from(status.broadcast_enabled),
            ),
            (
                ipc::DISCOVERY_MDNS_ACTIVE.to_owned(),
                OwnedVariant::from(status.mdns_active),
            ),
            (
                ipc::DISCOVERY_BROADCAST_ACTIVE.to_owned(),
                OwnedVariant::from(status.broadcast_active),
            ),
            (
                ipc::DISCOVERY_CANDIDATE_COUNT.to_owned(),
                OwnedVariant::from(u32::try_from(status.candidate_count).unwrap_or_default()),
            ),
            (
                ipc::DISCOVERY_DROPPED_PACKETS.to_owned(),
                OwnedVariant::from(status.dropped_packets),
            ),
        ])
    }

    async fn start_pairing(&self) -> Result<HashMap<String, OwnedVariant>, ServiceError> {
        let started = self
            .state
            .pairing()
            .start_pairing()
            .await
            .map_err(pairing_error)?;
        Ok(start_pairing_variant(&started))
    }

    async fn pair_with_pin(&self, node_id: &str, pin: &str) -> Result<bool, ServiceError> {
        self.state
            .pairing()
            .pair_with_pin(node_id, pin)
            .await
            .map_err(pairing_error)?;
        Ok(true)
    }

    async fn pair_with_qr(&self, payload: &str) -> Result<bool, ServiceError> {
        self.state
            .pairing()
            .pair_with_qr(payload)
            .await
            .map_err(pairing_error)?;
        Ok(true)
    }

    async fn cancel_pairing(&self) -> Result<(), ServiceError> {
        self.state.pairing().cancel_pairing().await;
        Ok(())
    }

    async fn forget_peer(&self, peer_id: &str) -> Result<bool, ServiceError> {
        self.state
            .pairing()
            .forget_peer(peer_id)
            .await
            .map_err(pairing_error)?;
        Ok(true)
    }

    async fn list_trusted_peers(&self) -> Result<Vec<HashMap<String, OwnedVariant>>, ServiceError> {
        let peers = self
            .state
            .pairing()
            .list_trusted_peers()
            .await
            .map_err(pairing_error)?;
        Ok(peers
            .into_iter()
            .map(|peer| {
                let connected = peer_is_connected(&self.state, &peer.id);
                peer_variant(peer, connected)
            })
            .collect())
    }

    async fn send_message(&self, peer_id: &str, body: &str) -> Result<String, ServiceError> {
        let message_id = self
            .state
            .messaging()
            .send_message(peer_id, body)
            .await
            .map_err(messaging_error)?;
        Ok(message_id.to_string())
    }

    async fn list_messages(
        &self,
        conversation_id: &str,
        limit: u32,
        offset: u32,
    ) -> Result<Vec<HashMap<String, OwnedVariant>>, ServiceError> {
        let messages = self
            .state
            .messaging()
            .list_messages(conversation_id, limit, offset)
            .await
            .map_err(messaging_error)?;
        let mut values = Vec::with_capacity(messages.len());
        for message in messages {
            let mut variant = stored_message_variant(message);
            let hash = variant
                .get(ipc::MESSAGE_ATTACHMENT_HASH)
                .map(display_variant_string)
                .unwrap_or_default();
            if !hash.is_empty()
                && let Ok(Some(path)) = self.state.attachments().path(&hash).await
            {
                variant.insert(
                    ipc::MESSAGE_ATTACHMENT_PATH.to_owned(),
                    string_variant(path.display().to_string()),
                );
            }
            values.push(variant);
        }
        Ok(values)
    }

    async fn count_messages(&self, conversation_id: &str) -> Result<u32, ServiceError> {
        self.state
            .messaging()
            .count_messages(conversation_id)
            .await
            .map_err(messaging_error)
    }

    async fn mark_read(
        &self,
        conversation_id: &str,
        message_ids: Vec<String>,
    ) -> Result<(), ServiceError> {
        self.state
            .messaging()
            .mark_read(conversation_id, &message_ids)
            .await
            .map_err(messaging_error)
    }

    async fn get_message_state(&self, message_id: &str) -> Result<String, ServiceError> {
        let state = self
            .state
            .messaging()
            .message_state(message_id)
            .await
            .map_err(messaging_error)?;
        Ok(state.as_str().to_owned())
    }

    async fn set_peer_policy(&self, peer_id: &str, policy: &str) -> Result<(), ServiceError> {
        let policy = policy
            .parse::<PeerPolicy>()
            .map_err(|error| ServiceError::Failed(error.to_string()))?;
        self.state
            .messaging()
            .set_peer_policy(peer_id, policy)
            .await
            .map_err(messaging_error)
    }

    async fn start_call(&self, peer_id: &str) -> Result<String, ServiceError> {
        let call = self
            .state
            .calls()
            .start_call(peer_id)
            .await
            .map_err(call_error)?;
        Ok(call.id.to_string())
    }

    async fn accept_call(&self, call_id: &str) -> Result<(), ServiceError> {
        let call_id = parse_call_id(call_id)?;
        self.state
            .calls()
            .accept_call(call_id)
            .await
            .map_err(call_error)?;
        Ok(())
    }

    async fn decline_call(&self, call_id: &str) -> Result<(), ServiceError> {
        let call_id = parse_call_id(call_id)?;
        self.state
            .calls()
            .decline_call(call_id)
            .await
            .map_err(call_error)
    }

    async fn hangup_call(&self, call_id: &str) -> Result<(), ServiceError> {
        let call_id = parse_call_id(call_id)?;
        self.state
            .calls()
            .hangup_call(call_id)
            .await
            .map_err(call_error)
    }

    async fn get_call_metrics(&self) -> HashMap<String, OwnedVariant> {
        let metrics = self.state.calls().metrics();
        HashMap::from([
            (
                ipc::CALL_FRAMES_SENT.to_owned(),
                OwnedVariant::from(metrics.frames_sent),
            ),
            (
                ipc::CALL_FRAMES_RECEIVED.to_owned(),
                OwnedVariant::from(metrics.frames_received),
            ),
            (
                ipc::CALL_PACKETS_LOST.to_owned(),
                OwnedVariant::from(metrics.packets_lost),
            ),
            (
                ipc::CALL_BYTES_SENT.to_owned(),
                OwnedVariant::from(metrics.bytes_sent),
            ),
        ])
    }

    async fn get_call_state(&self) -> HashMap<String, OwnedVariant> {
        self.state
            .calls()
            .state()
            .map_or_else(HashMap::new, |call| call_variant(&call))
    }

    async fn request_remote_session(
        &self,
        peer_id: &str,
    ) -> Result<HashMap<String, OwnedVariant>, ServiceError> {
        let snapshot = self
            .state
            .remote()
            .request_session(peer_id)
            .await
            .map_err(remote_error)?;
        Ok(remote_variant(&snapshot))
    }

    async fn accept_remote_session(&self, session_id: &str, pin: &str) -> Result<(), ServiceError> {
        self.state
            .remote()
            .accept_session(session_id, pin)
            .await
            .map_err(remote_error)?;
        Ok(())
    }

    async fn deny_remote_session(&self, session_id: &str) -> Result<(), ServiceError> {
        self.state
            .remote()
            .deny_session(session_id)
            .await
            .map_err(remote_error)
    }

    async fn end_remote_session(&self, session_id: &str) -> Result<(), ServiceError> {
        self.state
            .remote()
            .end_session(session_id)
            .await
            .map_err(remote_error)
    }

    async fn get_remote_session(&self) -> HashMap<String, OwnedVariant> {
        self.state
            .remote()
            .snapshot()
            .map_or_else(HashMap::new, |snapshot| remote_variant(&snapshot))
    }

    async fn send_attachment(
        &self,
        peer_id: &str,
        path: &str,
        mime: &str,
    ) -> Result<String, ServiceError> {
        let attachment = self
            .state
            .attachments()
            .prepare_file(std::path::Path::new(path), Some(mime))
            .await
            .map_err(attachment_error)?;
        self.state
            .attachments()
            .record_local(&attachment)
            .await
            .map_err(attachment_error)?;
        let message_id = self
            .state
            .messaging()
            .send_attachment(peer_id, &attachment)
            .await
            .map_err(messaging_error)?;
        Ok(message_id.to_string())
    }

    async fn get_transfer_state(
        &self,
        hash: &str,
    ) -> Result<HashMap<String, OwnedVariant>, ServiceError> {
        let state = self
            .state
            .attachments()
            .transfer_state(hash)
            .ok_or_else(|| ServiceError::NotFound(format!("unknown transfer: {hash}")))?;
        Ok(transfer_variant(&state))
    }

    async fn cancel_transfer(&self, hash: &str) -> Result<(), ServiceError> {
        if !self.state.attachments().cancel(hash) {
            return Err(ServiceError::NotFound(format!("unknown transfer: {hash}")));
        }
        Ok(())
    }

    async fn save_attachment(&self, hash: &str, destination: &str) -> Result<bool, ServiceError> {
        self.state
            .attachments()
            .save(hash, std::path::Path::new(destination))
            .await
            .map_err(attachment_error)
    }

    async fn get_attachment_path(&self, hash: &str) -> Result<String, ServiceError> {
        self.state
            .attachments()
            .path(hash)
            .await
            .map_err(attachment_error)?
            .map(|path| path.display().to_string())
            .ok_or_else(|| ServiceError::NotFound(format!("unknown attachment: {hash}")))
    }

    async fn set_dnd(&self, enabled: bool) -> Result<(), ServiceError> {
        self.state
            .messaging()
            .set_dnd(enabled)
            .await
            .map_err(messaging_error)?;
        self.state
            .replace_settings(self.state.messaging().settings());
        Ok(())
    }

    fn shutdown(&self) -> Result<(), ServiceError> {
        if self.state.shutdown().is_requested() {
            return Err(ServiceError::Failed(
                "daemon is already shutting down".to_owned(),
            ));
        }
        tracing::info!("daemon shutdown requested over D-Bus");
        self.state.shutdown().request();
        Ok(())
    }

    #[zbus(property)]
    fn version(&self) -> &'static str {
        ipc::VERSION
    }

    #[zbus(signal)]
    pub async fn daemon_started(emitter: SignalEmitter<'_>) -> zbus::Result<()>;

    #[zbus(signal)]
    pub async fn daemon_stopping(emitter: SignalEmitter<'_>) -> zbus::Result<()>;

    #[zbus(signal)]
    pub async fn settings_changed(emitter: SignalEmitter<'_>, key: &str) -> zbus::Result<()>;

    #[zbus(signal)]
    pub async fn candidate_found(
        emitter: SignalEmitter<'_>,
        candidate: HashMap<String, OwnedVariant>,
    ) -> zbus::Result<()>;

    #[zbus(signal)]
    pub async fn candidate_lost(emitter: SignalEmitter<'_>, fingerprint: &str) -> zbus::Result<()>;

    #[zbus(signal)]
    pub async fn pairing_started(
        emitter: SignalEmitter<'_>,
        pairing: HashMap<String, OwnedVariant>,
    ) -> zbus::Result<()>;

    #[zbus(signal)]
    pub async fn pairing_completed(
        emitter: SignalEmitter<'_>,
        peer_id: &str,
        fingerprint: &str,
    ) -> zbus::Result<()>;

    #[zbus(signal)]
    pub async fn pairing_failed(emitter: SignalEmitter<'_>, reason: &str) -> zbus::Result<()>;

    #[zbus(signal)]
    pub async fn peer_connected(
        emitter: SignalEmitter<'_>,
        peer: HashMap<String, OwnedVariant>,
    ) -> zbus::Result<()>;

    #[zbus(signal)]
    pub async fn peer_disconnected(
        emitter: SignalEmitter<'_>,
        peer: HashMap<String, OwnedVariant>,
    ) -> zbus::Result<()>;

    #[zbus(signal)]
    pub async fn message_received(
        emitter: SignalEmitter<'_>,
        message: HashMap<String, OwnedVariant>,
    ) -> zbus::Result<()>;

    #[zbus(signal)]
    pub async fn message_state_changed(
        emitter: SignalEmitter<'_>,
        message_id: &str,
        state: &str,
    ) -> zbus::Result<()>;

    #[zbus(signal)]
    pub async fn sync_completed(
        emitter: SignalEmitter<'_>,
        peer_id: &str,
        count: u32,
    ) -> zbus::Result<()>;

    #[zbus(signal)]
    pub async fn policy_changed(
        emitter: SignalEmitter<'_>,
        peer_id: &str,
        policy: &str,
    ) -> zbus::Result<()>;

    #[zbus(signal)]
    pub async fn remote_session_requested(
        emitter: SignalEmitter<'_>,
        session: HashMap<String, OwnedVariant>,
    ) -> zbus::Result<()>;

    #[zbus(signal)]
    pub async fn remote_session_started(
        emitter: SignalEmitter<'_>,
        session_id: &str,
    ) -> zbus::Result<()>;

    #[zbus(signal)]
    pub async fn remote_session_ended(
        emitter: SignalEmitter<'_>,
        session_id: &str,
        reason: &str,
    ) -> zbus::Result<()>;

    #[zbus(signal)]
    pub async fn remote_indicator_changed(
        emitter: SignalEmitter<'_>,
        session_id: &str,
        visible: bool,
    ) -> zbus::Result<()>;

    #[zbus(signal)]
    pub async fn incoming_call(
        emitter: SignalEmitter<'_>,
        call: HashMap<String, OwnedVariant>,
    ) -> zbus::Result<()>;

    #[zbus(signal)]
    pub async fn call_state_changed(
        emitter: SignalEmitter<'_>,
        call_id: &str,
        state: &str,
    ) -> zbus::Result<()>;

    #[zbus(signal)]
    pub async fn call_ended(
        emitter: SignalEmitter<'_>,
        call_id: &str,
        reason: &str,
    ) -> zbus::Result<()>;

    #[zbus(signal)]
    pub async fn transfer_progress(
        emitter: SignalEmitter<'_>,
        hash: &str,
        bytes: u64,
        total: u64,
    ) -> zbus::Result<()>;

    #[zbus(signal)]
    pub async fn transfer_done(emitter: SignalEmitter<'_>, hash: &str) -> zbus::Result<()>;

    #[zbus(signal)]
    pub async fn transfer_failed(
        emitter: SignalEmitter<'_>,
        hash: &str,
        reason: &str,
    ) -> zbus::Result<()>;
}

pub(crate) async fn emit_messaging_event(
    connection: &zbus::Connection,
    event: MessagingEvent,
) -> zbus::Result<()> {
    let emitter = SignalEmitter::new(connection, ipc::DAEMON_PATH)?;
    match event {
        MessagingEvent::MessageReceived {
            peer_id,
            message,
            notify,
        } => {
            DaemonService::message_received(
                emitter,
                stored_message_variant_with(peer_id, message, notify),
            )
            .await
        }
        MessagingEvent::MessageStateChanged { message_id, state } => {
            DaemonService::message_state_changed(emitter, &message_id, state.as_str()).await
        }
        MessagingEvent::SyncCompleted { peer_id, count } => {
            DaemonService::sync_completed(emitter, &peer_id, count).await
        }
        MessagingEvent::PolicyChanged { peer_id, policy } => {
            DaemonService::policy_changed(emitter, &peer_id, policy.as_str()).await
        }
    }
}

fn stored_message_variant(message: StoredMessage) -> HashMap<String, OwnedVariant> {
    stored_message_variant_with(String::new(), message, false)
}

fn stored_message_variant_with(
    peer_id: String,
    message: StoredMessage,
    notify: bool,
) -> HashMap<String, OwnedVariant> {
    let record = message.record;
    HashMap::from([
        (
            ipc::MESSAGE_ID.to_owned(),
            string_variant(record.message_id.to_string()),
        ),
        (
            ipc::MESSAGE_CONVERSATION_ID.to_owned(),
            string_variant(record.conversation_id),
        ),
        (ipc::MESSAGE_PEER_ID.to_owned(), string_variant(peer_id)),
        (
            ipc::MESSAGE_SENDER_ID.to_owned(),
            string_variant(record.sender_id),
        ),
        (
            ipc::MESSAGE_SEQ.to_owned(),
            OwnedVariant::from(u32::try_from(record.seq).unwrap_or_default()),
        ),
        (
            ipc::MESSAGE_SENT_AT.to_owned(),
            OwnedVariant::from(record.sent_at),
        ),
        (
            ipc::MESSAGE_RECEIVED_AT.to_owned(),
            OwnedVariant::from(message.received_at),
        ),
        (
            ipc::MESSAGE_KIND.to_owned(),
            string_variant(record.kind.as_str().to_owned()),
        ),
        (
            ipc::MESSAGE_TARGET_ID.to_owned(),
            string_variant(record.target_id.unwrap_or_default()),
        ),
        (
            ipc::MESSAGE_BODY.to_owned(),
            string_variant(record.body.unwrap_or_default()),
        ),
        (
            ipc::MESSAGE_ATTACHMENT_HASH.to_owned(),
            string_variant(record.attachment_hash.clone().unwrap_or_default()),
        ),
        (
            ipc::MESSAGE_ATTACHMENT_NAME.to_owned(),
            string_variant(record.attachment_name.clone().unwrap_or_default()),
        ),
        (
            ipc::MESSAGE_ATTACHMENT_SIZE.to_owned(),
            OwnedVariant::from(record.attachment_size.unwrap_or_default()),
        ),
        (
            ipc::MESSAGE_ATTACHMENT_MIME.to_owned(),
            string_variant(record.attachment_mime.clone().unwrap_or_default()),
        ),
        (
            ipc::MESSAGE_STATE.to_owned(),
            string_variant(
                message
                    .state
                    .map_or_else(String::new, |state| state.as_str().to_owned()),
            ),
        ),
        (ipc::MESSAGE_NOTIFY.to_owned(), OwnedVariant::from(notify)),
    ])
}

fn display_variant_string(value: &OwnedVariant) -> String {
    <&str>::try_from(value).map_or_else(|_| String::new(), str::to_owned)
}

fn setting_variant(value: SettingValue) -> OwnedVariant {
    match value {
        SettingValue::String(value) => string_variant(value),
        SettingValue::Bool(value) => OwnedVariant::from(value),
        SettingValue::Integer(value) => OwnedVariant::from(value),
        SettingValue::Path(Some(path)) => string_variant(path.display().to_string()),
        SettingValue::Path(None) => string_variant(String::new()),
    }
}

fn string_variant(value: String) -> OwnedVariant {
    OwnedVariant::from(Str::from(value))
}

fn peer_variant(peer: Peer, connected: bool) -> HashMap<String, OwnedVariant> {
    HashMap::from([
        (ipc::PEER_ID.to_owned(), string_variant(peer.id)),
        (
            ipc::PEER_FINGERPRINT.to_owned(),
            string_variant(peer.fingerprint),
        ),
        (
            ipc::PEER_DISPLAY_NAME.to_owned(),
            string_variant(peer.display_name.unwrap_or_default()),
        ),
        (
            ipc::PEER_POLICY.to_owned(),
            string_variant(peer.policy.as_str().to_owned()),
        ),
        (
            ipc::PEER_TRUSTED_AT.to_owned(),
            OwnedVariant::from(u64::try_from(peer.trusted_at).unwrap_or_default()),
        ),
        (
            ipc::PEER_LAST_SEEN.to_owned(),
            OwnedVariant::from(
                u64::try_from(peer.last_seen.unwrap_or_default()).unwrap_or_default(),
            ),
        ),
        (
            ipc::PEER_CONNECTED.to_owned(),
            OwnedVariant::from(connected),
        ),
    ])
}

fn peer_is_connected(state: &DaemonState, peer_id: &str) -> bool {
    peer_id
        .parse::<ticonnect_core::id::NodeId>()
        .ok()
        .and_then(|node_id| state.transport().session_by_node_id(node_id))
        .is_some()
}

pub(crate) fn candidate_variant(candidate: Candidate) -> HashMap<String, OwnedVariant> {
    HashMap::from([
        (
            ipc::CANDIDATE_NODE_ID.to_owned(),
            string_variant(candidate.node_id().to_owned()),
        ),
        (
            ipc::CANDIDATE_FINGERPRINT.to_owned(),
            string_variant(candidate.fingerprint().to_owned()),
        ),
        (
            ipc::CANDIDATE_DISPLAY_NAME.to_owned(),
            string_variant(candidate.display_name().to_owned()),
        ),
        (
            ipc::CANDIDATE_ADDRESSES.to_owned(),
            string_variant(candidate.addresses_display()),
        ),
        (
            ipc::CANDIDATE_PORT.to_owned(),
            OwnedVariant::from(u32::from(candidate.port())),
        ),
        (
            ipc::CANDIDATE_PROTOCOL_VERSION.to_owned(),
            OwnedVariant::from(u32::from(candidate.protocol_version())),
        ),
        (
            ipc::CANDIDATE_SOURCE.to_owned(),
            string_variant(candidate.source().as_str().to_owned()),
        ),
        (
            ipc::CANDIDATE_LAST_SEEN.to_owned(),
            OwnedVariant::from(candidate.last_seen_unix_ms()),
        ),
    ])
}

pub fn discovery_config(
    settings: &Settings,
    identity: &PublicIdentity,
    transport_port: u16,
) -> DiscoveryConfig {
    let mut config = DiscoveryConfig::new(
        settings.identity_display_name(),
        identity.node_id().to_string(),
        identity.fingerprint().to_hex(),
    );
    config.enabled = settings.discovery_enabled();
    config.mdns = settings.discovery_mdns();
    config.broadcast = settings.discovery_broadcast();
    config.transport_port = transport_port;
    config.beacon_interval =
        Duration::from_secs(u64::from(settings.discovery_beacon_interval_secs()));
    config
}

fn start_pairing_variant(started: &StartPairingResponse) -> HashMap<String, OwnedVariant> {
    HashMap::from([
        (
            ipc::PAIRING_PIN.to_owned(),
            string_variant(started.pin.clone()),
        ),
        (
            ipc::PAIRING_QR.to_owned(),
            string_variant(started.qr.clone()),
        ),
        (
            ipc::PAIRING_EXPIRES_AT.to_owned(),
            OwnedVariant::from(started.expires_at),
        ),
        (
            ipc::PAIRING_NODE_ID.to_owned(),
            string_variant(started.node_id.clone()),
        ),
        (
            ipc::PAIRING_PORT.to_owned(),
            OwnedVariant::from(u32::from(started.port)),
        ),
    ])
}

pub(crate) async fn emit_transfer_event(
    connection: &zbus::Connection,
    event: TransferEvent,
) -> zbus::Result<()> {
    let emitter = SignalEmitter::new(connection, ipc::DAEMON_PATH)?;
    match event {
        TransferEvent::Progress(snapshot) => {
            DaemonService::transfer_progress(emitter, &snapshot.hash, snapshot.bytes, snapshot.size)
                .await
        }
        TransferEvent::Done { hash } => DaemonService::transfer_done(emitter, &hash).await,
        TransferEvent::Failed { hash, reason } => {
            DaemonService::transfer_failed(emitter, &hash, &reason).await
        }
    }
}

pub(crate) async fn emit_remote_event(
    connection: &zbus::Connection,
    event: RemoteEventMessage,
) -> zbus::Result<()> {
    let emitter = SignalEmitter::new(connection, ipc::DAEMON_PATH)?;
    match event {
        RemoteEventMessage::Requested(snapshot) => {
            DaemonService::remote_session_requested(emitter, remote_variant(&snapshot)).await
        }
        RemoteEventMessage::Started(snapshot) => {
            DaemonService::remote_session_started(emitter, &snapshot.session_id).await
        }
        RemoteEventMessage::Ended { session_id, reason } => {
            DaemonService::remote_session_ended(emitter, &session_id, &reason).await
        }
        RemoteEventMessage::Indicator {
            session_id,
            visible,
        } => DaemonService::remote_indicator_changed(emitter, &session_id, visible).await,
    }
}

fn remote_variant(snapshot: &RemoteSnapshot) -> HashMap<String, OwnedVariant> {
    HashMap::from([
        (
            ipc::REMOTE_SESSION_ID.to_owned(),
            string_variant(snapshot.session_id.clone()),
        ),
        (
            ipc::REMOTE_PEER_ID.to_owned(),
            string_variant(snapshot.peer_id.clone()),
        ),
        (
            ipc::REMOTE_FINGERPRINT.to_owned(),
            string_variant(snapshot.fingerprint.clone()),
        ),
        (
            ipc::REMOTE_ROLE.to_owned(),
            string_variant(snapshot.role.clone()),
        ),
        (
            ipc::REMOTE_STATE.to_owned(),
            string_variant(snapshot.state.clone()),
        ),
        (
            ipc::REMOTE_PIN.to_owned(),
            string_variant(snapshot.pin.clone()),
        ),
        (
            ipc::REMOTE_INDICATOR.to_owned(),
            OwnedVariant::from(snapshot.indicator),
        ),
        (
            ipc::REMOTE_EXPIRES_AT.to_owned(),
            OwnedVariant::from(snapshot.expires_at),
        ),
    ])
}

fn remote_error(error: crate::remote::RemoteError) -> ServiceError {
    tracing::warn!(%error, "remote desktop request failed");
    ServiceError::Failed(error.to_string())
}

pub(crate) async fn emit_call_event(
    connection: &zbus::Connection,
    event: CallEvent,
) -> zbus::Result<()> {
    let emitter = SignalEmitter::new(connection, ipc::DAEMON_PATH)?;
    match event {
        CallEvent::IncomingCall(call) => {
            DaemonService::incoming_call(emitter, call_variant(&call)).await
        }
        CallEvent::StateChanged(call) => {
            DaemonService::call_state_changed(emitter, &call.id.to_string(), call.state.as_str())
                .await
        }
        CallEvent::Ended {
            call_id, reason, ..
        } => DaemonService::call_ended(emitter, &call_id.to_string(), &reason).await,
    }
}

fn call_variant(call: &crate::calls::state::CallInfo) -> HashMap<String, OwnedVariant> {
    HashMap::from([
        (ipc::CALL_ID.to_owned(), string_variant(call.id.to_string())),
        (
            ipc::CALL_PEER_ID.to_owned(),
            string_variant(call.peer_id.clone()),
        ),
        (
            ipc::CALL_DIRECTION.to_owned(),
            string_variant(
                match call.direction {
                    ticonnect_core::call::CallDirection::Incoming => "incoming",
                    ticonnect_core::call::CallDirection::Outgoing => "outgoing",
                }
                .to_owned(),
            ),
        ),
        (
            ipc::CALL_STATE.to_owned(),
            string_variant(call.state.as_str().to_owned()),
        ),
        (
            ipc::CALL_STARTED_AT.to_owned(),
            OwnedVariant::from(call.created_at),
        ),
        (
            ipc::CALL_CONNECTED_AT.to_owned(),
            OwnedVariant::from(call.connected_at.unwrap_or_default()),
        ),
    ])
}

fn parse_call_id(value: &str) -> Result<ticonnect_core::call::CallId, ServiceError> {
    value
        .parse()
        .map_err(|_| ServiceError::Failed(format!("invalid call id: {value}")))
}

fn call_error(error: crate::calls::CallError) -> ServiceError {
    tracing::warn!(%error, "call request failed");
    ServiceError::Failed(error.to_string())
}

fn transfer_variant(snapshot: &TransferSnapshot) -> HashMap<String, OwnedVariant> {
    HashMap::from([
        (
            ipc::TRANSFER_HASH.to_owned(),
            string_variant(snapshot.hash.clone()),
        ),
        (
            ipc::TRANSFER_PEER_ID.to_owned(),
            string_variant(snapshot.peer_id.clone()),
        ),
        (
            ipc::TRANSFER_NAME.to_owned(),
            string_variant(snapshot.name.clone()),
        ),
        (
            ipc::TRANSFER_SIZE.to_owned(),
            OwnedVariant::from(snapshot.size),
        ),
        (
            ipc::TRANSFER_BYTES.to_owned(),
            OwnedVariant::from(snapshot.bytes),
        ),
        (
            ipc::TRANSFER_PHASE.to_owned(),
            string_variant(snapshot.phase.as_str().to_owned()),
        ),
        (
            ipc::TRANSFER_REASON.to_owned(),
            string_variant(snapshot.reason.clone()),
        ),
    ])
}

fn attachment_error(error: AttachmentsError) -> ServiceError {
    tracing::warn!(%error, "attachment request failed");
    ServiceError::Failed(error.to_string())
}

fn messaging_error(error: crate::messaging::MessagingError) -> ServiceError {
    tracing::warn!(%error, "messaging request failed");
    ServiceError::Failed(error.to_string())
}

fn pairing_error(error: crate::pairing_service::PairingServiceError) -> ServiceError {
    tracing::warn!(%error, "pairing request failed");
    ServiceError::Failed(error.to_string())
}

fn database_error(error: StorageError) -> ServiceError {
    tracing::error!(%error, "database request failed");
    ServiceError::Failed("database request failed".to_owned())
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use std::net::{IpAddr, Ipv4Addr, SocketAddr};

    use super::*;
    use crate::identity::Identity;
    use crate::storage::DatabaseHandle;
    use ticonnect_net::transport::TransportConfig;

    #[tokio::test]
    async fn ping_and_version_are_stable() {
        let (service, _dir) = test_service().await;
        assert_eq!(service.ping(), "pong");
        assert_eq!(service.get_version(), ipc::VERSION);
        assert_eq!(service.version(), ipc::VERSION);
    }

    #[tokio::test]
    async fn status_has_stable_keys_and_values() {
        let (service, _dir) = test_service().await;
        let status = service.status().await.expect("status");

        assert!(status.contains_key(ipc::STATUS_UPTIME_SECONDS));
        assert_eq!(
            u32::try_from(&status[ipc::STATUS_PEER_COUNT]).expect("peer count"),
            0
        );
        assert_eq!(
            u32::try_from(&status[ipc::STATUS_OUTBOX_DEPTH]).expect("outbox depth"),
            0
        );
        assert_eq!(
            <&str>::try_from(&status[ipc::STATUS_TRANSPORT_STATE]).expect("transport state"),
            ipc::TRANSPORT_IDLE
        );
        assert_eq!(
            u32::try_from(&status[ipc::STATUS_ACTIVE_SESSIONS]).expect("active sessions"),
            0
        );
    }

    #[tokio::test]
    async fn identity_and_settings_are_exposed() {
        let (service, _dir) = test_service().await;
        let identity = service.get_identity().await;
        assert!(identity.contains_key(ipc::IDENTITY_NODE_ID));
        assert!(identity.contains_key(ipc::IDENTITY_FINGERPRINT));

        let settings = service.list_settings().await.expect("settings");
        assert!(settings.contains_key("dnd.enabled"));
        assert!(
            !bool::try_from(service.get_setting("dnd.enabled").await.expect("dnd")).expect("bool")
        );
    }

    #[tokio::test]
    async fn shutdown_requests_runtime_stop() {
        let (service, _dir) = test_service().await;
        let shorthand = service.state.shutdown().clone();
        let waiter = tokio::spawn({
            let shutdown = shorthand.clone();
            async move { shutdown.wait().await }
        });

        service.shutdown().expect("shutdown request");
        waiter.await.expect("shutdown wait");
        assert!(shorthand.is_requested());
        assert!(service.shutdown().is_err());
    }

    async fn test_service() -> (DaemonService, PathBuf) {
        let dir = std::env::temp_dir().join(format!(
            "ticonnect-service-{}-{}",
            std::process::id(),
            unique_id()
        ));
        std::fs::create_dir_all(&dir).expect("create test dir");
        let settings = Settings::load_from(dir.join("config.toml")).expect("settings");
        let identity = Identity::load_or_create(&dir).expect("identity");
        let public = identity.public();
        let database = DatabaseHandle::spawn(dir.join("ticonnect.db"))
            .await
            .expect("database");
        let trust = Arc::new(TrustStore::load(database.clone()).await.expect("trust"));
        let discovery = Arc::new(LanDiscovery::new(discovery_config(&settings, &public, 0)));
        let transport = Arc::new(
            Transport::bind(TransportConfig {
                identity: identity.key_pair().clone(),
                node_id: public.node_id(),
                fingerprint: public.fingerprint(),
                bind_addr: SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 0),
                trust: trust.clone(),
            })
            .await
            .expect("transport"),
        );
        let pairing = Arc::new(PairingService::new(
            public.clone(),
            trust.clone(),
            transport.clone(),
            discovery.clone(),
        ));
        let messaging = Arc::new(MessagingService::new(
            identity.key_pair().clone(),
            public.node_id(),
            database.clone(),
            trust.clone(),
            transport.clone(),
            settings.clone(),
        ));
        let attachments = Arc::new(
            AttachmentService::new(
                database.clone(),
                dir.join("attachments"),
                transport.clone(),
                settings.clone(),
            )
            .expect("attachments"),
        );
        let calls = Arc::new(CallService::new(
            public.node_id(),
            database.clone(),
            trust.clone(),
            transport.clone(),
            settings.clone(),
            Arc::new(ticonnect_media::ringtone::NullRingtone::default()),
        ));
        calls.set_media_enabled(false);
        let remote = Arc::new(RemoteService::new(
            trust.clone(),
            transport.clone(),
            settings.clone(),
            Arc::new(crate::remote::NoopRemoteHost),
        ));
        let state = Arc::new(DaemonState::new(
            Arc::new(Shutdown::new()),
            public,
            settings,
            database,
            discovery,
            trust,
            transport,
            pairing,
            messaging,
            attachments,
            calls,
            remote,
        ));
        (DaemonService::new(state), dir)
    }

    fn unique_id() -> u64 {
        use std::sync::atomic::{AtomicU64, Ordering};
        static NEXT: AtomicU64 = AtomicU64::new(0);
        NEXT.fetch_add(1, Ordering::Relaxed)
    }
}
