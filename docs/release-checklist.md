# Release checklist

- [ ] Version bumped in `Cargo.toml` and the Flatpak manifest.
- [ ] `CHANGELOG.md` updated.
- [ ] `cargo fmt --all -- --check` clean.
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` clean.
- [ ] `cargo test --workspace --all-targets` passes.
- [ ] `cargo xtask systems --check` passes.
- [ ] `cargo audit` clean or exceptions documented.
- [ ] Flatpak build reproducible on a clean VM.
- [ ] Install, upgrade, and uninstall tested on a clean VM.
- [ ] Two-node smoke test: pairing, messaging, file, call, remote desktop.
- [ ] X11 and Wayland smoke tests on GNOME and KDE.
- [ ] 24-hour soak test with stable memory and database integrity.
- [ ] Release notes written.
