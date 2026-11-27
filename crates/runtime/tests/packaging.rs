use std::fs;

#[test]
fn systemd_unit_has_required_lifecycle_and_hardening_settings() {
    let unit = read("packaging/systemd/ticonnectd.service");

    assert!(unit.contains("Type=dbus"));
    assert!(unit.contains("BusName=org.ticonnect.Daemon"));
    assert!(unit.contains("ExecStart=@BINDIR@/ticonnect daemon"));
    assert!(unit.contains("Restart=on-failure"));
    assert!(unit.contains("RestartSec=2"));
    assert!(unit.contains("NoNewPrivileges=yes"));
    assert!(unit.contains("PrivateTmp=yes"));
    assert!(unit.contains("ProtectSystem=strict"));
    assert!(unit.contains("ProtectHome=read-only"));
    assert!(unit.contains("ReadWritePaths=-%h/.config/ticonnect -%h/.local/share/ticonnect"));
    assert!(unit.contains("WantedBy=default.target"));
}

#[test]
fn autostart_entry_has_required_flags() {
    let entry = read("packaging/autostart/ticonnect.desktop");

    assert!(entry.contains("Exec=@BINDIR@/ticonnect daemon"));
    assert!(entry.contains("Hidden=false"));
    assert!(entry.contains("X-GNOME-Autostart-enabled=true"));
    assert!(entry.contains("NoDisplay=true"));
}

#[test]
fn makefile_substitutes_install_bindir() {
    let makefile = read("Makefile");

    assert!(makefile.contains("sed 's|@BINDIR@|$(BINDIR)|g'"));
    assert!(makefile.contains("packaging/systemd/ticonnectd.service"));
    assert!(makefile.contains("packaging/autostart/ticonnect.desktop"));
}

fn read(path: &str) -> String {
    fs::read_to_string(format!("{}/../../{path}", env!("CARGO_MANIFEST_DIR"))).expect("read file")
}

#[test]
fn hardened_units_cover_daemon_overlay_and_ui() {
    let daemon = read("packaging/systemd/ticonnectd.service");
    for key in [
        "NoNewPrivileges=yes",
        "PrivateTmp=yes",
        "ProtectSystem=strict",
        "ProtectHome=read-only",
        "ProtectKernelTunables=yes",
        "ProtectKernelModules=yes",
        "ProtectControlGroups=yes",
        "RestrictAddressFamilies=AF_UNIX AF_INET AF_INET6",
        "RestrictNamespaces=yes",
        "SystemCallFilter=@system-service",
        "UMask=0077",
    ] {
        assert!(daemon.contains(key), "missing {key}");
    }
    assert!(!daemon.contains("User=root"));

    for unit in [
        "packaging/systemd/ticonnect-overlay.service",
        "packaging/systemd/ticonnect-ui.service",
    ] {
        let contents = read(unit);
        assert!(contents.contains("ExecStart=@BINDIR@/ticonnect "));
        assert!(contents.contains("NoNewPrivileges=yes"));
        assert!(contents.contains("WantedBy=graphical-session.target"));
    }
}

#[test]
fn flatpak_manifest_installs_unified_binary_with_minimal_permissions() {
    let manifest = read("packaging/flatpak/org.ticonnect.TiConnect.yml");
    assert!(manifest.contains("command: ticonnect"));
    assert!(manifest.contains("install -Dm755 target/release/ticonnect /app/bin/ticonnect"));
    for permission in [
        "--share=network",
        "--socket=session-bus",
        "--socket=wayland",
        "--socket=fallback-x11",
        "--socket=pulseaudio",
        "--device=dri",
        "--filesystem=xdg-config/ticonnect:create",
        "--filesystem=xdg-data/ticonnect:create",
    ] {
        assert!(manifest.contains(permission), "missing {permission}");
    }
    assert!(manifest.contains("gtk4-layer-shell"));
    assert!(!manifest.contains("--filesystem=home"));
}

#[test]
fn makefile_installs_unified_binary_units_and_autostart_entries() {
    let makefile = read("Makefile");
    assert!(makefile.contains("install -Dm755 target/release/ticonnect $(BINDIR)/ticonnect"));
    for path in [
        "packaging/systemd/ticonnectd.service",
        "packaging/systemd/ticonnect-overlay.service",
        "packaging/systemd/ticonnect-ui.service",
        "packaging/autostart/ticonnect.desktop",
        "packaging/autostart/ticonnect-overlay.desktop",
        "packaging/autostart/ticonnect-ui.desktop",
    ] {
        assert!(makefile.contains(path), "missing {path}");
    }
}

#[test]
fn release_docs_and_flatpak_metadata_exist() {
    for path in [
        "docs/install.md",
        "docs/performance.md",
        "docs/remote-desktop.md",
        "docs/local-testing.md",
        "CHANGELOG.md",
        "packaging/flatpak/org.ticonnect.TiConnect.desktop",
        "packaging/flatpak/org.ticonnect.TiConnect.metainfo.xml",
        "packaging/flatpak/org.ticonnect.TiConnect.svg",
    ] {
        assert!(!read(path).is_empty(), "missing {path}");
    }
}

