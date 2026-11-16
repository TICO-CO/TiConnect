use std::fs::{self, OpenOptions};
use std::io::{self, BufRead, IsTerminal, Write};
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result, bail};

const DESKTOP: &str = include_str!("../../../packaging/flatpak/org.ticonnect.TiConnect.desktop");
const ICON: &[u8] = include_bytes!("../../../packaging/flatpak/org.ticonnect.TiConnect.svg");
const METAINFO: &str =
    include_str!("../../../packaging/flatpak/org.ticonnect.TiConnect.metainfo.xml");
const DAEMON_UNIT: &str = include_str!("../../../packaging/systemd/ticonnectd.service");
const OVERLAY_UNIT: &str = include_str!("../../../packaging/systemd/ticonnect-overlay.service");
const UI_UNIT: &str = include_str!("../../../packaging/systemd/ticonnect-ui.service");
const DAEMON_AUTOSTART: &str = include_str!("../../../packaging/autostart/ticonnect.desktop");
const OVERLAY_AUTOSTART: &str =
    include_str!("../../../packaging/autostart/ticonnect-overlay.desktop");
const UI_AUTOSTART: &str = include_str!("../../../packaging/autostart/ticonnect-ui.desktop");

const WRAPPER_MARKER: &str = "# TiConnect user install";

