use std::fs::{self, OpenOptions};
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use anyhow::{Context, bail};
use ticonnect_core::ipc::client::DaemonProxy;

const PING_TIMEOUT: Duration = Duration::from_millis(400);
const READY_TIMEOUT: Duration = Duration::from_secs(5);
const READY_INTERVAL: Duration = Duration::from_millis(100);

pub fn ensure_daemon() -> anyhow::Result<()> {
    ensure_daemon_with(spawn_daemon)
}

pub fn ensure_daemon_with(mut spawn: impl FnMut() -> anyhow::Result<()>) -> anyhow::Result<()> {
    ensure_daemon_with_timeout(&mut spawn, READY_TIMEOUT)
}

pub fn ensure_daemon_with_timeout(
    spawn: impl FnMut() -> anyhow::Result<()>,
    timeout: Duration,
) -> anyhow::Result<()> {
    ensure_daemon_with_options(spawn, timeout, autostart_disabled())
}

pub fn ensure_daemon_with_options(
    mut spawn: impl FnMut() -> anyhow::Result<()>,
    timeout: Duration,
    autostart_disabled: bool,
) -> anyhow::Result<()> {
    if daemon_present() {
        return Ok(());
    }
    if autostart_disabled {
        return Ok(());
    }

    let log = log_path();
    spawn()?;
    wait_for_daemon(timeout, &log)
}

fn autostart_disabled() -> bool {
    std::env::var_os("TICONNECT_NO_AUTOSTART").is_some_and(|value| value == "1")
}

fn daemon_present() -> bool {
    let Ok(runtime) = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    else {
        return false;
    };
    runtime.block_on(daemon_present_async())
}

async fn daemon_present_async() -> bool {
    let Ok(connection) = ticonnect_core::ipc::connect().await else {
        return false;
    };
    let Ok(proxy) = DaemonProxy::new(&connection).await else {
        return false;
    };
    tokio::time::timeout(PING_TIMEOUT, proxy.ping())
        .await
        .is_ok_and(|result| result.is_ok())
}

fn wait_for_daemon(timeout: Duration, log: &Path) -> anyhow::Result<()> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .context("create daemon readiness runtime")?;
    runtime.block_on(async {
        let deadline = Instant::now() + timeout;
        loop {
            if daemon_present_async().await {
                return Ok(());
            }
            if Instant::now() >= deadline {
                bail!(
                    "daemon did not become ready within {} ms; see {}",
                    timeout.as_millis(),
                    log.display()
                );
            }
            tokio::time::sleep(READY_INTERVAL).await;
        }
    })
}

fn spawn_daemon() -> anyhow::Result<()> {
    let executable = main_executable();
    let log = log_path();
    if let Some(parent) = log.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("create log directory {}", parent.display()))?;
        set_private_dir(parent)?;
    }

    let stderr = open_log(&log).map_or(Stdio::null(), Stdio::from);
    let mut command = Command::new("setsid");
    command.arg(&executable).arg("daemon");
    configure(&mut command, &log, stderr);

    match command.spawn() {
        Ok(_) => Ok(()),
        Err(error) if error.kind() == ErrorKind::NotFound => {
            let stderr = open_log(&log).map_or(Stdio::null(), Stdio::from);
            let mut fallback = Command::new(&executable);
            fallback.arg("daemon");
            configure(&mut fallback, &log, stderr);
            fallback
                .spawn()
                .with_context(|| format!("start {} daemon", executable.display()))?;
            Ok(())
        }
        Err(error) => Err(error).context("start ticonnect daemon"),
    }
}

fn configure(command: &mut Command, log: &Path, stderr: Stdio) {
    if std::env::var_os("TICONNECT_LOG_FILE").is_none() {
        command.env("TICONNECT_LOG_FILE", log);
    }
    command
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(stderr);
}

fn open_log(path: &Path) -> Option<std::fs::File> {
    let mut options = OpenOptions::new();
    options.create(true).append(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    options.open(path).ok()
}

#[cfg(unix)]
fn set_private_dir(path: &Path) -> anyhow::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(0o700))
        .with_context(|| format!("set permissions on {}", path.display()))
}

#[cfg(not(unix))]
fn set_private_dir(_path: &Path) -> anyhow::Result<()> {
    Ok(())
}

pub fn main_executable() -> PathBuf {
    std::env::current_exe().map_or_else(
        |_| PathBuf::from("ticonnect"),
        |current| resolve_executable(&current),
    )
}

fn resolve_executable(current: &Path) -> PathBuf {
    if current.file_name().is_some_and(|name| name == "ticonnect") {
        return current.to_path_buf();
    }
    if let Some(parent) = current.parent() {
        let sibling = parent.join("ticonnect");
        if sibling.is_file() {
            return sibling;
        }
    }
    PathBuf::from("ticonnect")
}

pub fn log_path() -> PathBuf {
    if let Some(path) = std::env::var_os("TICONNECT_LOG_FILE") {
        return PathBuf::from(path);
    }
    let base = std::env::var_os("XDG_STATE_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".local/state")))
        .unwrap_or_else(std::env::temp_dir);
    base.join("ticonnect").join("daemon.log")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn main_executable_is_used_when_already_resolved() {
        let path = Path::new("/tmp/ticonnect");
        assert_eq!(resolve_executable(path), path);
    }

    #[test]
    fn sibling_unified_binary_is_preferred() {
        let dir = std::env::temp_dir().join(format!("ticonnect-exe-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("create temp dir");
        let unified = dir.join("ticonnect");
        fs::write(&unified, "").expect("write sibling");

        assert_eq!(resolve_executable(&dir.join("ticonnect-ovl")), unified);

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn missing_sibling_falls_back_to_path_lookup() {
        let dir =
            std::env::temp_dir().join(format!("ticonnect-exe-missing-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("create temp dir");

        assert_eq!(
            resolve_executable(&dir.join("ticonnect-ovl")),
            PathBuf::from("ticonnect")
        );

        let _ = fs::remove_dir_all(&dir);
    }
}
