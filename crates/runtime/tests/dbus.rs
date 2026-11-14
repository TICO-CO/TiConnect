use std::collections::HashMap;
use std::fs;
use std::time::Duration;

use futures_util::StreamExt;
use ticonnect_core::ipc;
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
    fn list_peers(&self) -> zbus::Result<Vec<HashMap<String, OwnedVariant>>>;
    fn list_candidates(&self) -> zbus::Result<Vec<HashMap<String, OwnedVariant>>>;
    fn get_discovery_status(&self) -> zbus::Result<HashMap<String, OwnedVariant>>;
    fn start_pairing(&self) -> zbus::Result<HashMap<String, OwnedVariant>>;
    fn pair_with_pin(&self, node_id: &str, pin: &str) -> zbus::Result<bool>;
    fn pair_with_qr(&self, payload: &str) -> zbus::Result<bool>;
    fn cancel_pairing(&self) -> zbus::Result<()>;
    fn forget_peer(&self, peer_id: &str) -> zbus::Result<bool>;
    fn list_trusted_peers(&self) -> zbus::Result<Vec<HashMap<String, OwnedVariant>>>;
    fn send_message(&self, peer_id: &str, body: &str) -> zbus::Result<String>;
    fn list_messages(
        &self,
        conversation_id: &str,
        limit: u32,
        offset: u32,
    ) -> zbus::Result<Vec<HashMap<String, OwnedVariant>>>;
    fn get_message_state(&self, message_id: &str) -> zbus::Result<String>;
    fn set_peer_policy(&self, peer_id: &str, policy: &str) -> zbus::Result<()>;
    fn set_dnd(&self, enabled: bool) -> zbus::Result<()>;
    fn send_attachment(&self, peer_id: &str, path: &str, mime: &str) -> zbus::Result<String>;
    fn get_transfer_state(&self, hash: &str) -> zbus::Result<HashMap<String, OwnedVariant>>;
    fn cancel_transfer(&self, hash: &str) -> zbus::Result<()>;
    fn save_attachment(&self, hash: &str, destination: &str) -> zbus::Result<bool>;
    fn get_attachment_path(&self, hash: &str) -> zbus::Result<String>;
    fn start_call(&self, peer_id: &str) -> zbus::Result<String>;
    fn accept_call(&self, call_id: &str) -> zbus::Result<()>;
    fn decline_call(&self, call_id: &str) -> zbus::Result<()>;
    fn hangup_call(&self, call_id: &str) -> zbus::Result<()>;
    fn get_call_state(&self) -> zbus::Result<HashMap<String, OwnedVariant>>;
    fn request_remote_session(&self, peer_id: &str) -> zbus::Result<HashMap<String, OwnedVariant>>;
    fn accept_remote_session(&self, session_id: &str, pin: &str) -> zbus::Result<()>;
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
    if std::env::var_os("DBUS_SESSION_BUS_ADDRESS").is_none() {
        eprintln!("skipping D-Bus test: DBUS_SESSION_BUS_ADDRESS is not set");
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

    let connection = zbus::Connection::session().await.expect("session bus");
    let proxy = DaemonProxy::new(&connection).await.expect("daemon proxy");
    wait_for_daemon(&proxy).await;

    assert_eq!(proxy.ping().await.expect("ping"), "pong");
    assert_eq!(proxy.get_version().await.expect("version"), ipc::VERSION);

    let status = proxy.get_status().await.expect("status");
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

    let settings = proxy.list_settings().await.expect("settings");
    assert!(settings.contains_key("dnd.enabled"));
    assert!(settings.contains_key("appearance.color_scheme"));
    assert!(settings.contains_key("attachments.max_size_mib"));
    assert!(settings.contains_key("calls.timeout_secs"));
    assert!(settings.contains_key("remote.pin_attempts"));
    assert!(settings.contains_key("video.bitrate_kbps"));
    assert!(!bool::try_from(proxy.get_setting("dnd.enabled").await.expect("dnd")).expect("bool"));
    assert!(proxy.get_setting("missing.key").await.is_err());
    assert!(proxy.list_peers().await.expect("peers").is_empty());
    assert!(
        proxy
            .list_candidates()
            .await
            .expect("candidates")
            .is_empty()
    );
    assert!(
        proxy
            .list_trusted_peers()
            .await
            .expect("trusted peers")
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
            .set_peer_policy("00000000-0000-7000-8000-000000000000", "mute")
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

    let pairing = proxy.start_pairing().await.expect("start pairing");
    assert_eq!(
        <&str>::try_from(&pairing[ipc::PAIRING_PIN])
            .expect("pairing pin")
            .len(),
        6
    );
    assert!(
        <&str>::try_from(&pairing[ipc::PAIRING_QR])
            .expect("pairing qr")
            .starts_with("ticonnect://pair/v1?")
    );
    assert!(u64::try_from(&pairing[ipc::PAIRING_EXPIRES_AT]).expect("expiry") > 0);
    proxy.cancel_pairing().await.expect("cancel pairing");

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