struct Paths {
    data: PathBuf,
    config: PathBuf,
    state: PathBuf,
    bin: PathBuf,
    app: PathBuf,
    wrapper: PathBuf,
    desktop: PathBuf,
    icon: PathBuf,
    metainfo: PathBuf,
    units: PathBuf,
    autostart: PathBuf,
    manifest: PathBuf,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PayloadKind {
    Binary,
    AppImageRun,
    AppImageExtracted,
}

impl Paths {
    fn resolve() -> Result<Self> {
        let home = std::env::var_os("HOME")
            .map(PathBuf::from)
            .context("HOME is not set")?;
        let data = std::env::var_os("XDG_DATA_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| home.join(".local/share"));
        let config = std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| home.join(".config"));
        let state = std::env::var_os("XDG_STATE_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| home.join(".local/state"));
        let bin = std::env::var_os("TICONNECT_INSTALL_BIN_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| home.join(".local/bin"));
        let app = data.join("ticonnect/app");

        Ok(Self {
            data: data.clone(),
            config: config.clone(),
            state,
            wrapper: bin.join("ticonnect"),
            bin,
            app,
            desktop: data.join("applications/org.ticonnect.TiConnect.desktop"),
            icon: data.join("icons/hicolor/scalable/apps/org.ticonnect.TiConnect.svg"),
            metainfo: data.join("metainfo/org.ticonnect.TiConnect.metainfo.xml"),
            units: config.join("systemd/user"),
            autostart: config.join("autostart"),
            manifest: data.join("ticonnect/install-manifest.txt"),
        })
    }
}

pub fn install(force: bool) -> Result<()> {
    if is_root() && !force {
        bail!("refusing to install as root; run as your user or pass --force");
    }
    let paths = Paths::resolve()?;
    if paths.wrapper.exists() && !force {
        let existing = fs::read_to_string(&paths.wrapper).unwrap_or_default();
        if !is_our_wrapper(&existing) {
            bail!(
                "{} already exists and was not installed by ticonnect; pass --force to replace it",
                paths.wrapper.display()
            );
        }
    }
    if dry_run() {
        println!("Would install TiConnect to {}.", paths.bin.display());
        return Ok(());
    }

    let staging = paths
        .app
        .parent()
        .context("invalid application payload path")?
        .join(format!("app.new-{}", std::process::id()));
    let _ = fs::remove_dir_all(&staging);
    fs::create_dir_all(&staging).context("create application staging directory")?;

    let payload = match prepare_payload(&staging) {
        Ok(payload) => payload,
        Err(error) => {
            let _ = fs::remove_dir_all(&staging);
            return Err(error);
        }
    };

    if paths.app.exists() {
        fs::remove_dir_all(&paths.app).context("remove previous application payload")?;
    }
    fs::rename(&staging, &paths.app).context("activate application payload")?;

    write_wrapper(&paths, payload)?;

    let bindir = paths.bin.to_string_lossy();
    for (source, path) in [
        (DAEMON_UNIT, paths.units.join("ticonnectd.service")),
        (OVERLAY_UNIT, paths.units.join("ticonnect-overlay.service")),
        (UI_UNIT, paths.units.join("ticonnect-ui.service")),
        (DAEMON_AUTOSTART, paths.autostart.join("ticonnect.desktop")),
        (
            OVERLAY_AUTOSTART,
            paths.autostart.join("ticonnect-overlay.desktop"),
        ),
        (UI_AUTOSTART, paths.autostart.join("ticonnect-ui.desktop")),
    ] {
        write_bytes(&path, source.replace("@BINDIR@", &bindir).as_bytes(), 0o644)?;
    }

    write_bytes(&paths.desktop, DESKTOP.as_bytes(), 0o644)?;
    write_bytes(&paths.icon, ICON, 0o644)?;
    write_bytes(&paths.metainfo, METAINFO.as_bytes(), 0o644)?;

    let mut entries = vec![
        format!("F {}", paths.wrapper.display()),
        format!("F {}", paths.desktop.display()),
        format!("F {}", paths.icon.display()),
        format!("F {}", paths.metainfo.display()),
        format!("F {}", paths.units.join("ticonnectd.service").display()),
        format!(
            "F {}",
            paths.units.join("ticonnect-overlay.service").display()
        ),
        format!("F {}", paths.units.join("ticonnect-ui.service").display()),
        format!("F {}", paths.autostart.join("ticonnect.desktop").display()),
        format!(
            "F {}",
            paths.autostart.join("ticonnect-overlay.desktop").display()
        ),
        format!(
            "F {}",
            paths.autostart.join("ticonnect-ui.desktop").display()
        ),
        format!("D {}", paths.app.display()),
    ];
    entries.insert(0, format!("# ticonnect {}", env!("CARGO_PKG_VERSION")));
    write_atomic(&paths.manifest, entries.join("\n").as_bytes(), 0o600)?;

    enable_services();

    println!("Installed TiConnect.");
    println!("Launch it from your app menu.");
    println!("CLI: ticonnect status");
    Ok(())
}

pub fn uninstall(purge: bool, force: bool) -> Result<()> {
    if is_root() && !force {
        bail!("refusing to uninstall as root; run as your user or pass --force");
    }
    let paths = Paths::resolve()?;
    if purge {
        confirm_purge()?;
    }
    if dry_run() {
        println!("Would remove files listed in {}.", paths.manifest.display());
        if purge {
            println!(
                "Would purge {} and {}.",
                paths.config.join("ticonnect").display(),
                paths.data.join("ticonnect").display()
            );
        }
        return Ok(());
    }

    disable_services();

    let manifest = fs::read_to_string(&paths.manifest).unwrap_or_default();
    let mut parents = Vec::new();
    for line in manifest.lines() {
        let Some((kind, value)) = line.split_once(' ') else {
            continue;
        };
        let path = PathBuf::from(value);
        if kind == "F" {
            if let Some(parent) = path.parent() {
                parents.push(parent.to_path_buf());
            }
            let _ = fs::remove_file(&path);
        } else if kind == "D" {
            if let Some(parent) = path.parent() {
                parents.push(parent.to_path_buf());
            }
            let _ = fs::remove_dir_all(&path);
        }
    }
    let _ = fs::remove_file(&paths.manifest);

    parents.sort();
    parents.dedup();
    parents.sort_by_key(|path| std::cmp::Reverse(path.components().count()));
    for parent in parents {
        let _ = fs::remove_dir(&parent);
    }

    if purge {
        let _ = fs::remove_dir_all(paths.config.join("ticonnect"));
        let _ = fs::remove_dir_all(paths.data.join("ticonnect"));
        let _ = fs::remove_dir_all(paths.state.join("ticonnect"));
        println!("Purged TiConnect configuration and data.");
    } else {
        println!("Removed TiConnect.");
        println!(
            "Kept {} and {}.",
            paths.config.join("ticonnect").display(),
            paths.data.join("ticonnect").display()
        );
        println!("Run with --uninstall-user --purge to remove them.");
    }
    Ok(())
}

fn prepare_payload(staging: &Path) -> Result<PayloadKind> {
    if let Some(source) = std::env::var_os("TICONNECT_INSTALL_SOURCE") {
        let source = PathBuf::from(source);
        copy_executable(&source, &staging.join("ticonnect"))?;
        return Ok(PayloadKind::Binary);
    }
    if let Some(appimage) = std::env::var_os("APPIMAGE")
        .or_else(|| std::env::var_os("TICONNECT_APPIMAGE"))
        .map(PathBuf::from)
    {
        match extract_appimage(&appimage, staging) {
            Ok(()) => return Ok(PayloadKind::AppImageExtracted),
            Err(error) => {
                eprintln!("warning: could not extract AppImage: {error}");
            }
        }
        let destination = staging.join("ticonnect.AppImage");
        fs::copy(&appimage, &destination).context("copy AppImage payload")?;
        set_mode(&destination, 0o755)?;
        return Ok(PayloadKind::AppImageRun);
    }

    let current = std::env::current_exe().context("locate the ticonnect executable")?;
    copy_executable(&current, &staging.join("ticonnect"))?;
    Ok(PayloadKind::Binary)
}

fn extract_appimage(appimage: &Path, staging: &Path) -> Result<()> {
    let status = Command::new(appimage)
        .arg("--appimage-extract")
        .current_dir(staging)
        .status()
        .context("run AppImage extraction")?;
    if !status.success() {
        bail!("AppImage extraction exited with {status}");
    }
    if !staging.join("squashfs-root/AppRun").is_file() {
        bail!("AppImage extraction did not produce squashfs-root/AppRun");
    }
    Ok(())
}

fn write_wrapper(paths: &Paths, payload: PayloadKind) -> Result<()> {
    let (target, extra) = match payload {
        PayloadKind::Binary => (paths.app.join("ticonnect"), ""),
        PayloadKind::AppImageRun => (
            paths.app.join("ticonnect.AppImage"),
            " --appimage-extract-and-run",
        ),
        PayloadKind::AppImageExtracted => (paths.app.join("squashfs-root/AppRun"), ""),
    };
    let contents = format!(
        "#!/bin/sh\n{WRAPPER_MARKER}\nexec {}{extra} \"$@\"\n",
        shell_quote(&target)
    );
    write_atomic(&paths.wrapper, contents.as_bytes(), 0o755)
}

fn shell_quote(path: &Path) -> String {
    let value = path.to_string_lossy();
    format!("'{}'", value.replace('\'', "'\\''"))
}

fn is_our_wrapper(contents: &str) -> bool {
    contents.contains(WRAPPER_MARKER)
}

fn enable_services() {
    if std::env::var_os("TICONNECT_INSTALL_NO_SYSTEMD").is_some() {
        return;
    }
    if !command_ok("systemctl", &["--user", "daemon-reload"]) {
        return;
    }
    let _ = command_ok(
        "systemctl",
        &[
            "--user",
            "enable",
            "--now",
            "ticonnectd.service",
            "ticonnect-overlay.service",
        ],
    );
}

fn disable_services() {
    if std::env::var_os("TICONNECT_INSTALL_NO_SYSTEMD").is_some() {
        return;
    }
    let _ = command_ok(
        "systemctl",
        &[
            "--user",
            "disable",
            "--now",
            "ticonnectd.service",
            "ticonnect-overlay.service",
            "ticonnect-ui.service",
        ],
    );
    let _ = command_ok("systemctl", &["--user", "daemon-reload"]);
}

fn command_ok(program: &str, args: &[&str]) -> bool {
    Command::new(program)
        .args(args)
        .output()
        .is_ok_and(|output| output.status.success())
}

fn confirm_purge() -> Result<()> {
    if std::env::var_os("TICONNECT_INSTALL_ASSUME_YES").is_some() {
        return Ok(());
    }
    if !io::stdin().is_terminal() {
        bail!(
            "--purge needs confirmation; set TICONNECT_INSTALL_ASSUME_YES=1 for non-interactive use"
        );
    }
    eprint!("This permanently removes TiConnect configuration and data. Type 'yes' to continue: ");
    io::stderr().flush().ok();
    let mut answer = String::new();
    io::stdin()
        .lock()
        .read_line(&mut answer)
        .context("read purge confirmation")?;
    if answer.trim() != "yes" {
        bail!("purge cancelled");
    }
    Ok(())
}

fn dry_run() -> bool {
    std::env::var_os("TICONNECT_INSTALL_DRY_RUN").is_some()
}

fn is_root() -> bool {
    Command::new("id")
        .arg("-u")
        .output()
        .ok()
        .is_some_and(|output| output.status.success() && output.stdout.trim_ascii() == b"0")
}

fn copy_executable(source: &Path, destination: &Path) -> Result<()> {
    fs::copy(source, destination)
        .with_context(|| format!("copy {} to {}", source.display(), destination.display()))?;
    set_mode(destination, 0o755)
}

fn write_bytes(path: &Path, contents: &[u8], mode: u32) -> Result<()> {
    write_atomic(path, contents, mode)
}

fn write_atomic(path: &Path, contents: &[u8], mode: u32) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("create directory {}", parent.display()))?;
    }
    let temporary = path.with_extension(format!("tmp-{}", std::process::id()));
    {
        let mut options = OpenOptions::new();
        options.create(true).write(true).truncate(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(mode);
        }
        let mut file = options
            .open(&temporary)
            .with_context(|| format!("write {}", temporary.display()))?;
        file.write_all(contents)
            .with_context(|| format!("write {}", temporary.display()))?;
    }
    set_mode(&temporary, mode)?;
    fs::rename(&temporary, path).with_context(|| format!("replace {}", path.display()))?;
    Ok(())
}

#[cfg(unix)]
fn set_mode(path: &Path, mode: u32) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(mode))
        .with_context(|| format!("set permissions on {}", path.display()))
}

#[cfg(not(unix))]
fn set_mode(_path: &Path, _mode: u32) -> Result<()> {
    Ok(())
}
