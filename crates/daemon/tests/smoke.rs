use std::fs;
use std::process::Command;

#[test]
fn daemon_binary_exits_successfully() {
    let dir = std::env::temp_dir().join(format!("ticonnect-smoke-daemon-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("create smoke dir");

    let output = Command::new(env!("CARGO_BIN_EXE_ticonnectd"))
        .env_clear()
        .env("TICONNECT_CONFIG_DIR", &dir)
        .output()
        .expect("run daemon binary");

    let _ = fs::remove_dir_all(&dir);

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}
