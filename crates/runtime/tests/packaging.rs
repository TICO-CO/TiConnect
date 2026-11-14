use std::fs;

#[test]
fn systemd_unit_has_required_lifecycle_and_hardening_settings() {
    let unit = read("packaging/systemd/ticonnectd.service");

    assert!(unit.contains("Type=dbus"));
    assert!(unit.contains("BusName=org.ticonnect.Daemon"));
    assert!(unit.contains("ExecStart=@BINDIR@/ticonnectd"));
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

    assert!(entry.contains("Exec=@BINDIR@/ticonnectd"));
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
        assert!(contents.contains("ExecStart=@BINDIR@/ticonnect-"));
        assert!(contents.contains("NoNewPrivileges=yes"));
        assert!(contents.contains("WantedBy=graphical-session.target"));
    }
}

#[test]
fn flatpak_manifest_installs_all_binaries_with_minimal_permissions() {
    let manifest = read("packaging/flatpak/org.ticonnect.TiConnect.yml");
    for binary in [
        "ticonnectd",
        "ticonnect-ui",
        "ticonnect-ovl",
        "ticonnect-cli",
    ] {
        assert!(manifest.contains(binary), "missing {binary}");
    }
    for permission in [
        "--share=network",
        "--socket=session-bus",
        "--socket=wayland",
        "--socket=fallback-x11",
        "--device=dri",
        "--filesystem=xdg-config/ticonnect:create",
        "--filesystem=xdg-data/ticonnect:create",
    ] {
        assert!(manifest.contains(permission), "missing {permission}");
    }
    assert!(manifest.contains("gtk4-layer-shell"));
    assert!(!manifest.contains("--filesystem=home"));
    assert!(!manifest.contains("--socket=pulseaudio"));
}

#[test]
fn makefile_installs_all_binaries_units_and_autostart_entries() {
    let makefile = read("Makefile");
    for binary in [
        "ticonnectd",
        "ticonnect-ui",
        "ticonnect-ovl",
        "ticonnect-cli",
    ] {
        assert!(makefile.contains(binary), "missing {binary}");
    }
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
