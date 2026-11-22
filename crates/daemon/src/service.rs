use std::collections::HashMap;
use std::sync::RwLock;
use std::time::{Duration, Instant};

use ticonnect_core::ipc;
use ticonnect_net::discovery::candidate::{Candidate, UserPlatform};
use ticonnect_net::discovery::{Discovery, DiscoveryConfig, LanDiscovery};
use ticonnect_settings::{SettingValue, Settings};
use zbus::interface;
use zbus::object_server::SignalEmitter;
use zbus::{DBusError, OwnedVariant, Str};

use std::sync::Arc;

use crate::attachments::{AttachmentService, AttachmentsError, TransferEvent, TransferSnapshot};
use crate::auto_trust::TrustService;
use crate::calls::{CallEvent, CallService};
use crate::identity::PublicIdentity;
use crate::messaging::{MessagingEvent, MessagingService};
use crate::profile::ProfileService;
use crate::remote::{RemoteEventMessage, RemoteService, RemoteSnapshot};
use crate::runtime::Shutdown;
use crate::storage::{DatabaseHandle, StorageError, StoredMessage, User, UserPolicy, UserProfile};
use crate::trust::TrustStore;
use crate::watch_party::{WatchPartyEvent, WatchPartyService, WatchPartySnapshot};
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
    auto_trust: Arc<TrustService>,
    profile: Arc<ProfileService>,
    messaging: Arc<MessagingService>,
    attachments: Arc<AttachmentService>,
    calls: Arc<CallService>,
    remote: Arc<RemoteService>,
    watch_party: Arc<WatchPartyService>,
}

