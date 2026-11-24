#![forbid(unsafe_code)]

use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result, bail};
use serde_json::{Map, Value, json};
use ticonnect_settings::Settings;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Status {
    Pass,
    Warn,
    Fail,
}

impl Status {
    fn label(self) -> &'static str {
        match self {
            Self::Pass => "pass",
            Self::Warn => "warn",
            Self::Fail => "fail",
        }
    }
}

#[derive(Debug, Clone)]
struct Check {
    name: &'static str,
    status: Status,
    detail: String,
    remedy: Option<&'static str>,
}

impl Check {
    fn new(
        name: &'static str,
        status: Status,
        detail: impl Into<String>,
        remedy: Option<&'static str>,
    ) -> Self {
        Self {
            name,
            status,
            detail: detail.into(),
            remedy,
        }
    }
}

pub fn run(json: bool) -> Result<()> {
    let checks = collect()?;
    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&report_json(&checks)).context("serialize doctor")?
        );
    } else {
        print!("{}", report_human(&checks));
    }

    if checks.iter().any(|check| check.status == Status::Fail) {
        bail!("preflight failed");
    }
    Ok(())
}

fn collect() -> Result<Vec<Check>> {
    Ok(vec![
        executable()?,
        session_bus()?,
        display()?,
        desktop()?,
        audio()?,
        youtube()?,
        media_plugins()?,
        config()?,
    ])
}

fn report_json(checks: &[Check]) -> Value {
    let mut output = Map::new();
    output.insert("version".to_owned(), json!(env!("CARGO_PKG_VERSION")));
    output.insert(
        "failed".to_owned(),
        json!(checks.iter().any(|check| check.status == Status::Fail)),
    );
    output.insert(
        "checks".to_owned(),
        Value::Array(checks.iter().map(check_json).collect()),
    );
    Value::Object(output)
}

fn check_json(check: &Check) -> Value {
    let mut output = Map::new();
    output.insert("name".to_owned(), json!(check.name));
    output.insert("status".to_owned(), json!(check.status.label()));
    output.insert("detail".to_owned(), json!(check.detail.clone()));
    output.insert(
        "remedy".to_owned(),
        check.remedy.map_or(Value::Null, |remedy| json!(remedy)),
    );
    Value::Object(output)
}

fn report_human(checks: &[Check]) -> String {
    let mut output = String::new();
    for check in checks {
        output.push_str(&format!(
            "[{}] {}: {}\n",
            check.status.label(),
            check.name,
            check.detail
        ));
        if let Some(remedy) = check.remedy {
            output.push_str(&format!("  remedy: {remedy}\n"));
        }
    }
    output
}

