use std::collections::HashMap;
use std::fs;
use std::time::Duration;

use futures_util::StreamExt;
use ticonnect_core::attachment::{AttachmentHash, AttachmentRef};
use ticonnect_core::id::NodeId;
use ticonnect_core::identity::IdentityKeyPair;
use ticonnect_core::ipc;
use ticonnect_core::message::{MessageKind, MessageRecord, conversation_id};
use ticonnect_daemon::storage::{
    AttachmentRecord, DatabaseHandle, MessageState, NewUser, UserPolicy,
};
use tokio::process::Command;
use zbus::OwnedVariant;
use zbus::proxy;

#[proxy(
    default_service = "org.ticonnect.Daemon",
    default_path = "/org/ticonnect/Daemon",
    interface = "org.ticonnect.Daemon"
)]
trait Daemon {
    fn ping(&self) -> zbus::Result<String>;
    fn get_version(&self) -> zbus::Result<String>;
    fn get_status(&self) -> zbus::Result<HashMap<String, OwnedVariant>>;
    fn get_identity(&self) -> zbus::Result<HashMap<String, OwnedVariant>>;
    fn get_setting(&self, key: &str) -> zbus::Result<OwnedVariant>;
    fn list_settings(&self) -> zbus::Result<HashMap<String, OwnedVariant>>;
    fn list_users(&self) -> zbus::Result<Vec<HashMap<String, OwnedVariant>>>;
    fn list_candidates(&self) -> zbus::Result<Vec<HashMap<String, OwnedVariant>>>;
    fn get_discovery_status(&self) -> zbus::Result<HashMap<String, OwnedVariant>>;
    fn forget_user(&self, user_id: &str) -> zbus::Result<bool>;
    fn send_message(&self, user_id: &str, body: &str) -> zbus::Result<String>;
    fn list_messages(
        &self,
        conversation_id: &str,
        limit: u32,
        offset: u32,
    ) -> zbus::Result<Vec<HashMap<String, OwnedVariant>>>;
    fn get_message_state(&self, message_id: &str) -> zbus::Result<String>;
    fn clear_history(&self, user_id: &str) -> zbus::Result<u32>;
    fn get_storage_usage(&self) -> zbus::Result<HashMap<String, OwnedVariant>>;
    fn set_user_policy(&self, user_id: &str, policy: &str) -> zbus::Result<()>;
    fn set_dnd(&self, enabled: bool) -> zbus::Result<()>;
    fn send_attachment(&self, user_id: &str, path: &str, mime: &str) -> zbus::Result<String>;
    fn get_transfer_state(&self, hash: &str) -> zbus::Result<HashMap<String, OwnedVariant>>;
    fn cancel_transfer(&self, hash: &str) -> zbus::Result<()>;
    fn save_attachment(&self, hash: &str, destination: &str) -> zbus::Result<bool>;
    fn get_attachment_path(&self, hash: &str) -> zbus::Result<String>;
    fn start_call(&self, user_id: &str) -> zbus::Result<String>;
    fn accept_call(&self, call_id: &str) -> zbus::Result<()>;
    fn decline_call(&self, call_id: &str) -> zbus::Result<()>;
    fn hangup_call(&self, call_id: &str) -> zbus::Result<()>;
    fn get_call_state(&self) -> zbus::Result<HashMap<String, OwnedVariant>>;
    fn request_remote_session(&self, user_id: &str) -> zbus::Result<HashMap<String, OwnedVariant>>;
    fn accept_remote_session(&self, session_id: &str) -> zbus::Result<()>;
    fn deny_remote_session(&self, session_id: &str) -> zbus::Result<()>;
    fn end_remote_session(&self, session_id: &str) -> zbus::Result<()>;
    fn get_remote_session(&self) -> zbus::Result<HashMap<String, OwnedVariant>>;
    fn shutdown(&self) -> zbus::Result<()>;

    #[zbus(signal)]
    fn daemon_stopping(&self) -> zbus::Result<()>;

    #[zbus(signal)]
    fn settings_changed(&self, key: &str) -> zbus::Result<()>;

    #[zbus(signal)]
    fn transfer_progress(&self, hash: &str, bytes: u64, total: u64) -> zbus::Result<()>;

    #[zbus(signal)]
    fn transfer_done(&self, hash: &str) -> zbus::Result<()>;

    #[zbus(signal)]
    fn transfer_failed(&self, hash: &str, reason: &str) -> zbus::Result<()>;

