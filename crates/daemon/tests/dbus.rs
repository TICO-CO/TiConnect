use std::collections::HashMap;
use std::fs;
use std::time::Duration;

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
    fn shutdown(&self) -> zbus::Result<()>;
}

#[tokio::test]
async fn daemon_serves_dbus_contract_enforces_single_instance_and_stops_on_sigterm() {
    if std::env::var_os("DBUS_SESSION_BUS_ADDRESS").is_none() {
        eprintln!("skipping D-Bus test: DBUS_SESSION_BUS_ADDRESS is not set");
        return;
    }

    let dir = std::env::temp_dir().join(format!("ticonnect-dbus-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("create config dir");

    let mut daemon = Command::new(env!("CARGO_BIN_EXE_ticonnectd"))
        .env("TICONNECT_CONFIG_DIR", &dir)
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
        ipc::TRANSPORT_OFFLINE
    );

    let second = Command::new(env!("CARGO_BIN_EXE_ticonnectd"))
        .env("TICONNECT_CONFIG_DIR", dir.join("second"))
        .output()
        .await
        .expect("run second daemon");
    assert!(
        !second.status.success(),
        "second instance must not acquire the bus name"
    );

    let pid = daemon.id().expect("daemon pid");
    let kill = Command::new("kill")
        .arg("-TERM")
        .arg(pid.to_string())
        .status()
        .await
        .expect("send SIGTERM");
    assert!(kill.success());

    let status = tokio::time::timeout(Duration::from_secs(5), daemon.wait())
        .await
        .expect("daemon exits after SIGTERM")
        .expect("daemon wait");
    assert!(status.success(), "daemon must exit cleanly on SIGTERM");

    let _ = fs::remove_dir_all(&dir);
}

async fn wait_for_daemon(proxy: &DaemonProxy<'_>) {
    for _ in 0..50 {
        if proxy.ping().await.is_ok() {
            return;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    panic!("daemon did not become reachable");
}
