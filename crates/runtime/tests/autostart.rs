use std::fs;
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Mutex, MutexGuard};
use std::time::Duration;

use ticonnect_core::ipc::client::DaemonProxy;
use ticonnect_runtime::ensure_daemon_with_options;

static LOCK: Mutex<()> = Mutex::new(());

fn lock() -> MutexGuard<'static, ()> {
    LOCK.lock().unwrap_or_else(|error| error.into_inner())
}

#[test]
fn disabled_autostart_does_not_spawn() {
    let _guard = lock();
    let calls = AtomicUsize::new(0);
    ensure_daemon_with_options(
        || {
            calls.fetch_add(1, Ordering::Relaxed);
            Ok(())
        },
        Duration::from_millis(50),
        true,
    )
    .expect("disabled autostart is best effort");
    assert_eq!(calls.load(Ordering::Relaxed), 0);
}

#[test]
fn absent_daemon_spawns_once_and_reports_log_path() {
    let _guard = lock();
    let calls = AtomicUsize::new(0);
    let error = ensure_daemon_with_options(
        || {
            calls.fetch_add(1, Ordering::Relaxed);
            Ok(())
        },
        Duration::from_millis(50),
        false,
    )
    .expect_err("readiness must time out without a daemon");
    assert_eq!(calls.load(Ordering::Relaxed), 1);
    assert!(error.to_string().contains("daemon.log"), "{error}");
}

#[test]
fn existing_daemon_is_not_spawned() {
    let _guard = lock();
    if std::env::var_os("DBUS_SESSION_BUS_ADDRESS").is_none() {
        eprintln!("skipping: DBUS_SESSION_BUS_ADDRESS is not set");
        return;
    }
    let dir = std::env::temp_dir().join(format!("ticonnect-autostart-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("create temp dir");

    let mut daemon = Command::new(env!("CARGO_BIN_EXE_ticonnectd"))
        .env("TICONNECT_CONFIG_DIR", dir.join("config"))
        .env("TICONNECT_DATA_DIR", dir.join("data"))
        .env_remove("DISPLAY")
        .env_remove("WAYLAND_DISPLAY")
        .spawn()
        .expect("start test daemon");

    let runtime = tokio::runtime::Runtime::new().expect("test runtime");
    let ready = runtime.block_on(async {
        let connection = zbus::Connection::session().await.expect("session bus");
        let proxy = DaemonProxy::new(&connection).await.expect("daemon proxy");
        for _ in 0..150 {
            if proxy.ping().await.is_ok() {
                return true;
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        false
    });
    assert!(ready, "test daemon did not start");

    let calls = AtomicUsize::new(0);
    ensure_daemon_with_options(
        || {
            calls.fetch_add(1, Ordering::Relaxed);
            Ok(())
        },
        Duration::from_millis(50),
        false,
    )
    .expect("existing daemon is ready");
    assert_eq!(calls.load(Ordering::Relaxed), 0);

    let _ = daemon.kill();
    let _ = daemon.wait();
    let _ = fs::remove_dir_all(&dir);
}
