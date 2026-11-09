use std::fs;
use std::process::Command;

#[test]
fn daemon_without_session_bus_fails_cleanly() {
    let dir = std::env::temp_dir().join(format!("ticonnect-smoke-daemon-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("create smoke dir");

    let output = Command::new(env!("CARGO_BIN_EXE_ticonnectd"))
        .env_clear()
        .env("TICONNECT_CONFIG_DIR", &dir)
        .env(
            "DBUS_SESSION_BUS_ADDRESS",
            "unix:path=/nonexistent/ticonnect-bus",
        )
        .output()
        .expect("run daemon binary");

    let _ = fs::remove_dir_all(&dir);

    assert!(
        !output.status.success(),
        "daemon must fail without a session bus"
    );
}