fn executable() -> Result<Check> {
    let path = env::current_exe().context("locate the current executable")?;
    let metadata = fs::metadata(&path).with_context(|| format!("inspect {}", path.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        if metadata.permissions().mode() & 0o111 == 0 {
            return Ok(Check::new(
                "executable",
                Status::Fail,
                format!("{} is not executable", path.display()),
                Some("Reinstall TiConnect or restore the executable bit."),
            ));
        }
    }
    Ok(Check::new(
        "executable",
        Status::Pass,
        path.display().to_string(),
        None,
    ))
}

fn session_bus() -> Result<Check> {
    if let Some(mode) = test_override("TICONNECT_DOCTOR_BUS")? {
        return match mode.as_str() {
            "present" => Ok(Check::new(
                "session-bus",
                Status::Pass,
                "test override reports a bus address".to_owned(),
                None,
            )),
            "missing" => Ok(Check::new(
                "session-bus",
                Status::Fail,
                "no D-Bus address is configured".to_owned(),
                Some("Launch TiConnect from a desktop session or set DBUS_SESSION_BUS_ADDRESS."),
            )),
            _ => bail!("invalid TICONNECT_DOCTOR_BUS override: {mode}"),
        };
    }

    if let Some(address) = env::var_os("TICONNECT_IPC_BUS_ADDRESS")
        && let Some(address) = address.to_str()
    {
        return Ok(Check::new(
            "session-bus",
            Status::Pass,
            format!("using TICONNECT_IPC_BUS_ADDRESS={address}"),
            None,
        ));
    }
    if let Some(address) = env::var_os("DBUS_SESSION_BUS_ADDRESS")
        && let Some(address) = address.to_str()
    {
        return Ok(Check::new(
            "session-bus",
            Status::Pass,
            format!("using DBUS_SESSION_BUS_ADDRESS={address}"),
            None,
        ));
    }
    Ok(Check::new(
        "session-bus",
        Status::Fail,
        "neither TICONNECT_IPC_BUS_ADDRESS nor DBUS_SESSION_BUS_ADDRESS is set".to_owned(),
        Some("Launch TiConnect from a desktop session or set DBUS_SESSION_BUS_ADDRESS."),
    ))
}

fn display() -> Result<Check> {
    if let Some(mode) = test_override("TICONNECT_DOCTOR_DISPLAY")? {
        return match mode.as_str() {
            "wayland" => Ok(Check::new(
                "display",
                Status::Pass,
                "test override reports Wayland".to_owned(),
                None,
            )),
            "x11" => Ok(Check::new(
                "display",
                Status::Pass,
                "test override reports X11".to_owned(),
                None,
            )),
            "missing" => Ok(Check::new(
                "display",
                Status::Fail,
                "no Wayland or X11 display is configured".to_owned(),
                Some("Run the UI from a graphical session with WAYLAND_DISPLAY or DISPLAY set."),
            )),
            _ => bail!("invalid TICONNECT_DOCTOR_DISPLAY override: {mode}"),
        };
    }

    let mut backends = Vec::new();
    if let Some(name) = env::var_os("WAYLAND_DISPLAY")
        && let Some(name) = name.to_str()
        && !name.is_empty()
    {
        match wayland_socket(name) {
            Some(path) if path.exists() => {
                backends.push(format!("Wayland socket {}", path.display()));
            }
            _ => {
                return Ok(Check::new(
                    "display",
                    Status::Fail,
                    format!("WAYLAND_DISPLAY={name} does not point to an existing socket"),
                    Some("Restart the Wayland session or unset the stale WAYLAND_DISPLAY value."),
                ));
            }
        }
    }
    if let Some(display) = env::var_os("DISPLAY")
        && let Some(display) = display.to_str()
        && !display.is_empty()
    {
        backends.push(format!("X11 display {display}"));
    }

    if backends.is_empty() {
        Ok(Check::new(
            "display",
            Status::Fail,
            "neither WAYLAND_DISPLAY nor DISPLAY is set".to_owned(),
            Some("Run the UI from a graphical session with WAYLAND_DISPLAY or DISPLAY set."),
        ))
    } else {
        Ok(Check::new(
            "display",
            Status::Pass,
            backends.join("; "),
            None,
        ))
    }
}

fn wayland_socket(name: &str) -> Option<PathBuf> {
    let path = Path::new(name);
    if path.is_absolute() {
        return Some(path.to_path_buf());
    }
    let runtime = env::var_os("XDG_RUNTIME_DIR")?.to_str()?.to_owned();
    Some(Path::new(&runtime).join(name))
}

fn desktop() -> Result<Check> {
    let desktop =
        env::var_os("XDG_CURRENT_DESKTOP").and_then(|value| value.to_str().map(str::to_owned));
    let session =
        env::var_os("XDG_SESSION_TYPE").and_then(|value| value.to_str().map(str::to_owned));
    match (desktop, session) {
        (Some(desktop), Some(session)) => Ok(Check::new(
            "desktop",
            Status::Pass,
            format!("desktop {desktop}; session {session}"),
            None,
        )),
        (Some(desktop), None) => Ok(Check::new(
            "desktop",
            Status::Warn,
            format!("desktop {desktop}; session type is unknown"),
            Some("Remote desktop and tray behavior depend on the active session."),
        )),
        (None, Some(session)) => Ok(Check::new(
            "desktop",
            Status::Warn,
            format!("session {session}; desktop is unknown"),
            Some("Remote desktop and tray behavior depend on the active desktop."),
        )),
        (None, None) => Ok(Check::new(
            "desktop",
            Status::Warn,
            "XDG_CURRENT_DESKTOP and XDG_SESSION_TYPE are not set".to_owned(),
            Some("Remote desktop and tray behavior depend on the active desktop."),
        )),
    }
}

fn audio() -> Result<Check> {
    if let Some(mode) = test_override("TICONNECT_DOCTOR_AUDIO")? {
        return match mode.as_str() {
            "available" => Ok(Check::new(
                "audio",
                Status::Pass,
                "test override reports an audio device".to_owned(),
                None,
            )),
            "missing" => Ok(Check::new(
                "audio",
                Status::Warn,
                "test override reports no audio device".to_owned(),
                Some("Calls need ALSA, PipeWire, or PulseAudio; messaging does not."),
            )),
            _ => bail!("invalid TICONNECT_DOCTOR_AUDIO override: {mode}"),
        };
    }

    let mut devices = Vec::new();
    if Path::new("/dev/snd").exists() {
        devices.push("ALSA /dev/snd".to_owned());
    }
    if let Some(runtime) = env::var_os("XDG_RUNTIME_DIR")
        && let Some(runtime) = runtime.to_str()
    {
        let runtime = Path::new(runtime);
        if runtime.join("pulse/native").exists() {
            devices.push("PulseAudio socket".to_owned());
        }
        if runtime.join("pipewire-0").exists() {
            devices.push("PipeWire socket".to_owned());
        }
    }
    if devices.is_empty() {
        Ok(Check::new(
            "audio",
            Status::Warn,
            "no ALSA, PulseAudio, or PipeWire device was found".to_owned(),
            Some("Calls need ALSA, PipeWire, or PulseAudio; messaging does not."),
        ))
    } else {
        Ok(Check::new("audio", Status::Pass, devices.join("; "), None))
    }
}

fn youtube() -> Result<Check> {
    if let Some(mode) = test_override("TICONNECT_DOCTOR_YTDLP")? {
        return match mode.as_str() {
            "available" => Ok(Check::new(
                "youtube",
                Status::Pass,
                "test override reports yt-dlp".to_owned(),
                None,
            )),
            "missing" => Ok(Check::new(
                "youtube",
                Status::Warn,
                "test override reports no yt-dlp".to_owned(),
                Some("YouTube playback needs the bundled yt-dlp executable."),
            )),
            _ => bail!("invalid TICONNECT_DOCTOR_YTDLP override: {mode}"),
        };
    }

    let configured =
        env::var_os("TICONNECT_YTDLP").and_then(|value| value.to_str().map(PathBuf::from));
    let binary = configured.unwrap_or_else(|| PathBuf::from("yt-dlp"));
    match find_executable(&binary) {
        Some(path) => Ok(Check::new(
            "youtube",
            Status::Pass,
            format!("yt-dlp at {}", path.display()),
            None,
        )),
        None => Ok(Check::new(
            "youtube",
            Status::Warn,
            format!("{} was not found", binary.display()),
            Some("YouTube playback needs the bundled yt-dlp executable."),
        )),
    }
}

fn media_plugins() -> Result<Check> {
    if let Some(mode) = test_override("TICONNECT_DOCTOR_GSTREAMER")? {
        return match mode.as_str() {
            "available" => Ok(Check::new(
                "media-plugins",
                Status::Pass,
                "test override reports GStreamer plugins".to_owned(),
                None,
            )),
            "missing" => Ok(Check::new(
                "media-plugins",
                Status::Warn,
                "test override reports missing GStreamer plugins".to_owned(),
                Some("Voice and YouTube playback need GStreamer plugins."),
            )),
            _ => bail!("invalid TICONNECT_DOCTOR_GSTREAMER override: {mode}"),
        };
    }

    let Some(inspector) = find_executable(Path::new("gst-inspect-1.0")) else {
        return Ok(Check::new(
            "media-plugins",
            Status::Warn,
            "gst-inspect-1.0 was not found".to_owned(),
            Some("Voice and YouTube playback need GStreamer plugins."),
        ));
    };
    let mut missing = Vec::new();
    for plugin in ["appsink", "pulsesrc", "avdec_h264", "avdec_aac"] {
        let status = Command::new(&inspector)
            .arg(plugin)
            .output()
            .with_context(|| format!("inspect GStreamer plugin {plugin}"))?;
        if !status.status.success() {
            missing.push(plugin);
        }
    }
    if missing.is_empty() {
        Ok(Check::new(
            "media-plugins",
            Status::Pass,
            format!("gst-inspect-1.0 at {}", inspector.display()),
            None,
        ))
    } else {
        Ok(Check::new(
            "media-plugins",
            Status::Warn,
            format!("missing GStreamer plugins: {}", missing.join(", ")),
            Some("Voice and YouTube playback need GStreamer plugins."),
        ))
    }
}

fn config() -> Result<Check> {
    let settings = Settings::load().context("load settings")?;
    let dir = settings.config_dir();
    match fs::metadata(dir) {
        Ok(metadata) if metadata.is_dir() => Ok(Check::new(
            "config",
            Status::Pass,
            format!("config directory {}", dir.display()),
            None,
        )),
        Ok(_) => Ok(Check::new(
            "config",
            Status::Fail,
            format!("{} is not a directory", dir.display()),
            Some("Point TICONNECT_CONFIG_DIR at a writable directory."),
        )),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Check::new(
            "config",
            Status::Warn,
            format!("config directory {} does not exist yet", dir.display()),
            Some("TiConnect creates it when saving settings."),
        )),
        Err(error) => Ok(Check::new(
            "config",
            Status::Fail,
            format!("cannot inspect {}: {error}", dir.display()),
            Some("Point TICONNECT_CONFIG_DIR at a readable directory."),
        )),
    }
}

