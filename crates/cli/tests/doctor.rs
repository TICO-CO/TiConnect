use std::fs;
use std::path::PathBuf;
use std::process::Command;

use serde_json::Value;

#[test]
fn doctor_passes_with_overridden_environment() {
    let dir = temp_dir();
    fs::create_dir_all(&dir).expect("create config dir");
    let output = Command::new(env!("CARGO_BIN_EXE_ticonnect-cli"))
        .arg("doctor")
        .env_clear()
        .env(
            "LD_LIBRARY_PATH",
            std::env::var_os("LD_LIBRARY_PATH").unwrap_or_default(),
        )
        .env("TICONNECT_CONFIG_DIR", &dir)
        .env("TICONNECT_DOCTOR_BUS", "present")
        .env("TICONNECT_DOCTOR_DISPLAY", "wayland")
        .env("TICONNECT_DOCTOR_AUDIO", "available")
        .env("TICONNECT_DOCTOR_YTDLP", "available")
        .env("TICONNECT_DOCTOR_GSTREAMER", "available")
        .output()
        .expect("run doctor");
    let _ = fs::remove_dir_all(&dir);

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("[pass] session-bus"));
    assert!(stdout.contains("[pass] display"));
}

#[test]
fn doctor_json_reports_missing_bus_and_display() {
    let dir = temp_dir();
    fs::create_dir_all(&dir).expect("create config dir");
    let output = Command::new(env!("CARGO_BIN_EXE_ticonnect-cli"))
        .args(["doctor", "--json"])
        .env_clear()
        .env(
            "LD_LIBRARY_PATH",
            std::env::var_os("LD_LIBRARY_PATH").unwrap_or_default(),
        )
        .env("TICONNECT_CONFIG_DIR", &dir)
        .env("TICONNECT_DOCTOR_BUS", "missing")
        .env("TICONNECT_DOCTOR_DISPLAY", "missing")
        .env("TICONNECT_DOCTOR_AUDIO", "missing")
        .env("TICONNECT_DOCTOR_YTDLP", "missing")
        .env("TICONNECT_DOCTOR_GSTREAMER", "missing")
        .output()
        .expect("run doctor");
    let _ = fs::remove_dir_all(&dir);

    assert!(!output.status.success());
    let report: Value = serde_json::from_slice(&output.stdout).expect("doctor prints JSON");
    assert_eq!(report["failed"], true);
    assert_eq!(status(&report, "session-bus"), "fail");
    assert_eq!(status(&report, "display"), "fail");
    assert_eq!(status(&report, "youtube"), "warn");
    assert_eq!(status(&report, "media-plugins"), "warn");
}

fn status<'a>(report: &'a Value, name: &str) -> &'a str {
    report["checks"]
        .as_array()
        .expect("checks is an array")
        .iter()
        .find(|check| check["name"] == name)
        .expect("check is present")["status"]
        .as_str()
        .expect("status is a string")
}

fn temp_dir() -> PathBuf {
    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT: AtomicU64 = AtomicU64::new(0);
    let id = NEXT.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!("ticonnect-doctor-{}-{id}", std::process::id()))
}