    #[zbus(signal)]
    fn incoming_call(&self, call: HashMap<String, OwnedVariant>) -> zbus::Result<()>;

    #[zbus(signal)]
    fn call_state_changed(&self, call_id: &str, state: &str) -> zbus::Result<()>;

    #[zbus(signal)]
    fn call_ended(&self, call_id: &str, reason: &str) -> zbus::Result<()>;

    #[zbus(signal)]
    fn remote_session_requested(&self, session: HashMap<String, OwnedVariant>) -> zbus::Result<()>;

    #[zbus(signal)]
    fn remote_session_started(&self, session_id: &str) -> zbus::Result<()>;

    #[zbus(signal)]
    fn remote_session_ended(&self, session_id: &str, reason: &str) -> zbus::Result<()>;

    #[zbus(signal)]
    fn remote_indicator_changed(&self, session_id: &str, visible: bool) -> zbus::Result<()>;
}

#[tokio::test]
async fn daemon_serves_dbus_contract_enforces_single_instance_and_stops_on_sigterm() {
    if ipc::ipc_bus_address().is_none() && std::env::var_os("DBUS_SESSION_BUS_ADDRESS").is_none() {
        eprintln!("skipping D-Bus test: no D-Bus address is configured");
        return;
    }

    let dir = std::env::temp_dir().join(format!("ticonnect-dbus-{}", std::process::id()));
    let config_dir = dir.join("config");
    let data_dir = dir.join("data");
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&config_dir).expect("create config dir");

    let mut daemon = Command::new(env!("CARGO_BIN_EXE_ticonnectd"))
        .env("TICONNECT_CONFIG_DIR", &config_dir)
        .env("TICONNECT_DATA_DIR", &data_dir)
        .env_remove("DISPLAY")
        .env_remove("WAYLAND_DISPLAY")
        .spawn()
        .expect("start daemon");

    let connection = ipc::connect().await.expect("IPC bus");
    let proxy = DaemonProxy::new(&connection).await.expect("daemon proxy");
    wait_for_daemon(&proxy).await;

    assert_eq!(proxy.ping().await.expect("ping"), "pong");
    assert_eq!(proxy.get_version().await.expect("version"), ipc::VERSION);

    let status = proxy.get_status().await.expect("status");
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

    let identity = proxy.get_identity().await.expect("identity");
    assert!(
        <&str>::try_from(&identity[ipc::IDENTITY_NODE_ID])
            .expect("node id")
            .contains('-')
    );
    assert_eq!(
        <&str>::try_from(&identity[ipc::IDENTITY_FINGERPRINT])
            .expect("fingerprint")
            .len(),
        64
    );
    assert_eq!(
        <&str>::try_from(&identity[ipc::IDENTITY_DISPLAY_NAME]).expect("display name"),
        ""
    );
    assert!(u64::try_from(&identity[ipc::IDENTITY_CREATED_AT]).expect("identity created_at") > 0);

    let usage = proxy.get_storage_usage().await.expect("storage usage");
    assert_eq!(
        u64::try_from(&usage[ipc::STORAGE_MESSAGE_COUNT]).expect("message count"),
        0
    );
    assert_eq!(
        u64::try_from(&usage[ipc::STORAGE_ATTACHMENT_COUNT]).expect("attachment count"),
        0
    );
    assert_eq!(
        u64::try_from(&usage[ipc::STORAGE_ATTACHMENT_BYTES]).expect("attachment bytes"),
        0
    );
    assert!(u64::try_from(&usage[ipc::STORAGE_DATABASE_BYTES]).expect("database bytes") > 0);

    let settings = proxy.list_settings().await.expect("settings");
    assert!(settings.contains_key("dnd.enabled"));
    assert!(settings.contains_key("appearance.color_scheme"));
    assert!(settings.contains_key("attachments.max_size_mib"));
    assert!(settings.contains_key("calls.timeout_secs"));
    assert!(settings.contains_key("remote.session_timeout_secs"));
    assert!(settings.contains_key("video.bitrate_kbps"));
    assert!(!bool::try_from(proxy.get_setting("dnd.enabled").await.expect("dnd")).expect("bool"));
    assert!(proxy.get_setting("missing.key").await.is_err());
    assert!(proxy.list_users().await.expect("users").is_empty());
    assert!(
        proxy
            .list_candidates()
            .await
            .expect("candidates")
            .is_empty()
    );
    assert!(
        proxy
            .list_messages("missing-conversation", 10, 0)
            .await
            .expect("messages")
            .is_empty()
    );
    assert!(
        proxy
            .send_message("00000000-0000-7000-8000-000000000000", "hello")
            .await
            .is_err()
    );
    assert!(
        proxy
            .get_message_state("00000000-0000-7000-8000-000000000000")
            .await
            .is_err()
    );
    assert!(
        proxy
            .set_user_policy("00000000-0000-7000-8000-000000000000", "mute")
            .await
            .is_err()
    );
    proxy.set_dnd(true).await.expect("set dnd");
    assert!(bool::try_from(proxy.get_setting("dnd.enabled").await.expect("dnd")).expect("dnd"));
    assert!(proxy.get_call_state().await.expect("call state").is_empty());
    assert!(
        proxy
            .get_remote_session()
            .await
            .expect("remote state")
            .is_empty()
    );
    assert!(
        proxy
            .start_call("00000000-0000-7000-8000-000000000000")
            .await
            .is_err()
    );
    assert!(
        proxy
            .request_remote_session("00000000-0000-7000-8000-000000000000")
            .await
            .is_err()
    );
    assert!(proxy.get_transfer_state(&"0".repeat(64)).await.is_err());
    assert!(proxy.cancel_transfer(&"0".repeat(64)).await.is_err());
    assert!(
        proxy
            .send_attachment(
                "00000000-0000-7000-8000-000000000000",
                "/missing/path.bin",
                ""
            )
            .await
            .is_err()
    );
    assert!(
        proxy
            .save_attachment(&"0".repeat(64), "/tmp/does-not-exist")
            .await
            .is_err()
    );
    assert!(proxy.get_attachment_path(&"0".repeat(64)).await.is_err());
    let discovery = proxy
        .get_discovery_status()
        .await
        .expect("discovery status");
    assert!(bool::try_from(&discovery[ipc::DISCOVERY_ENABLED]).expect("discovery enabled"));
    assert!(bool::try_from(&discovery[ipc::DISCOVERY_MDNS_ENABLED]).expect("mdns enabled"));
    assert!(
        bool::try_from(&discovery[ipc::DISCOVERY_BROADCAST_ENABLED]).expect("broadcast enabled")
    );
    assert_eq!(
        u32::try_from(&discovery[ipc::DISCOVERY_CANDIDATE_COUNT]).expect("candidate count"),
        0
    );

    let local_node = <&str>::try_from(&identity[ipc::IDENTITY_NODE_ID])
        .expect("node id")
        .parse::<NodeId>()
        .expect("parse node id");
    let user = NodeId::generate();
    let database = DatabaseHandle::spawn(data_dir.join("ticonnect.db"))
        .await
        .expect("test database");
    let identity_key = IdentityKeyPair::from_secret_bytes([91_u8; 32]);
    database
        .insert_user(NewUser {
            id: user.to_string(),
            fingerprint: "feature-test".to_owned(),
            public_key: Some(identity_key.verifying_key().to_bytes()),
            display_name: None,
            policy: UserPolicy::Accept,
            trusted_at: 1,
            last_seen: None,
        })
        .await
        .expect("insert user");
    let conversation = conversation_id(local_node, user);
    database
        .ensure_conversation(&conversation, &user.to_string(), 1)
        .await
        .expect("ensure conversation");

    let text = MessageRecord::text(
        conversation.clone(),
        local_node.to_string(),
        1,
        1_700_000_000_000,
        "local history",
    );
    database
        .insert_message(
            text.clone(),
            1_700_000_000_000,
            vec![0_u8; 64],
            Some(MessageState::Queued),
        )
        .await
        .expect("insert text");

    let bytes = b"voice feature attachment";
    let hash = AttachmentHash::of(bytes);
    let path = data_dir
        .join("attachments")
        .join(&hash.to_hex()[..2])
        .join(hash.to_hex());
    fs::create_dir_all(path.parent().expect("attachment parent")).expect("attachment dir");
    fs::write(&path, bytes).expect("attachment file");
    let attachment = AttachmentRef::new(hash, "voice.webm", bytes.len() as u64, "audio/webm")
        .expect("attachment");
    let attachment_message = MessageRecord::new(
        conversation.clone(),
        local_node.to_string(),
        2,
        1_700_000_000_001,
        MessageKind::Audio,
        None,
    )
    .with_attachment(&attachment)
    .expect("attachment message");
    database
        .insert_message(
            attachment_message,
            1_700_000_000_001,
            vec![0_u8; 64],
            Some(MessageState::Queued),
        )
        .await
        .expect("insert attachment message");
    database
        .upsert_attachment(AttachmentRecord {
            hash: hash.to_hex(),
            size: bytes.len() as u64,
            path: path.to_string_lossy().into_owned(),
            mime: "audio/webm".to_owned(),
            name: "voice.webm".to_owned(),
            created_at: 1,
        })
        .await
        .expect("insert attachment row");

    let usage = proxy.get_storage_usage().await.expect("feature usage");
    assert_eq!(
        u64::try_from(&usage[ipc::STORAGE_MESSAGE_COUNT]).expect("message count"),
        2
    );
    assert_eq!(
        u64::try_from(&usage[ipc::STORAGE_ATTACHMENT_COUNT]).expect("attachment count"),
        1
    );
    assert_eq!(
        u64::try_from(&usage[ipc::STORAGE_ATTACHMENT_BYTES]).expect("attachment bytes"),
        bytes.len() as u64
    );

    assert_eq!(
        proxy
            .clear_history(&user.to_string())
            .await
            .expect("clear history"),
        2
    );
    assert!(!path.exists(), "attachment file was not removed");
    assert_eq!(
        u64::try_from(
            &proxy.get_storage_usage().await.expect("cleared usage")[ipc::STORAGE_MESSAGE_COUNT]
        )
        .expect("message count"),
        0
    );
    assert!(
        proxy
            .list_messages(&conversation, 10, 0)
            .await
            .expect("cleared messages")
            .is_empty()
    );
    drop(database);

    assert!(config_dir.join("identity.key").is_file());
    assert!(config_dir.join("identity.toml").is_file());
    assert!(data_dir.join("ticonnect.db").is_file());

    let mut settings_signals = proxy
        .receive_settings_changed()
        .await
        .expect("settings signal stream");
    tokio::time::sleep(Duration::from_millis(150)).await;
    fs::write(
        config_dir.join("config.toml"),
        "[dnd]\nenabled = true\n\n[discovery]\nenabled = false\n",
    )
    .expect("write settings");
    tokio::time::timeout(Duration::from_secs(5), settings_signals.next())
        .await
        .expect("settings signal timeout")
        .expect("settings signal");
    assert!(
        bool::try_from(proxy.get_setting("dnd.enabled").await.expect("dnd")).expect("dnd bool")
    );
    let discovery = tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let status = proxy
                .get_discovery_status()
                .await
                .expect("discovery status");
            if !bool::try_from(&status[ipc::DISCOVERY_ENABLED]).expect("discovery enabled") {
                return status;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    })
    .await
    .expect("discovery reload timeout");
    assert!(!bool::try_from(&discovery[ipc::DISCOVERY_MDNS_ACTIVE]).expect("mdns active"));
    assert!(
        !bool::try_from(&discovery[ipc::DISCOVERY_BROADCAST_ACTIVE]).expect("broadcast active")
    );

    let second = Command::new(env!("CARGO_BIN_EXE_ticonnectd"))
        .env("TICONNECT_CONFIG_DIR", dir.join("second-config"))
        .env("TICONNECT_DATA_DIR", dir.join("second-data"))
        .env_remove("DISPLAY")
        .env_remove("WAYLAND_DISPLAY")
        .output()
        .await
        .expect("run second daemon");
    assert!(
        !second.status.success(),
        "second instance must not acquire the bus name"
    );

    let mut stopping = proxy
        .receive_daemon_stopping()
        .await
        .expect("stopping signal stream");
    let pid = daemon.id().expect("daemon pid");
    let kill = Command::new("kill")
        .arg("-TERM")
        .arg(pid.to_string())
        .status()
        .await
        .expect("send SIGTERM");
    assert!(kill.success());

    tokio::time::timeout(Duration::from_secs(5), stopping.next())
        .await
        .expect("DaemonStopping timeout")
        .expect("DaemonStopping signal");

    let status = tokio::time::timeout(Duration::from_secs(5), daemon.wait())
        .await
        .expect("daemon exits after SIGTERM")
        .expect("daemon wait");
    assert!(status.success(), "daemon must exit cleanly on SIGTERM");

    let _ = fs::remove_dir_all(&dir);
}

async fn wait_for_daemon(proxy: &DaemonProxy<'_>) {
    for _ in 0..150 {
        if proxy.ping().await.is_ok() {
            return;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    panic!("daemon did not become reachable");
}
