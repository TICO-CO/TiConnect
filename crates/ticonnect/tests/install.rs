use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};

struct Home {
    root: PathBuf,
}

impl Home {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let id = NEXT.fetch_add(1, Ordering::Relaxed);
        let root =
            std::env::temp_dir().join(format!("ticonnect-install-{}-{id}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).expect("create temp home");
        Self { root }
    }

    fn bin(&self) -> PathBuf {
        self.root.join(".local/bin/ticonnect")
    }

    fn data(&self) -> PathBuf {
        self.root.join(".local/share")
    }

    fn config(&self) -> PathBuf {
        self.root.join(".config")
    }

    fn command(&self, args: &[&str]) -> Command {
        let mut command = Command::new(env!("CARGO_BIN_EXE_ticonnect"));
        command
            .args(args)
            .env("HOME", &self.root)
            .env("XDG_DATA_HOME", self.data())
            .env("XDG_CONFIG_HOME", self.config())
            .env("XDG_STATE_HOME", self.root.join(".local/state"))
            .env("TICONNECT_INSTALL_NO_SYSTEMD", "1")
            .env("TICONNECT_INSTALL_SOURCE", std::env::current_exe().unwrap())
            .env_remove("APPIMAGE")
            .env_remove("TICONNECT_APPIMAGE");
        command
    }

    fn run(&self, args: &[&str]) -> Output {
        self.command(args).output().expect("run installer")
    }
}

impl Drop for Home {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

#[test]
fn install_creates_user_surface() {
    let home = Home::new();
    assert_success(&home.run(&["--install-user"]));

    assert!(home.bin().is_file(), "wrapper missing");
    assert!(is_executable(&home.bin()), "wrapper is not executable");
    assert!(
        home.data()
            .join("applications/org.ticonnect.TiConnect.desktop")
            .is_file()
    );
    assert!(
        home.data()
            .join("icons/hicolor/scalable/apps/org.ticonnect.TiConnect.svg")
            .is_file()
    );
    assert!(
        home.data()
            .join("metainfo/org.ticonnect.TiConnect.metainfo.xml")
            .is_file()
    );
    assert!(
        home.config()
            .join("systemd/user/ticonnectd.service")
            .is_file()
    );
    assert!(home.config().join("autostart/ticonnect.desktop").is_file());
    assert!(home.data().join("ticonnect/install-manifest.txt").is_file());

    let wrapper = fs::read_to_string(home.bin()).expect("read wrapper");
    assert!(wrapper.contains("ticonnect/app/ticonnect"));
    assert!(wrapper.contains("# TiConnect user install"));
}

#[test]
fn install_substitutes_bindir_in_units_and_autostart() {
    let home = Home::new();
    assert_success(&home.run(&["--install-user"]));

    for path in [
        home.config().join("systemd/user/ticonnectd.service"),
        home.config().join("systemd/user/ticonnect-overlay.service"),
        home.config().join("systemd/user/ticonnect-ui.service"),
        home.config().join("autostart/ticonnect.desktop"),
        home.config().join("autostart/ticonnect-overlay.desktop"),
        home.config().join("autostart/ticonnect-ui.desktop"),
    ] {
        let contents = fs::read_to_string(&path).expect("read rendered file");
        assert!(!contents.contains("@BINDIR@"), "marker left in {path:?}");
        assert!(contents.contains("/.local/bin"));
    }
}

#[test]
fn reinstall_is_idempotent() {
    let home = Home::new();
    assert_success(&home.run(&["--install-user"]));
    assert_success(&home.run(&["--install-user"]));
    assert!(home.bin().is_file());
    assert!(home.data().join("ticonnect/install-manifest.txt").is_file());
}

#[test]
fn uninstall_removes_files_and_keeps_config_and_data() {
    let home = Home::new();
    assert_success(&home.run(&["--install-user"]));
    fs::create_dir_all(home.config().join("ticonnect")).unwrap();
    fs::write(home.config().join("ticonnect/config.toml"), "[dnd]\n").unwrap();
    fs::write(home.data().join("ticonnect/keep.txt"), "keep").unwrap();

    assert_success(&home.run(&["--uninstall-user"]));

    assert!(!home.bin().exists());
    assert!(
        !home
            .data()
            .join("applications/org.ticonnect.TiConnect.desktop")
            .exists()
    );
    assert!(
        !home
            .config()
            .join("systemd/user/ticonnectd.service")
            .exists()
    );
    assert!(home.config().join("ticonnect/config.toml").is_file());
    assert!(home.data().join("ticonnect/keep.txt").is_file());
}

#[test]
fn purge_removes_config_and_data_with_confirmation_flag() {
    let home = Home::new();
    assert_success(&home.run(&["--install-user"]));
    fs::create_dir_all(home.config().join("ticonnect")).unwrap();
    fs::write(home.config().join("ticonnect/identity.key"), "secret").unwrap();
    fs::write(home.data().join("ticonnect/ticonnect.db"), "db").unwrap();

    let output = home
        .command(&["--uninstall-user", "--purge"])
        .env("TICONNECT_INSTALL_ASSUME_YES", "1")
        .output()
        .expect("run purge");
    assert_success(&output);

    assert!(!home.config().join("ticonnect").exists());
    assert!(!home.data().join("ticonnect").exists());
}

#[test]
fn unrelated_wrapper_is_not_replaced_without_force() {
    let home = Home::new();
    fs::create_dir_all(home.root.join(".local/bin")).unwrap();
    fs::write(home.bin(), "#!/bin/sh\necho unrelated\n").unwrap();

    let output = home.run(&["--install-user"]);
    assert!(
        !output.status.success(),
        "install must refuse unrelated wrapper"
    );
    assert_eq!(
        fs::read_to_string(home.bin()).unwrap(),
        "#!/bin/sh\necho unrelated\n"
    );

    assert_success(&home.run(&["--install-user", "--force"]));
    assert!(
        fs::read_to_string(home.bin())
            .unwrap()
            .contains("# TiConnect user install")
    );
}

#[test]
fn dry_run_writes_nothing() {
    let home = Home::new();
    let output = home
        .command(&["--install-user", "--force"])
        .env("TICONNECT_INSTALL_DRY_RUN", "1")
        .output()
        .expect("run dry install");
    assert_success(&output);
    assert!(!home.bin().exists());
    assert!(!home.data().join("ticonnect/install-manifest.txt").exists());
    assert!(!home.config().join("autostart/ticonnect.desktop").exists());
}

fn assert_success(output: &Output) {
    assert!(
        output.status.success(),
        "status: {:?}\nstdout: {}\nstderr: {}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[cfg(unix)]
fn is_executable(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    fs::metadata(path)
        .map(|metadata| metadata.permissions().mode() & 0o111 != 0)
        .unwrap_or(false)
}

#[cfg(not(unix))]
fn is_executable(_path: &Path) -> bool {
    true
}

#[test]
fn appimage_payload_uses_extracted_apprun() {
    let home = Home::new();
    let appimage = home.root.join("fake.AppImage");
    fs::write(
        &appimage,
        "#!/bin/sh\nmkdir -p squashfs-root\nprintf '#!/bin/sh\\n' > squashfs-root/AppRun\nchmod 755 squashfs-root/AppRun\n",
    )
    .unwrap();
    set_executable(&appimage);

    let output = home
        .command(&["--install-user"])
        .env_remove("TICONNECT_INSTALL_SOURCE")
        .env("APPIMAGE", &appimage)
        .output()
        .expect("run appimage install");
    assert_success(&output);

    let wrapper = fs::read_to_string(home.bin()).expect("read wrapper");
    assert!(wrapper.contains("ticonnect/app/squashfs-root/AppRun"));
    assert!(
        home.data()
            .join("ticonnect/app/squashfs-root/AppRun")
            .is_file()
    );
}

#[test]
fn failed_extraction_falls_back_to_extract_and_run() {
    let home = Home::new();
    let appimage = home.root.join("broken.AppImage");
    fs::write(&appimage, "#!/bin/sh\nexit 1\n").unwrap();
    set_executable(&appimage);

    let output = home
        .command(&["--install-user"])
        .env_remove("TICONNECT_INSTALL_SOURCE")
        .env("APPIMAGE", &appimage)
        .output()
        .expect("run failed appimage install");
    assert_success(&output);

    let wrapper = fs::read_to_string(home.bin()).expect("read wrapper");
    assert!(wrapper.contains("ticonnect.AppImage"));
    assert!(wrapper.contains("--appimage-extract-and-run"));
    assert!(
        home.data()
            .join("ticonnect/app/ticonnect.AppImage")
            .is_file()
    );
}

#[cfg(unix)]
fn set_executable(path: &Path) {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(0o755)).unwrap();
}

#[cfg(not(unix))]
fn set_executable(_path: &Path) {}