#[test]
fn desktop_asset_launches_the_unified_binary() {
    let desktop = read("packaging/flatpak/org.ticonnect.TiConnect.desktop");
    assert!(desktop.contains("Exec=ticonnect\n"));
    assert!(desktop.contains("TryExec=ticonnect\n"));
    assert!(desktop.contains("StartupWMClass=org.ticonnect.UI"));
    assert!(!desktop.contains("ticonnect ui"));
}

#[test]
fn makefile_installs_the_app_menu_surface() {
    let makefile = read("Makefile");
    for path in [
        "packaging/flatpak/org.ticonnect.TiConnect.desktop",
        "packaging/flatpak/org.ticonnect.TiConnect.svg",
        "packaging/flatpak/org.ticonnect.TiConnect.metainfo.xml",
    ] {
        assert!(makefile.contains(path), "missing {path}");
    }
    let uninstall = makefile
        .split("uninstall-user:")
        .nth(1)
        .expect("uninstall target");
    assert!(uninstall.contains("org.ticonnect.TiConnect.desktop"));
}

#[test]
fn dev_desktop_asset_launches_the_dev_wrapper() {
    let desktop = read("packaging/dev/org.ticonnect.TiConnect.Dev.desktop");
    assert!(desktop.contains("Name=TiConnect Dev\n"));
    assert!(desktop.contains("Exec=ticonnect-dev\n"));
    assert!(desktop.contains("TryExec=ticonnect-dev\n"));
    assert!(desktop.contains("Icon=org.ticonnect.TiConnect.Dev\n"));
    assert!(desktop.contains("StartupWMClass=org.ticonnect.UI.Dev"));
    assert!(!desktop.contains("NoDisplay=true"));
}

#[test]
fn dev_wrapper_marks_the_build_and_sets_its_title() {
    let wrapper = read("packaging/dev/ticonnect-dev");
    assert!(wrapper.contains("export TICONNECT_DEV_BUILD=1"));
    assert!(wrapper.contains("TICONNECT_UI_TITLE"));
    assert!(wrapper.contains("TiConnect Dev"));
    assert!(wrapper.contains("NIX_LD_LIBRARY_PATH"));
}

#[test]
fn makefile_installs_and_removes_the_dev_menu_surface() {
    let makefile = read("Makefile");
    let install_dev = makefile
        .split("install-dev:")
        .nth(1)
        .and_then(|rest| rest.split("\nuninstall-dev:").next())
        .expect("install-dev target");
    for path in [
        "packaging/dev/org.ticonnect.TiConnect.Dev.desktop",
        "packaging/flatpak/org.ticonnect.TiConnect.svg",
        "$(DEV_DESKTOP)",
        "$(DEV_ICON)",
    ] {
        assert!(install_dev.contains(path), "missing {path}");
    }

    let uninstall_dev = makefile
        .split("uninstall-dev:")
        .nth(1)
        .and_then(|rest| rest.split("\nrelease-bin:").next())
        .expect("uninstall-dev target");
    assert!(uninstall_dev.contains("$(DEV_DESKTOP)"));
    assert!(uninstall_dev.contains("$(DEV_ICON)"));
}

#[test]
fn appimage_installs_metainfo_and_desktop_asset() {
    let script = read("scripts/package-appimage.sh");
    assert!(script.contains("usr/share/metainfo"));
    assert!(script.contains("org.ticonnect.TiConnect.metainfo.xml"));
    assert!(script.contains("org.ticonnect.TiConnect.desktop"));
    assert!(script.contains("export APPDIR="));
}

#[test]
fn release_does_not_publish_the_internal_tarball() {
    let workflow = read(".github/workflows/release.yml");
    assert!(workflow.contains("Drop internal tarball"));
    let artifact = workflow
        .split("uses: actions/upload-artifact")
        .nth(1)
        .expect("artifact upload");
    assert!(artifact.contains("dist/*.AppImage"));
    assert!(artifact.contains("dist/*.pkg.tar.zst"));
    assert!(!artifact.contains("dist/ticonnect-*"));
}

#[test]
fn unified_help_hides_internal_commands() {
    let source = read("crates/ticonnect/src/lib.rs");
    let help = source
        .split("pub const HELP: &str = ")
        .nth(1)
        .and_then(|rest| rest.split(";\n").next())
        .expect("help constant");
    let words: Vec<&str> = help
        .split_whitespace()
        .map(|word| word.trim_matches(|c: char| !c.is_alphanumeric()))
        .collect();
    for hidden in ["daemon", "overlay", "cli"] {
        assert!(!words.contains(&hidden), "help mentions {hidden}");
    }
}

#[test]
fn interactive_lab_is_wired_and_documented() {
    let makefile = read("Makefile");
    assert!(makefile.contains("tests/local/lab.sh"));
    assert!(makefile.contains("lab-smoke"));
    let docs = read("docs/local-testing.md");
    assert!(docs.contains("make lab"));
    assert!(docs.contains("make lab-smoke"));
    assert!(!read("tests/local/lab.sh").is_empty());
}
