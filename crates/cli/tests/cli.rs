use std::fs;
use std::path::PathBuf;
use std::process::Command;

#[test]
fn settings_commands_round_trip_without_daemon() {
    let dir = temp_dir();
    fs::create_dir_all(&dir).expect("create config dir");

    let set = run(&dir, &["settings", "set", "dnd.enabled", "true"]);
    assert_success(&set);
    let contents = fs::read_to_string(dir.join("config.toml")).expect("config");
    assert!(contents.contains("enabled = true"));

    let get = run(&dir, &["settings", "get", "dnd.enabled"]);
    assert_success(&get);
    assert_eq!(String::from_utf8_lossy(&get.stdout).trim(), "true");

    let list = run(&dir, &["settings", "list"]);
    assert_success(&list);
    assert!(String::from_utf8_lossy(&list.stdout).contains("outbox.ttl_days = 7"));

    let invalid = run(&dir, &["settings", "set", "outbox.ttl_days", "0"]);
    assert!(!invalid.status.success());
    assert!(
        String::from_utf8_lossy(&invalid.stderr).contains("invalid setting"),
        "stderr: {}",
        String::from_utf8_lossy(&invalid.stderr)
    );

    let _ = fs::remove_dir_all(dir);
}

#[test]
fn ping_without_daemon_exits_non_zero() {
    let dir = temp_dir();
    fs::create_dir_all(&dir).expect("create config dir");

    let output = Command::new(env!("CARGO_BIN_EXE_ticonnect-cli"))
        .args(["ping"])
        .env_clear()
        .env("TICONNECT_CONFIG_DIR", &dir)
        .env(
            "DBUS_SESSION_BUS_ADDRESS",
            "unix:path=/nonexistent/ticonnect-bus",
        )
        .output()
        .expect("run cli");

    assert!(!output.status.success(), "ping must fail without daemon");
    assert!(!output.stderr.is_empty());

    let _ = fs::remove_dir_all(dir);
}

fn run(dir: &std::path::Path, args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_ticonnect-cli"))
        .args(args)
        .env_clear()
        .env("TICONNECT_CONFIG_DIR", dir)
        .output()
        .expect("run cli")
}

fn assert_success(output: &std::process::Output) {
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

fn temp_dir() -> PathBuf {
    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT: AtomicU64 = AtomicU64::new(0);
    let id = NEXT.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!("ticonnect-cli-{}-{id}", std::process::id()))
}
