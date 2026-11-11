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