#[derive(Debug, thiserror::Error)]
pub enum DaemonInitError {
    #[error(transparent)]
    Trust(#[from] crate::trust::TrustError),

    #[error(transparent)]
    Transport(#[from] ticonnect_net::transport::TransportError),

    #[error(transparent)]
    Attachments(#[from] AttachmentsError),

    #[error(transparent)]
    Profile(#[from] ticonnect_core::profile::ProfileError),
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
        let data_dir = settings.data_dir().map_err(|error| {
            DaemonInitError::Attachments(AttachmentsError::Invalid(error.to_string()))
        })?;
        let profile = Arc::new(ProfileService::load(
            &data_dir,
            identity.key_pair().clone(),
            public.node_id(),
            settings.identity_display_name().to_owned(),
            database.clone(),
            transport.clone(),
        )?);
        let discovery = Arc::new(LanDiscovery::new(discovery_config(
            &settings,
            &public,
            port,
            profile.avatar_hash(),
        )));
        let auto_trust = Arc::new(TrustService::new(
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
                data_dir.join("attachments"),
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
        ));
        let remote = Arc::new(RemoteService::new(
            trust.clone(),
            transport.clone(),
            settings.clone(),
            Arc::new(crate::remote::DesktopRemoteHost::default()),
        ));
        let watch_party = Arc::new(WatchPartyService::new(public.node_id(), transport.clone()));
        Ok(Self::new(
            shutdown,
            public,
            settings,
            database,
            discovery,
            trust,
            transport,
            auto_trust,
            profile,
            messaging,
            attachments,
            calls,
            remote,
            watch_party,
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
        auto_trust: Arc<TrustService>,
        profile: Arc<ProfileService>,
        messaging: Arc<MessagingService>,
        attachments: Arc<AttachmentService>,
        calls: Arc<CallService>,
        remote: Arc<RemoteService>,
        watch_party: Arc<WatchPartyService>,
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
            auto_trust,
            profile,
            messaging,
            attachments,
            calls,
            remote,
            watch_party,
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

    pub fn auto_trust(&self) -> &Arc<TrustService> {
        &self.auto_trust
    }

    pub fn profile(&self) -> &Arc<ProfileService> {
        &self.profile
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

    pub fn watch_party(&self) -> &Arc<WatchPartyService> {
        &self.watch_party
    }

    pub fn settings(&self) -> Settings {
        self.settings
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }

    pub fn replace_settings(&self, settings: Settings) {
        self.profile
            .set_display_name(settings.identity_display_name().to_owned());
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
        let config = discovery_config(
            &settings,
            &self.identity,
            settings.transport_port(),
            self.profile.avatar_hash(),
        );
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
        let user_count = self
            .state
            .database()
            .count_users()
            .await
            .map_err(database_error)?;
        let outbox_depth = self
            .state
            .database()
            .outbox_depth()
            .await
            .map_err(database_error)?;

        let sessions = self.state.transport().session_count();
        let active_sessions = u32::try_from(sessions)
            .map_err(|_| ServiceError::Failed("active session count exceeds u32".to_owned()))?;
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
            ipc::STATUS_USER_COUNT.to_owned(),
            OwnedVariant::from(user_count),
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
            OwnedVariant::from(active_sessions),
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
            (
                ipc::IDENTITY_AVATAR_HASH.to_owned(),
                string_variant(self.state.profile().avatar_hash().unwrap_or_default()),
            ),
            (
                ipc::IDENTITY_CREATED_AT.to_owned(),
                OwnedVariant::from(u64::try_from(identity.created_at()).unwrap_or_default()),
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

    async fn get_avatar(&self, fingerprint: &str) -> Result<Vec<u8>, ServiceError> {
        if fingerprint == self.state.identity().fingerprint().to_hex() {
            let avatar = self.state.profile().avatar();
            if !avatar.is_empty() {
                return Ok(avatar);
            }
        }
        let profile = self
            .state
            .database()
            .get_user_profile(fingerprint)
            .await
            .map_err(database_error)?
            .ok_or_else(|| ServiceError::NotFound(format!("avatar {fingerprint}")))?;
        if profile.avatar.is_empty() {
            return Err(ServiceError::NotFound(format!("avatar {fingerprint}")));
        }
        Ok(profile.avatar)
    }

    async fn set_avatar(&self, avatar: Vec<u8>) -> Result<(), ServiceError> {
        self.state
            .profile()
            .set_avatar(avatar)
            .map_err(|error| ServiceError::Failed(error.to_string()))?;
        self.state.reconfigure_discovery().await;
        Ok(())
    }

    async fn clear_avatar(&self) -> Result<(), ServiceError> {
        self.state
            .profile()
            .clear_avatar()
            .map_err(|error| ServiceError::Failed(error.to_string()))?;
        self.state.reconfigure_discovery().await;
        Ok(())
    }

    async fn list_conversation_summaries(
        &self,
    ) -> Result<Vec<HashMap<String, OwnedVariant>>, ServiceError> {
        let summaries = self
            .state
            .database()
            .list_conversation_summaries(&self.state.identity().node_id().to_string())
            .await
            .map_err(database_error)?;
        summaries
            .into_iter()
            .map(conversation_summary_variant)
            .collect()
    }

    async fn set_typing(&self, user_id: &str, typing: bool) -> Result<(), ServiceError> {
        self.state
            .messaging()
            .set_typing(user_id, typing)
            .await
            .map_err(|error| ServiceError::Failed(error.to_string()))
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

    async fn list_users(&self) -> Result<Vec<HashMap<String, OwnedVariant>>, ServiceError> {
        let users = self
            .state
            .database()
            .list_users()
            .await
            .map_err(database_error)?;
        let candidates = self.state.discovery().snapshot();
        let mut values = Vec::with_capacity(users.len());
        for user in users {
            let profile = self
                .state
                .database()
                .get_user_profile(&user.fingerprint)
                .await
                .map_err(database_error)?;
            let candidate = candidates
                .iter()
                .find(|candidate| candidate.fingerprint() == user.fingerprint);
            let connected = user_is_connected(&self.state, &user.id);
            values.push(user_variant(user, connected, profile.as_ref(), candidate)?);
        }
        Ok(values)
    }

    async fn get_user(&self, user_id: &str) -> Result<HashMap<String, OwnedVariant>, ServiceError> {
        let user = self
            .state
            .database()
            .get_user(user_id)
            .await
            .map_err(database_error)?
            .ok_or_else(|| ServiceError::NotFound(format!("user {user_id}")))?;
        let connected = user_is_connected(&self.state, &user.id);
        let fingerprint = user.fingerprint.clone();
        let profile = self
            .state
            .database()
            .get_user_profile(&fingerprint)
            .await
            .map_err(database_error)?;
        let candidate = self
            .state
            .discovery()
            .snapshot()
            .into_iter()
            .find(|candidate| candidate.fingerprint() == fingerprint);
        let mut values = user_variant(user, connected, profile.as_ref(), candidate.as_ref())?;
        if let Some(candidate) = candidate {
            let addresses = candidate.addresses_display();
            values.insert(
                ipc::USER_ADDRESSES.to_owned(),
                string_variant(addresses.clone()),
            );
            values.insert(
                ipc::USER_PORT.to_owned(),
                OwnedVariant::from(u32::from(candidate.port())),
            );
            values.insert(
                ipc::USER_SOURCE.to_owned(),
                string_variant(candidate.source().as_str().to_owned()),
            );
            let platform = candidate.platform();
            values.insert(ipc::USER_OS.to_owned(), string_variant(platform.os.clone()));
            values.insert(
                ipc::USER_DESKTOP.to_owned(),
                string_variant(platform.desktop.clone()),
            );
            values.insert(
                ipc::USER_DISPLAY_SERVER.to_owned(),
                string_variant(platform.display_server.clone()),
            );
            values.insert(
                ipc::USER_APP_VERSION.to_owned(),
                string_variant(platform.app_version.clone()),
            );
            let mac = candidate
                .addresses()
                .find_map(mac_for_address)
                .unwrap_or_default();
            values.insert(ipc::USER_MAC.to_owned(), string_variant(mac));
        }
        if let Ok(node_id) = user_id.parse::<ticonnect_core::id::NodeId>()
            && let Some(session) = self.state.transport().session_by_node_id(node_id)
        {
            values.insert(
                ipc::USER_ADDRESS.to_owned(),
                string_variant(session.user().address.to_string()),
            );
        }
        Ok(values)
    }

    async fn list_candidates(&self) -> Vec<HashMap<String, OwnedVariant>> {
        self.state
            .discovery()
            .snapshot()
            .into_iter()
            .map(candidate_variant)
            .collect()
    }

    async fn get_discovery_status(&self) -> Result<HashMap<String, OwnedVariant>, ServiceError> {
        let status = self.state.discovery().status();
        let candidate_count = u32::try_from(status.candidate_count).map_err(|_| {
            ServiceError::Failed("discovery candidate count exceeds u32".to_owned())
        })?;
        Ok(HashMap::from([
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
                OwnedVariant::from(candidate_count),
            ),
            (
                ipc::DISCOVERY_DROPPED_PACKETS.to_owned(),
                OwnedVariant::from(status.dropped_packets),
            ),
        ]))
    }

    async fn forget_user(&self, user_id: &str) -> Result<bool, ServiceError> {
        self.state
            .auto_trust()
            .forget_user(user_id)
            .await
            .map_err(trust_service_error)?;
        Ok(true)
    }

    async fn list_blocked_users(&self) -> Result<Vec<HashMap<String, OwnedVariant>>, ServiceError> {
        let users = self
            .state
            .auto_trust()
            .list_blocked_users()
            .await
            .map_err(trust_service_error)?;
        users
            .into_iter()
            .map(|user| {
                let blocked_at = checked_u64("blocked_at", user.blocked_at)?;
                Ok(HashMap::from([
                    (
                        ipc::USER_FINGERPRINT.to_owned(),
                        string_variant(user.fingerprint),
                    ),
                    (
                        ipc::USER_ID.to_owned(),
                        string_variant(user.node_id.unwrap_or_default()),
                    ),
                    (
                        ipc::USER_DISPLAY_NAME.to_owned(),
                        string_variant(user.display_name.unwrap_or_default()),
                    ),
                    (ipc::BLOCKED_AT.to_owned(), OwnedVariant::from(blocked_at)),
                ]))
            })
            .collect()
    }

    async fn unblock_user(&self, fingerprint: &str) -> Result<bool, ServiceError> {
        self.state
            .auto_trust()
            .unblock_user(fingerprint)
            .await
            .map_err(trust_service_error)
    }

    async fn send_message(&self, user_id: &str, body: &str) -> Result<String, ServiceError> {
        let message_id = self
            .state
            .messaging()
            .send_message(user_id, body)
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
            let mut variant = stored_message_variant(message)?;
            let hash = match variant.get(ipc::MESSAGE_ATTACHMENT_HASH) {
                Some(value) => display_variant_string(value)?,
                None => String::new(),
            };
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

    async fn get_message(
        &self,
        message_id: &str,
    ) -> Result<HashMap<String, OwnedVariant>, ServiceError> {
        let stored = self
            .state
            .database()
            .get_message(message_id)
            .await
            .map_err(database_error)?
            .ok_or_else(|| ServiceError::NotFound(format!("message {message_id}")))?;
        let user_id = self
            .state
            .database()
            .conversation_by_id(&stored.record.conversation_id)
            .await
            .map_err(database_error)?
            .map_or_else(String::new, |conversation| conversation.user_id);
        stored_message_variant_with(user_id, stored, false)
    }

    async fn count_messages(&self, conversation_id: &str) -> Result<u32, ServiceError> {
        self.state
            .messaging()
            .count_messages(conversation_id)
            .await
            .map_err(messaging_error)
    }

    async fn clear_history(&self, user_id: &str) -> Result<u32, ServiceError> {
        let filter = (!user_id.is_empty()).then_some(user_id);
        let outcome = self
            .state
            .database()
            .clear_history(filter)
            .await
            .map_err(database_error)?;
        self.state
            .attachments()
            .remove_stored(&outcome.attachments)
            .map_err(|error| ServiceError::Failed(error.to_string()))?;
        self.state
            .database()
            .clear_call_logs(filter)
            .await
            .map_err(database_error)?;
        Ok(outcome.messages)
    }

    async fn get_storage_usage(&self) -> Result<HashMap<String, OwnedVariant>, ServiceError> {
        let usage = self
            .state
            .database()
            .storage_usage()
            .await
            .map_err(database_error)?;
        Ok(HashMap::from([
            (
                ipc::STORAGE_MESSAGE_COUNT.to_owned(),
                OwnedVariant::from(usage.message_count),
            ),
            (
                ipc::STORAGE_ATTACHMENT_COUNT.to_owned(),
                OwnedVariant::from(usage.attachment_count),
            ),
            (
                ipc::STORAGE_ATTACHMENT_BYTES.to_owned(),
                OwnedVariant::from(usage.attachment_bytes),
            ),
            (
                ipc::STORAGE_DATABASE_BYTES.to_owned(),
                OwnedVariant::from(usage.database_bytes),
            ),
        ]))
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

    async fn mark_conversation_read(&self, conversation_id: &str) -> Result<(), ServiceError> {
        self.state
            .messaging()
            .mark_conversation_read(conversation_id)
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

    async fn set_user_policy(&self, user_id: &str, policy: &str) -> Result<(), ServiceError> {
        let policy = policy
            .parse::<UserPolicy>()
            .map_err(|error| ServiceError::Failed(error.to_string()))?;
        self.state
            .messaging()
            .set_user_policy(user_id, policy)
            .await
            .map_err(messaging_error)
    }

    async fn start_call(&self, user_id: &str) -> Result<String, ServiceError> {
        let call = self
            .state
            .calls()
            .start_call(user_id)
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

    async fn set_call_muted(
        &self,
        call_id: &str,
        input_muted: bool,
        output_muted: bool,
    ) -> Result<(), ServiceError> {
        let call_id = parse_call_id(call_id)?;
        self.state
            .calls()
            .set_muted(call_id, input_muted, output_muted)
            .map_err(call_error)?;
        Ok(())
    }

    async fn list_call_logs(
        &self,
        user_id: &str,
        limit: u32,
    ) -> Result<Vec<HashMap<String, OwnedVariant>>, ServiceError> {
        let filter = (!user_id.is_empty()).then_some(user_id);
        let records = self
            .state
            .database()
            .list_call_logs(filter, limit)
            .await
            .map_err(database_error)?;
        records
            .iter()
            .map(call_log_variant)
            .collect::<Result<Vec<_>, _>>()
    }

    async fn list_audio_devices(&self) -> Result<Vec<HashMap<String, OwnedVariant>>, ServiceError> {
        let devices = ticonnect_media::audio::AudioDevices::discover()
            .map_err(|error| ServiceError::Failed(error.to_string()))?;
        Ok(devices
            .inputs
            .iter()
            .map(|device| audio_device_variant(device, "input"))
            .chain(
                devices
                    .outputs
                    .iter()
                    .map(|device| audio_device_variant(device, "output")),
            )
            .collect())
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

    async fn start_watch_party(
        &self,
        user_ids: Vec<String>,
        video_id: &str,
    ) -> Result<HashMap<String, OwnedVariant>, ServiceError> {
        let ids: Vec<&str> = user_ids.iter().map(String::as_str).collect();
        let snapshot = self
            .state
            .watch_party()
            .start(&ids, video_id)
            .await
            .map_err(watch_party_error)?;
        Ok(watch_party_variant(&snapshot))
    }

    async fn invite_watch_party(
        &self,
        party_id: &str,
        user_ids: Vec<String>,
    ) -> Result<HashMap<String, OwnedVariant>, ServiceError> {
        let ids: Vec<&str> = user_ids.iter().map(String::as_str).collect();
        let snapshot = self
            .state
            .watch_party()
            .invite(party_id, &ids)
            .await
            .map_err(watch_party_error)?;
        Ok(watch_party_variant(&snapshot))
    }

    async fn accept_watch_party(
        &self,
        party_id: &str,
    ) -> Result<HashMap<String, OwnedVariant>, ServiceError> {
        let snapshot = self
            .state
            .watch_party()
            .accept(party_id)
            .await
            .map_err(watch_party_error)?;
        Ok(watch_party_variant(&snapshot))
    }

    async fn decline_watch_party(&self, party_id: &str) -> Result<(), ServiceError> {
        self.state
            .watch_party()
            .decline(party_id)
            .await
            .map_err(watch_party_error)
    }

    async fn leave_watch_party(&self, party_id: &str) -> Result<(), ServiceError> {
        self.state
            .watch_party()
            .leave(party_id)
            .await
            .map_err(watch_party_error)
    }

    async fn publish_watch_party(
        &self,
        party_id: &str,
        position_ms: u64,
        playing: bool,
    ) -> Result<HashMap<String, OwnedVariant>, ServiceError> {
        let snapshot = self
            .state
            .watch_party()
            .publish(party_id, position_ms, playing)
            .await
            .map_err(watch_party_error)?;
        Ok(watch_party_variant(&snapshot))
    }

    async fn end_watch_party(&self, party_id: &str) -> Result<(), ServiceError> {
        self.state
            .watch_party()
            .end(party_id)
            .await
            .map_err(watch_party_error)
    }

    async fn get_watch_party(&self) -> HashMap<String, OwnedVariant> {
        self.state
            .watch_party()
            .snapshot()
            .map_or_else(HashMap::new, |snapshot| watch_party_variant(&snapshot))
    }

    async fn request_remote_session(
        &self,
        user_id: &str,
    ) -> Result<HashMap<String, OwnedVariant>, ServiceError> {
        let snapshot = self
            .state
            .remote()
            .request_session(user_id)
            .await
            .map_err(remote_error)?;
        Ok(remote_variant(&snapshot))
    }

    async fn accept_remote_session(&self, session_id: &str) -> Result<(), ServiceError> {
        self.state
            .remote()
            .accept_session(session_id)
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
        user_id: &str,
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
            .send_attachment(user_id, &attachment)
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
    pub async fn user_connected(
        emitter: SignalEmitter<'_>,
        user: HashMap<String, OwnedVariant>,
    ) -> zbus::Result<()>;

    #[zbus(signal)]
    pub async fn user_disconnected(
        emitter: SignalEmitter<'_>,
        user: HashMap<String, OwnedVariant>,
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
    pub async fn conversation_changed(
        emitter: SignalEmitter<'_>,
        user_id: &str,
    ) -> zbus::Result<()>;

    #[zbus(signal)]
    pub async fn typing_changed(
        emitter: SignalEmitter<'_>,
        user_id: &str,
        typing: bool,
    ) -> zbus::Result<()>;

    #[zbus(signal)]
    pub async fn user_profile_changed(
        emitter: SignalEmitter<'_>,
        fingerprint: &str,
    ) -> zbus::Result<()>;

    #[zbus(signal)]
    pub async fn self_profile_changed(emitter: SignalEmitter<'_>) -> zbus::Result<()>;

    #[zbus(signal)]
    pub async fn sync_completed(
        emitter: SignalEmitter<'_>,
        user_id: &str,
        count: u32,
    ) -> zbus::Result<()>;

    #[zbus(signal)]
    pub async fn policy_changed(
        emitter: SignalEmitter<'_>,
        user_id: &str,
        policy: &str,
    ) -> zbus::Result<()>;

    #[zbus(signal)]
    pub async fn watch_party_requested(
        emitter: SignalEmitter<'_>,
        party: HashMap<String, OwnedVariant>,
    ) -> zbus::Result<()>;

    #[zbus(signal)]
    pub async fn watch_party_changed(
        emitter: SignalEmitter<'_>,
        party: HashMap<String, OwnedVariant>,
    ) -> zbus::Result<()>;

    #[zbus(signal)]
    pub async fn watch_party_ended(
        emitter: SignalEmitter<'_>,
        party_id: &str,
        reason: &str,
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
    pub async fn call_mute_changed(
        emitter: SignalEmitter<'_>,
        call_id: &str,
        input_muted: bool,
        output_muted: bool,
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
            user_id,
            message,
            notify,
        } => {
            DaemonService::conversation_changed(
                SignalEmitter::new(connection, ipc::DAEMON_PATH)?,
                &user_id,
            )
            .await?;
            let message = stored_message_variant_with(user_id, message, notify)
                .map_err(|error| zbus::Error::Failure(error.to_string()))?;
            DaemonService::message_received(emitter, message).await
        }
        MessagingEvent::MessageStateChanged { message_id, state } => {
            DaemonService::message_state_changed(emitter, &message_id, state.as_str()).await
        }
        MessagingEvent::SyncCompleted { user_id, count } => {
            DaemonService::sync_completed(emitter, &user_id, count).await
        }
        MessagingEvent::PolicyChanged { user_id, policy } => {
            DaemonService::policy_changed(emitter, &user_id, policy.as_str()).await
        }
        MessagingEvent::TypingChanged { user_id, typing } => {
            DaemonService::typing_changed(emitter, &user_id, typing).await
        }
    }
}

fn stored_message_variant(
    message: StoredMessage,
) -> Result<HashMap<String, OwnedVariant>, ServiceError> {
    stored_message_variant_with(String::new(), message, false)
}

fn stored_message_variant_with(
    user_id: String,
    message: StoredMessage,
    notify: bool,
) -> Result<HashMap<String, OwnedVariant>, ServiceError> {
    let record = message.record;
    let sent_at = checked_u64("message sent_at", record.sent_at)?;
    let received_at = checked_u64("message received_at", message.received_at)?;
    Ok(HashMap::from([
        (
            ipc::MESSAGE_ID.to_owned(),
            string_variant(record.message_id.to_string()),
        ),
        (
            ipc::MESSAGE_CONVERSATION_ID.to_owned(),
            string_variant(record.conversation_id),
        ),
        (ipc::MESSAGE_USER_ID.to_owned(), string_variant(user_id)),
        (
            ipc::MESSAGE_SENDER_ID.to_owned(),
            string_variant(record.sender_id),
        ),
        (ipc::MESSAGE_SEQ.to_owned(), OwnedVariant::from(record.seq)),
        (ipc::MESSAGE_SENT_AT.to_owned(), OwnedVariant::from(sent_at)),
        (
            ipc::MESSAGE_RECEIVED_AT.to_owned(),
            OwnedVariant::from(received_at),
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
    ]))
}

fn display_variant_string(value: &OwnedVariant) -> Result<String, ServiceError> {
    <&str>::try_from(value)
        .map(str::to_owned)
        .map_err(|_| ServiceError::Failed("expected string variant".to_owned()))
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

fn checked_u64(field: &str, value: i64) -> Result<u64, ServiceError> {
    u64::try_from(value).map_err(|_| ServiceError::Failed(format!("{field} is negative: {value}")))
}

fn user_variant(
    user: User,
    connected: bool,
    profile: Option<&UserProfile>,
    candidate: Option<&Candidate>,
) -> Result<HashMap<String, OwnedVariant>, ServiceError> {
    let display_name = preferred_display_name(
        candidate.map(Candidate::display_name),
        profile.map(|profile| profile.display_name.as_str()),
        user.display_name.as_deref(),
    );
    let avatar_hash = profile
        .and_then(|profile| profile.avatar_hash.clone())
        .or_else(|| candidate.and_then(|candidate| candidate.avatar_hash().map(str::to_owned)))
        .unwrap_or_default();
    let trusted_at = checked_u64("user trusted_at", user.trusted_at)?;
    let last_seen = checked_u64("user last_seen", user.last_seen.unwrap_or_default())?;
    Ok(HashMap::from([
        (ipc::USER_ID.to_owned(), string_variant(user.id)),
        (
            ipc::USER_FINGERPRINT.to_owned(),
            string_variant(user.fingerprint),
        ),
        (
            ipc::USER_DISPLAY_NAME.to_owned(),
            string_variant(display_name),
        ),
        (
            ipc::USER_AVATAR_HASH.to_owned(),
            string_variant(avatar_hash),
        ),
        (
            ipc::USER_POLICY.to_owned(),
            string_variant(user.policy.as_str().to_owned()),
        ),
        (
            ipc::USER_TRUSTED_AT.to_owned(),
            OwnedVariant::from(trusted_at),
        ),
        (
            ipc::USER_LAST_SEEN.to_owned(),
            OwnedVariant::from(last_seen),
        ),
        (
            ipc::USER_CONNECTED.to_owned(),
            OwnedVariant::from(connected),
        ),
    ]))
}

fn preferred_display_name(
    candidate_name: Option<&str>,
    profile_name: Option<&str>,
    stored_name: Option<&str>,
) -> String {
    [candidate_name, profile_name, stored_name]
        .into_iter()
        .flatten()
        .map(str::trim)
        .find(|name| !name.is_empty())
        .unwrap_or_default()
        .to_owned()
}

fn mac_for_address(address: &str) -> Option<String> {
    let contents = std::fs::read_to_string("/proc/net/arp").ok()?;
    contents.lines().skip(1).find_map(|line| {
        let fields: Vec<&str> = line.split_whitespace().collect();
        (fields.len() >= 4 && fields[0] == address && fields[3] != "00:00:00:00:00:00")
            .then(|| fields[3].to_owned())
    })
}

fn user_is_connected(state: &DaemonState, user_id: &str) -> bool {
    user_id
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
            ipc::CANDIDATE_AVATAR_HASH.to_owned(),
            string_variant(candidate.avatar_hash().unwrap_or_default().to_owned()),
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

fn conversation_summary_variant(
    summary: crate::storage::messages::ConversationSummary,
) -> Result<HashMap<String, OwnedVariant>, ServiceError> {
    let last_sent_at = checked_u64(
        "conversation last_sent_at",
        summary.last_sent_at.unwrap_or_default(),
    )?;
    Ok(HashMap::from([
        (
            ipc::CONVERSATION_ID.to_owned(),
            string_variant(summary.conversation_id),
        ),
        (
            ipc::CONVERSATION_USER_ID.to_owned(),
            string_variant(summary.user_id),
        ),
        (
            ipc::CONVERSATION_LAST_MESSAGE_ID.to_owned(),
            string_variant(summary.last_message_id.unwrap_or_default()),
        ),
        (
            ipc::CONVERSATION_LAST_SENDER_ID.to_owned(),
            string_variant(summary.last_sender_id.unwrap_or_default()),
        ),
        (
            ipc::CONVERSATION_LAST_BODY.to_owned(),
            string_variant(summary.last_body.unwrap_or_default()),
        ),
        (
            ipc::CONVERSATION_LAST_SENT_AT.to_owned(),
            OwnedVariant::from(last_sent_at),
        ),
        (
            ipc::CONVERSATION_UNREAD_COUNT.to_owned(),
            OwnedVariant::from(u64::from(summary.unread_count)),
        ),
    ]))
}

fn local_platform() -> UserPlatform {
    UserPlatform {
        os: std::fs::read_to_string("/etc/os-release")
            .ok()
            .and_then(|contents| {
                contents.lines().find_map(|line| {
                    line.strip_prefix("PRETTY_NAME=")
                        .map(|value| value.trim_matches('"').to_owned())
                })
            })
            .unwrap_or_default(),
        desktop: std::env::var("XDG_CURRENT_DESKTOP").unwrap_or_default(),
        display_server: std::env::var("XDG_SESSION_TYPE")
            .ok()
            .filter(|value| !value.is_empty())
            .or_else(|| {
                std::env::var_os("WAYLAND_DISPLAY")
                    .map(|_| "wayland".to_owned())
                    .or_else(|| std::env::var_os("DISPLAY").map(|_| "x11".to_owned()))
            })
            .unwrap_or_default(),
        app_version: ipc::VERSION.to_owned(),
    }
}

pub fn discovery_config(
    settings: &Settings,
    identity: &PublicIdentity,
    transport_port: u16,
    avatar_hash: Option<String>,
) -> DiscoveryConfig {
    let mut config = DiscoveryConfig::new(
        settings.identity_display_name(),
        identity.node_id().to_string(),
        identity.fingerprint().to_hex(),
    );
    config.avatar_hash = avatar_hash;
    config.platform = local_platform();
    config.enabled = settings.discovery_enabled();
    config.mdns = settings.discovery_mdns();
    config.broadcast = settings.discovery_broadcast();
    config.transport_port = transport_port;
    config.beacon_interval =
        Duration::from_secs(u64::from(settings.discovery_beacon_interval_secs()));
    config
}

pub(crate) async fn emit_profile_event(
    connection: &zbus::Connection,
    fingerprint: &str,
) -> zbus::Result<()> {
    let emitter = SignalEmitter::new(connection, ipc::DAEMON_PATH)?;
    DaemonService::user_profile_changed(emitter, fingerprint).await
}

pub(crate) async fn emit_self_profile_event(connection: &zbus::Connection) -> zbus::Result<()> {
    let emitter = SignalEmitter::new(connection, ipc::DAEMON_PATH)?;
    DaemonService::self_profile_changed(emitter).await
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

pub(crate) async fn emit_watch_party_event(
    connection: &zbus::Connection,
    event: WatchPartyEvent,
) -> zbus::Result<()> {
    let emitter = SignalEmitter::new(connection, ipc::DAEMON_PATH)?;
    match event {
        WatchPartyEvent::Requested(snapshot) => {
            DaemonService::watch_party_requested(emitter, watch_party_variant(&snapshot)).await
        }
        WatchPartyEvent::Changed(snapshot) => {
            DaemonService::watch_party_changed(emitter, watch_party_variant(&snapshot)).await
        }
        WatchPartyEvent::Ended { party_id, reason } => {
            DaemonService::watch_party_ended(emitter, &party_id, &reason).await
        }
    }
}

fn watch_party_variant(snapshot: &WatchPartySnapshot) -> HashMap<String, OwnedVariant> {
    HashMap::from([
        (
            ipc::WATCH_PARTY_ID.to_owned(),
            string_variant(snapshot.party_id.clone()),
        ),
        (
            ipc::WATCH_PARTY_HOST_ID.to_owned(),
            string_variant(snapshot.host_id.clone()),
        ),
        (
            ipc::WATCH_PARTY_PARTICIPANTS.to_owned(),
            string_variant(snapshot.participants.join(",")),
        ),
        (
            ipc::WATCH_PARTY_INVITED.to_owned(),
            string_variant(snapshot.invited.join(",")),
        ),
        (
            ipc::WATCH_PARTY_VIDEO_ID.to_owned(),
            string_variant(snapshot.video_id.clone()),
        ),
        (
            ipc::WATCH_PARTY_ROLE.to_owned(),
            string_variant(snapshot.role.clone()),
        ),
        (
            ipc::WATCH_PARTY_STATE.to_owned(),
            string_variant(snapshot.state.clone()),
        ),
        (
            ipc::WATCH_PARTY_POSITION_MS.to_owned(),
            OwnedVariant::from(snapshot.position_ms),
        ),
        (
            ipc::WATCH_PARTY_PLAYING.to_owned(),
            OwnedVariant::from(snapshot.playing),
        ),
        (
            ipc::WATCH_PARTY_REVISION.to_owned(),
            OwnedVariant::from(snapshot.revision),
        ),
        (
            ipc::WATCH_PARTY_ACTOR.to_owned(),
            string_variant(snapshot.actor.clone()),
        ),
        (
            ipc::WATCH_PARTY_UPDATED_AT.to_owned(),
            OwnedVariant::from(snapshot.updated_at),
        ),
    ])
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
            ipc::REMOTE_USER_ID.to_owned(),
            string_variant(snapshot.user_id.clone()),
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
            let state_emitter = SignalEmitter::new(connection, ipc::DAEMON_PATH)?;
            DaemonService::call_state_changed(
                state_emitter,
                &call.id.to_string(),
                call.state.as_str(),
            )
            .await?;
            let mute_emitter = SignalEmitter::new(connection, ipc::DAEMON_PATH)?;
            DaemonService::call_mute_changed(
                mute_emitter,
                &call.id.to_string(),
                call.input_muted,
                call.output_muted,
            )
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
            ipc::CALL_USER_ID.to_owned(),
            string_variant(call.user_id.clone()),
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
        (
            ipc::CALL_INPUT_MUTED.to_owned(),
            OwnedVariant::from(call.input_muted),
        ),
        (
            ipc::CALL_OUTPUT_MUTED.to_owned(),
            OwnedVariant::from(call.output_muted),
        ),
    ])
}

fn call_log_variant(
    record: &crate::storage::CallRecord,
) -> Result<HashMap<String, OwnedVariant>, ServiceError> {
    let started_at = u64::try_from(record.started_at)
        .map_err(|_| ServiceError::Failed("call start time is out of range".to_owned()))?;
    let connected_at = record
        .connected_at
        .map(u64::try_from)
        .transpose()
        .map_err(|_| ServiceError::Failed("call connect time is out of range".to_owned()))?
        .unwrap_or_default();
    let ended_at = u64::try_from(record.ended_at)
        .map_err(|_| ServiceError::Failed("call end time is out of range".to_owned()))?;
    Ok(HashMap::from([
        (ipc::CALL_ID.to_owned(), string_variant(record.id.clone())),
        (
            ipc::CALL_USER_ID.to_owned(),
            string_variant(record.user_id.clone()),
        ),
        (
            ipc::CALL_DIRECTION.to_owned(),
            string_variant(record.direction.clone()),
        ),
        (
            ipc::CALL_STARTED_AT.to_owned(),
            OwnedVariant::from(started_at),
        ),
        (
            ipc::CALL_CONNECTED_AT.to_owned(),
            OwnedVariant::from(connected_at),
        ),
        (ipc::CALL_ENDED_AT.to_owned(), OwnedVariant::from(ended_at)),
        (
            ipc::CALL_DURATION_SECS.to_owned(),
            OwnedVariant::from(record.duration_secs),
        ),
        (
            ipc::CALL_REASON.to_owned(),
            string_variant(record.reason.clone()),
        ),
    ]))
}

fn audio_device_variant(
    device: &ticonnect_media::audio::AudioDevice,
    kind: &str,
) -> HashMap<String, OwnedVariant> {
    HashMap::from([
        (
            ipc::AUDIO_DEVICE_ID.to_owned(),
            string_variant(device.id.clone()),
        ),
        (
            ipc::AUDIO_DEVICE_NAME.to_owned(),
            string_variant(device.name.clone()),
        ),
        (
            ipc::AUDIO_DEVICE_KIND.to_owned(),
            string_variant(kind.to_owned()),
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
            ipc::TRANSFER_USER_ID.to_owned(),
            string_variant(snapshot.user_id.clone()),
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

fn watch_party_error(error: crate::watch_party::WatchPartyError) -> ServiceError {
    ServiceError::Failed(error.to_string())
}

fn trust_service_error(error: crate::auto_trust::TrustServiceError) -> ServiceError {
    tracing::warn!(%error, "auto_trust request failed");
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
    use crate::storage::{DatabaseHandle, NewUser, UserProfile};
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
            u32::try_from(&status[ipc::STATUS_USER_COUNT]).expect("user count"),
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

    #[test]
    fn display_name_prefers_candidate_then_profile_then_stored() {
        assert_eq!(
            preferred_display_name(Some("Lab Node B"), Some("Profile"), Some("Stored")),
            "Lab Node B"
        );
        assert_eq!(
            preferred_display_name(Some(""), Some("Profile"), Some("Stored")),
            "Profile"
        );
        assert_eq!(
            preferred_display_name(None, Some("  "), Some("Stored")),
            "Stored"
        );
        assert_eq!(preferred_display_name(None, None, None), "");
    }

    #[tokio::test]
    async fn user_payloads_prefer_cached_profile_fields() {
        let (service, _dir) = test_service().await;
        service
            .state
            .database()
            .insert_user(NewUser {
                id: "user-1".to_owned(),
                fingerprint: "fingerprint-1".to_owned(),
                public_key: None,
                display_name: Some(String::new()),
                policy: UserPolicy::Accept,
                trusted_at: 1,
                last_seen: None,
            })
            .await
            .expect("user");
        service
            .state
            .database()
            .upsert_user_profile(UserProfile {
                fingerprint: "fingerprint-1".to_owned(),
                node_id: Some("user-1".to_owned()),
                display_name: "Ada".to_owned(),
                avatar_hash: Some("avatar-hash".to_owned()),
                avatar_mime: Some("image/png".to_owned()),
                avatar: Vec::new(),
                updated_at: 1,
            })
            .await
            .expect("profile");

        let users = service.list_users().await.expect("users");
        assert_eq!(users.len(), 1);
        let listed = &users[0];
        assert_eq!(
            <&str>::try_from(&listed[ipc::USER_DISPLAY_NAME]).expect("display name"),
            "Ada"
        );
        assert_eq!(
            <&str>::try_from(&listed[ipc::USER_AVATAR_HASH]).expect("avatar hash"),
            "avatar-hash"
        );

        let user = service.get_user("user-1").await.expect("user");
        assert_eq!(
            <&str>::try_from(&user[ipc::USER_DISPLAY_NAME]).expect("display name"),
            "Ada"
        );
        assert_eq!(
            <&str>::try_from(&user[ipc::USER_AVATAR_HASH]).expect("avatar hash"),
            "avatar-hash"
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
        let discovery = Arc::new(LanDiscovery::new(discovery_config(
            &settings, &public, 0, None,
        )));
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
        let auto_trust = Arc::new(TrustService::new(
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
        ));
        calls.set_media_enabled(false);
        let remote = Arc::new(RemoteService::new(
            trust.clone(),
            transport.clone(),
            settings.clone(),
            Arc::new(crate::remote::NoopRemoteHost),
        ));
        let profile = Arc::new(
            ProfileService::load(
                &dir,
                identity.key_pair().clone(),
                public.node_id(),
                settings.identity_display_name().to_owned(),
                database.clone(),
                transport.clone(),
            )
            .expect("profile"),
        );
        let watch_party = Arc::new(WatchPartyService::new(public.node_id(), transport.clone()));
        let state = Arc::new(DaemonState::new(
            Arc::new(Shutdown::new()),
            public,
            settings,
            database,
            discovery,
            trust,
            transport,
            auto_trust,
            profile,
            messaging,
            attachments,
            calls,
            remote,
            watch_party,
        ));
        (DaemonService::new(state), dir)
    }

    fn unique_id() -> u64 {
        use std::sync::atomic::{AtomicU64, Ordering};
        static NEXT: AtomicU64 = AtomicU64::new(0);
        NEXT.fetch_add(1, Ordering::Relaxed)
    }
}