fn find_executable(binary: &Path) -> Option<PathBuf> {
    if binary.components().count() > 1 {
        return is_executable(binary).then(|| binary.to_path_buf());
    }
    let path = env::var_os("PATH")?;
    let path = path.to_str()?;
    for dir in path.split(':') {
        let candidate = Path::new(dir).join(binary);
        if is_executable(&candidate) {
            return Some(candidate);
        }
    }
    None
}

#[cfg(unix)]
fn is_executable(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;

    fs::metadata(path)
        .is_ok_and(|metadata| metadata.is_file() && metadata.permissions().mode() & 0o111 != 0)
}

#[cfg(not(unix))]
fn is_executable(path: &Path) -> bool {
    fs::metadata(path).is_ok_and(|metadata| metadata.is_file())
}

fn test_override(name: &str) -> Result<Option<String>> {
    match env::var_os(name) {
        Some(value) => Ok(Some(
            value
                .to_str()
                .with_context(|| format!("{name} must be UTF-8"))?
                .to_owned(),
        )),
        None => Ok(None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn report_json_marks_failed_checks() {
        let checks = vec![
            Check::new("example-pass", Status::Pass, "ok".to_owned(), None),
            Check::new(
                "example-fail",
                Status::Fail,
                "broken".to_owned(),
                Some("Fix it."),
            ),
        ];
        let report = report_json(&checks);

        assert_eq!(report["failed"], true);
        assert_eq!(report["checks"][0]["status"], "pass");
        assert_eq!(report["checks"][1]["remedy"], "Fix it.");
    }

    #[test]
    fn report_human_lists_all_checks() {
        let checks = vec![
            Check::new("example-pass", Status::Pass, "ok".to_owned(), None),
            Check::new(
                "example-warn",
                Status::Warn,
                "uncertain".to_owned(),
                Some("Check it."),
            ),
        ];
        let report = report_human(&checks);

        assert!(report.contains("[pass] example-pass: ok"));
        assert!(report.contains("[warn] example-warn: uncertain"));
        assert!(report.contains("remedy: Check it."));
    }
}
