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
    fn shutdown(&self) -> zbus::Result<()>;

    #[zbus(signal)]
    fn daemon_stopping(&self) -> zbus::Result<()>;

    #[zbus(signal)]
    fn settings_changed(&self, key: &str) -> zbus::Result<()>;
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
