use std::fs;
use std::process::Command;

fn temp_config() -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("ticonnect-unified-cli-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("create config dir");
    dir
}

#[test]
fn settings_do_not_require_a_daemon() {
    let dir = temp_config();
    let output = Command::new(env!("CARGO_BIN_EXE_ticonnect"))
        .args(["settings", "list"])
        .env("TICONNECT_CONFIG_DIR", &dir)
        .env(
            "DBUS_SESSION_BUS_ADDRESS",
            "unix:path=/nonexistent/ticonnect-bus",
        )
        .env("TICONNECT_NO_AUTOSTART", "1")
        .output()
        .expect("run ticonnect settings");
    let _ = fs::remove_dir_all(&dir);

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(String::from_utf8_lossy(&output.stdout).contains("outbox.ttl_days"));
}

#[test]
fn status_reports_a_clear_connection_error_without_a_bus() {
    let dir = temp_config();
    let output = Command::new(env!("CARGO_BIN_EXE_ticonnect"))
        .arg("status")
        .env("TICONNECT_CONFIG_DIR", &dir)
        .env(
            "DBUS_SESSION_BUS_ADDRESS",
            "unix:path=/nonexistent/ticonnect-bus",
        )
        .env("TICONNECT_NO_AUTOSTART", "1")
        .output()
        .expect("run ticonnect status");
    let _ = fs::remove_dir_all(&dir);

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("session bus"), "stderr: {stderr}");
}
