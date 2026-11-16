# TiConnect distribution refactor - detailed plan

## Target contract

```text
ticonnect                     -> open GUI; start daemon if absent
ticonnect --version           -> print binary version
ticonnect --help              -> user help only
ticonnect <command> [...]     -> CLI command
ticonnect version             -> print binary version (friendly alias)
ticonnect cli <command>       -> hidden compatibility alias
ticonnect daemon              -> hidden internal process entry
ticonnect overlay             -> hidden internal process entry
ticonnect ui                  -> hidden internal process entry
ticonnect --install-user      -> install per-user app integration
ticonnect --uninstall-user    -> remove installed integration
```

No user-facing help text mentions `daemon`, `overlay`, `cli`, systemd, D-Bus, or AppImage internals.

## Decisions

- Primary artifact: AppImage with `--install-user` / `--uninstall-user`.
- One binary: `ticonnect`.
- Release artifacts: AppImage, Arch package, Nix flake/module.
- Tarball is an internal build input for the Arch package only.
- Flatpak stays in the tree as experimental, not a release gate.
- Wayland validation target: Hyprland VM or second machine.
- Host validation: Mint X11 plus Docker; Wayland and true LAN elsewhere.
- Daemon lifecycle: systemd/autostart when installed; auto-spawn fallback.
- Internal commands keep their names but are hidden from help.

## Current findings

- `ticonnect` opens the UI with no args (`crates/ticonnect/src/main.rs:25`), but `--help` advertises `daemon`, `ui`, `overlay`, `cli`. There is no `--version`.
- The UI does not start the daemon. If it is absent, the banner says "Daemon unavailable. Reconnecting..." (`crates/ui/src/app.rs:243`).
- The overlay spawns hardcoded `ticonnect-ui` from `PATH` (`crates/overlay/src/client.rs:203`). Release packages install only `ticonnect`, so click-to-open is broken.
- `packaging/flatpak/org.ticonnect.TiConnect.desktop` uses `Exec=ticonnect ui`; the AppImage-generated entry uses `Exec=ticonnect`.
- `make install-user` installs the binary, units, and autostart, but not the desktop entry, icon, or metainfo. Source installs get no app-menu entry.
- Docs teach internals: `ticonnect daemon`, `ticonnect ui`, `ticonnect cli status`, `systemctl --user start...`.
- Host validation is ready for X11: GTK4, libadwaita, gtk4-layer-shell, ALSA, PipeWire, dbus, Docker, `desktop-file-validate`, `appstreamcli`, `flatpak`, `gtk-launch`.
- Host is missing: `Xvfb`, `xdotool`, `weston`, `kwin_wayland`, `flatpak-builder`. No Wayland session on the host.

---

## Workstream 1 - Unified dispatcher

### New structure

- `crates/ticonnect/src/main.rs` becomes a thin wrapper.
- Add `crates/ticonnect/src/lib.rs` with testable parsing and execution.

```rust
pub enum Action {
    App,
    Ui,
    Overlay,
    Daemon,
    Cli(Vec<OsString>),
    Help,
    Version,
    InstallUser,
    UninstallUser { purge: bool },
}
```

### Parse rules

Order matters:

```text
no args                         -> App
--help | -h | help              -> Help
--version | -V | version        -> Version
--install-user                  -> InstallUser
--uninstall-user [--purge]      -> UninstallUser
daemon                          -> Daemon
overlay                         -> Overlay
ui                              -> Ui
cli ...                         -> Cli(args after cli)
anything else                   -> Cli(all args)
```

Details:

- `ticonnect cli version` still reaches the old CLI `version` command (daemon version).
- `ticonnect version` prints the binary version without contacting the daemon.
- `ticonnect status --help` delegates to CLI help for `status`, not unified help.
- Unknown flags before delegation produce exit code `2` and the unified help.
- Reconstruct CLI argv as `["ticonnect", ...]` so clap help says `ticonnect`, not `ticonnect-cli`.

### Execution rules

| Action | Behavior |
|---|---|
| `App` | `ensure_daemon()` best effort; then `ticonnect_ui::run()` |
| `Ui` | Same as `App` |
| `Overlay` | Check graphical session; `ensure_daemon()` best effort; then `ticonnect_overlay::run()` |
| `Daemon` | Tokio runtime; `ticonnect_runtime::run_daemon()` |
| `Cli` | Skip daemon ensure for `settings`; otherwise `ensure_daemon()` best effort; then `ticonnect_cli::entry::run()` |
| `Help` | Print unified help; exit `0` |
| `Version` | Print `CARGO_PKG_VERSION`; exit `0` |
| `InstallUser` | Installer; no daemon |
| `UninstallUser` | Installer; no daemon |

### User help

```text
Usage:
  ticonnect              Launch TiConnect
  ticonnect <command>    Use the command-line client
  ticonnect --version
  ticonnect --help

Install:
  ticonnect --install-user
  ticonnect --uninstall-user [--purge]
```

### Tests

Add `crates/ticonnect/tests/dispatch.rs`:

- no args -> `App`
- `--version` -> `Version`
- `version` -> `Version`
- `--help` -> `Help`
- `daemon` -> `Daemon`
- `overlay` -> `Overlay`
- `ui` -> `Ui`
- `cli status` -> `Cli(["status"])`
- `status` -> `Cli(["status"])`
- `pair start` -> `Cli(["pair","start"])`
- `status --help` -> `Cli(["status","--help"])`
- `--install-user` -> `InstallUser`
- `--uninstall-user --purge` -> `UninstallUser { purge: true }`

Add binary tests:

- `ticonnect --help` stdout contains `Launch TiConnect`.
- stdout does not contain `daemon`, `overlay`, or `cli`.
- `ticonnect --version` prints the workspace version.
- exit code checks for unknown commands.

---

## Workstream 2 - Daemon auto-start and overlay fix

### New module

Add `crates/runtime/src/app.rs`:

```rust
pub fn ensure_daemon() -> Result<()>;
pub fn ensure_daemon_with(spawn: impl FnMut() -> Result<()>) -> Result<()>;
```

`ensure_daemon_with` exists for tests and dependency injection.

Add dependencies to `crates/runtime/Cargo.toml`:

- `ticonnect-core`
- `zbus`
- `tokio` features: `process`, `time`

### Detection

- Connect to the session bus with `zbus`.
- Build `ticonnect_core::ipc::client::DaemonProxy`.
- Call `ping()` with a short timeout, around 400 ms.
- If successful, return `Ok(())`.
- If `TICONNECT_NO_AUTOSTART=1`, return `Ok(())` without spawning.

### Spawn

- Resolve `std::env::current_exe()`.
- Prefer `setsid` when available:

```text
setsid <current-exe> daemon
```

- Fallback:

```text
<current-exe> daemon
```

- Use `stdin(Stdio::null())` and `stdout(Stdio::null())`.
- Redirect stderr to a log path when possible.
- If `TICONNECT_LOG_FILE` is unset, set it to:

```text
$XDG_STATE_HOME/ticonnect/daemon.log
```

Fallback:

```text
$HOME/.local/state/ticonnect/daemon.log
```

- Create the directory with mode `0700`.
- Do not override an existing `TICONNECT_LOG_FILE`.
- Do not use `unsafe`; invoking `setsid` is enough. `crates/runtime` has `#![forbid(unsafe_code)]`.

### Readiness wait

- Poll `ping()` every 100 ms for up to 5 seconds.
- On timeout, return an error containing the log path.
- App mode logs the warning but still opens the UI.
- CLI mode returns the error to the user.

### Duplicate protection

- The daemon already acquires `org.ticonnect.Daemon` with `replace_existing_names(false)`, so a second daemon exits.
- Do not add a separate lock file.
- If the child exits because the name is taken, treat it as success after the readiness check passes.

### Overlay changes

`crates/overlay/src/client.rs`:

- Remove `Command::new("ticonnect-ui")`.
- Add helper:

```rust
fn ui_command() -> std::process::Command {
    let exe = std::env::current_exe().unwrap_or_else(|_| "ticonnect".into());
    let mut command = std::process::Command::new(exe);
    command.arg("ui");
    command
}
```

- Use `ui_command().spawn()`.
- Add a unit test asserting the argument is `ui`.
- This fixes packaged installs that only contain `ticonnect`.

`crates/overlay/src/lib.rs`:

- Keep the existing graphical-session check first.
- After the check passes, call `ticonnect_runtime::ensure_daemon()` best effort.
- Then start the GTK application.

### UI changes

`crates/ui/src/app.rs` and `crates/ui/src/client.rs` need no behavior change if `ensure_daemon()` runs in the dispatcher. Keep the existing reconnect banner for crash recovery.

### Tests

Add `crates/runtime/tests/autostart.rs`:

- `TICONNECT_NO_AUTOSTART=1` with no bus does not spawn.
- fake spawn callback is called once when daemon is absent.
- fake spawn callback is not called when a test daemon is present.
- readiness timeout returns an error that mentions the log path.

Add overlay test:

- `ui_command()` points at the current executable and passes `ui`.

Add CLI tests:

- `settings list` does not require a daemon.
- `status` attempts daemon ensure and reports a clear error when the bus is absent.

---

## Workstream 3 - `--install-user` / `--uninstall-user`

### Location

Add `crates/ticonnect/src/install.rs`.

Embed canonical assets at compile time:

```rust
const DESKTOP: &str = include_str!("../../../packaging/flatpak/org.ticonnect.TiConnect.desktop");
const ICON: &[u8] = include_bytes!("../../../packaging/flatpak/org.ticonnect.TiConnect.svg");
const METAINFO: &str = include_str!("../../../packaging/flatpak/org.ticonnect.TiConnect.metainfo.xml");
const DAEMON_UNIT: &str = include_str!("../../../packaging/systemd/ticonnectd.service");
const OVERLAY_UNIT: &str = include_str!("../../../packaging/systemd/ticonnect-overlay.service");
const UI_UNIT: &str = include_str!("../../../packaging/systemd/ticonnect-ui.service");
const DAEMON_AUTOSTART: &str = include_str!("../../../packaging/autostart/ticonnect.desktop");
const OVERLAY_AUTOSTART: &str = include_str!("../../../packaging/autostart/ticonnect-overlay.desktop");
const UI_AUTOSTART: &str = include_str!("../../../packaging/autostart/ticonnect-ui.desktop");
```

This makes the binary self-contained for installer purposes.

### XDG paths

| Item | Path |
|---|---|
| Wrapper | `$HOME/.local/bin/ticonnect` |
| App payload | `$XDG_DATA_HOME/ticonnect/app/` |
| Desktop entry | `$XDG_DATA_HOME/applications/org.ticonnect.TiConnect.desktop` |
| Icon | `$XDG_DATA_HOME/icons/hicolor/scalable/apps/org.ticonnect.TiConnect.svg` |
| Metainfo | `$XDG_DATA_HOME/metainfo/org.ticonnect.TiConnect.metainfo.xml` |
| systemd units | `$XDG_CONFIG_HOME/systemd/user/` |
| Autostart | `$XDG_CONFIG_HOME/autostart/` |
| Manifest | `$XDG_DATA_HOME/ticonnect/install-manifest.txt` |
| Daemon log | `$XDG_STATE_HOME/ticonnect/daemon.log` |

Fallbacks:

```text
$XDG_DATA_HOME   -> $HOME/.local/share
$XDG_CONFIG_HOME -> $HOME/.config
$XDG_STATE_HOME  -> $HOME/.local/state
```

### Install flow

1. Reject `euid == 0` unless explicit `--force`.
2. Resolve paths.
3. Refuse to overwrite `~/.local/bin/ticonnect` if it exists and is not our wrapper, unless `--force`.
4. Prepare payload:
   - If `APPIMAGE` env is set: run the AppImage with `--appimage-extract` in `$XDG_DATA_HOME/ticonnect/app/` and use `squashfs-root/AppRun`.
   - If extraction fails: copy the AppImage and write a wrapper using `--appimage-extract-and-run`.
   - If `APPIMAGE` is unset: copy `current_exe()` to `$XDG_DATA_HOME/ticonnect/app/ticonnect`.
5. Write wrapper `~/.local/bin/ticonnect` mode `0755`:

```sh
#!/bin/sh
exec "$HOME/.local/share/ticonnect/app/squashfs-root/AppRun" "$@"
```

For the binary fallback:

```sh
#!/bin/sh
exec "$HOME/.local/share/ticonnect/app/ticonnect" "$@"
```

6. Render templates:
   - replace `@BINDIR@` with `$HOME/.local/bin`
   - write daemon, overlay, UI units with mode `0644`
   - write daemon and overlay autostart entries with mode `0644`
7. Write desktop, icon, metainfo.
8. Write manifest listing every installed path.
9. Enable services when available:

```text
systemctl --user daemon-reload
systemctl --user enable --now ticonnectd.service ticonnect-overlay.service
```

Skip silently when `systemctl` is absent or the user bus is unavailable. Autostart entries remain the fallback.

10. Print:

```text
Installed TiConnect.
Launch it from your app menu.
CLI: ticonnect status
```

### Uninstall flow

1. Stop and disable services when available.
2. Read manifest.
3. Remove only manifest-listed files.
4. Remove empty generated directories.
5. Remove manifest.
6. Keep `$XDG_CONFIG_HOME/ticonnect` and `$XDG_DATA_HOME/ticonnect` by default.
7. `--purge` requires confirmation and removes:
   - config dir
   - data dir
   - identity
   - database
   - attachments
8. Print what was kept and how to remove it.

### Upgrade behavior

- Re-running `--install-user` replaces payload, wrapper, desktop assets, units, autostart.
- Config, data, and identity are untouched.
- Manifest is replaced atomically.
- Version is recorded in the manifest for debugging.

### Test hooks

Support these env vars:

```text
TICONNECT_INSTALL_NO_SYSTEMD=1
TICONNECT_INSTALL_SOURCE=/path/to/file
TICONNECT_INSTALL_BIN_DIR=/tmp/bin
TICONNECT_INSTALL_DRY_RUN=1
```

Tests set `HOME`, `XDG_CONFIG_HOME`, `XDG_DATA_HOME`, `XDG_STATE_HOME`.

### Tests

Add `crates/ticonnect/tests/install.rs`:

- temp HOME install creates wrapper, desktop, icon, metainfo, units, autostart, manifest.
- wrapper is executable and points to installed payload.
- `@BINDIR@` is fully substituted; no template markers remain.
- re-install is idempotent.
- uninstall removes installed files.
- uninstall keeps config and data.
- `--purge` removes config and data only after confirmation/flag.
- existing unrelated `~/.local/bin/ticonnect` is not overwritten without `--force`.
- dry run writes nothing.

Add CI smoke with temp HOME and `--install-user`, then `desktop-file-validate` and `gtk-launch` under Xvfb.

---

## Workstream 4 - Packaging and release, file by file

### `packaging/flatpak/org.ticonnect.TiConnect.desktop`

Change:

```ini
Exec=ticonnect
TryExec=ticonnect
StartupWMClass=org.ticonnect.UI
```

No `ui` argument.

### `Makefile`

Add install directories:

```text
$(PREFIX)/share/applications
$(PREFIX)/share/icons/hicolor/scalable/apps
$(PREFIX)/share/metainfo
```

Install:

```text
packaging/flatpak/org.ticonnect.TiConnect.desktop
packaging/flatpak/org.ticonnect.TiConnect.svg
packaging/flatpak/org.ticonnect.TiConnect.metainfo.xml
```

Keep existing unit/autostart substitution. Add the same files to `uninstall-user`.

Add a `check-user-surface` target if useful:

```sh
target/release/ticonnect --help | grep -v -E 'daemon|overlay|cli'
```

### `scripts/package-appimage.sh`

- Add metainfo install:

```text
$APPDIR/usr/share/metainfo/org.ticonnect.TiConnect.metainfo.xml
```

- Generated desktop entry: add `TryExec=ticonnect`.
- Keep `Exec=ticonnect`.
- Ensure `APPIMAGE` and `APPDIR` env survive AppRun so `--install-user` can locate the AppImage.
- Verify no-arg launch opens the UI.
- Verify `--help` hides internals.

### `scripts/package-arch.sh`

- Keep desktop, icon, metainfo, units, autostart installation.
- Use the updated desktop asset.
- No user-facing tarball docs.
- Keep the tarball only as build input.

### `flake.nix`

- Keep desktop, icon, metainfo `postInstall`.
- Keep `mainProgram = "ticonnect"`.
- Add release check or document manual:

```sh
nix build .#ticonnect
nix flake check
```

### `nix/module.nix`

- No command changes; units continue using `${cfg.package}/bin/ticonnect daemon` and `overlay`.
- Optional: add UI autostart package asset if needed.

### `.github/workflows/release.yml`

Build and validate:

```sh
target/release/ticonnect --version
target/release/ticonnect --help
desktop-file-validate packaging/flatpak/org.ticonnect.TiConnect.desktop
appstreamcli validate packaging/flatpak/org.ticonnect.TiConnect.metainfo.xml
systemd-analyze --user verify packaging/systemd/*.service
```

Assert help hides internals:

```sh
if target/release/ticonnect --help | grep -E '^\s*(daemon|overlay|cli)\b'; then exit 1; fi
```

Artifacts:

- upload `*.AppImage`
- upload `*.pkg.tar.zst`
- upload `SHA256SUMS.txt`
- keep the tarball only inside the job for `package-arch.sh`
- remove the tarball from release files and install docs

Add version-tag consistency check:

```sh
TAG_VERSION="${GITHUB_REF_NAME#v}"
WORKSPACE_VERSION="$(cargo metadata --no-deps --format-version 1 | jq -r '.packages[] | select(.name=="ticonnect") | .version')"
[ "$TAG_VERSION" = "$WORKSPACE_VERSION" ]
```

### `.github/workflows/ci.yml`

Add packages:

```text
desktop-file-utils
appstream
systemd
```

Add checks:

```sh
desktop-file-validate packaging/flatpak/org.ticonnect.TiConnect.desktop
appstreamcli validate packaging/flatpak/org.ticonnect.TiConnect.metainfo.xml
systemd-analyze --user verify packaging/systemd/*.service
target/debug/ticonnect --help
target/debug/ticonnect --version
```

Add AppImage install smoke:

- run `--install-user` into temp HOME
- validate desktop file
- start installed wrapper under Xvfb
- confirm daemon process appears
- confirm second launch exits or focuses existing UI

---

## Workstream 5 - Documentation

### `README.md`

Replace the current "Run" section with:

1. Install
   - AppImage + `--install-user`
   - Arch
   - Nix
2. Use
   - launch TiConnect from app menu
   - pairing, messaging, calls, remote desktop
3. Command line
   - top-level examples:
     - `ticonnect status`
     - `ticonnect id show`
     - `ticonnect pair start`
     - `ticonnect send <peer> "hello"`
     - `ticonnect peers list`
4. Development / internals
   - `ticonnect daemon`
   - `ticonnect overlay`
   - `ticonnect ui`
   - `ticonnect cli ...`
   - local test harness
   - LAN Docker suite

### `docs/install.md`

Rewrite around user paths:

```sh
chmod +x ticonnect-v0.0.5-x86_64.AppImage
./ticonnect-v0.0.5-x86_64.AppImage --install-user
# launch TiConnect from the app menu
```

Then:

- Arch package
- Nix flake/module
- advanced CLI
- uninstall and purge
- troubleshooting:
  - `systemctl --user status ticonnectd`
  - `journalctl --user -u ticonnectd`
  - `$XDG_STATE_HOME/ticonnect/daemon.log`
  - `ticonnect status`
- Flatpak marked experimental
- remove tarball quickstart

### `docs/local-testing.md`

Add Hyprland:

```sh
tests/local/run.sh hyprland all
PORTAL_REQUIRE=1 tests/local/run.sh hyprland remote
```

Add portal preflight:

```sh
gdbus introspect --session --dest org.freedesktop.portal.Desktop \
  --object-path /org/freedesktop/portal/desktop | grep portal.RemoteDesktop
```

Document:

- Hyprland needs a portal backend exposing `org.freedesktop.portal.RemoteDesktop`.
- `xdg-desktop-portal-hyprland` support landed upstream in 2026; older packages may lack it.
- fallback is `xdg-desktop-portal-luminous` with both RemoteDesktop and ScreenCast routed to it.
- layer-shell overlay is expected to work on Hyprland.
- notification and tray validation depends on `mako` and `waybar`.

### `docs/release-checklist.md`

Add:

- `ticonnect --help` hides internals.
- `ticonnect --version` matches the tag.
- AppImage `--install-user` succeeds in a clean HOME.
- app menu launches the UI.
- daemon and overlay start after install.
- `--uninstall-user` leaves config and data.
- `desktop-file-validate`, `appstreamcli`, and `systemd-analyze` pass.
- Hyprland portal preflight is recorded.

---

## Workstream 6 - Test matrix

### Unit and integration

| Area | Test |
|---|---|
| Dispatcher | `crates/ticonnect/tests/dispatch.rs` |
| Unified help/version | binary integration tests |
| Daemon ensure | `crates/runtime/tests/autostart.rs` |
| Overlay UI spawn | `crates/overlay/tests/card.rs` or new test |
| Installer | `crates/ticonnect/tests/install.rs` |
| Packaging assets | extend `crates/runtime/tests/packaging.rs` |
| CLI compatibility | existing `crates/cli/tests/*` |
| UI behavior | existing `crates/ui/tests/*` |

### Packaging assertions

Extend `crates/runtime/tests/packaging.rs`:

- desktop contains `Exec=ticonnect` and `TryExec=ticonnect`
- desktop does not contain `ticonnect ui`
- Makefile installs desktop, icon, and metainfo
- AppImage script installs metainfo
- release workflow does not publish the tarball as a user artifact
- units and autostart still use hidden `daemon` and `overlay`
- help text in `crates/ticonnect` contains no internal names

### CI commands

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo build --workspace --bins
dbus-run-session -- cargo test --workspace --all-targets
cargo xtask systems --check
desktop-file-validate packaging/flatpak/org.ticonnect.TiConnect.desktop
appstreamcli validate packaging/flatpak/org.ticonnect.TiConnect.metainfo.xml
systemd-analyze --user verify packaging/systemd/*.service
tests/local/run.sh x11 messaging
tests/local/run.sh x11 attachments
tests/local/run.sh x11 ui
tests/local/run.sh x11 remote
make lan-test
```

---

## Validation runbook

### Host X11

```sh
cargo build --release --bin ticonnect
cargo test --workspace --all-targets
tests/local/run.sh x11 all
make lan-test
```

Expected:

- all tests pass
- no top-level help mentions internal commands
- local harness passes messaging, attachments, UI, calls, and remote signaling
- Docker LAN suite passes

### AppImage install in temp HOME

```sh
TMP_HOME="$(mktemp -d)"
HOME="$TMP_HOME" XDG_CONFIG_HOME="$TMP_HOME/.config" \
  XDG_DATA_HOME="$TMP_HOME/.local/share" \
  ./dist/ticonnect-v0.0.5-x86_64.AppImage --install-user

test -x "$TMP_HOME/.local/bin/ticonnect"
test -f "$TMP_HOME/.local/share/applications/org.ticonnect.TiConnect.desktop"
test -f "$TMP_HOME/.config/systemd/user/ticonnectd.service"
test -f "$TMP_HOME/.config/autostart/ticonnect.desktop"

HOME="$TMP_HOME" gtk-launch org.ticonnect.TiConnect
```

Expected:

- UI opens
- daemon starts
- second launch reuses the UI
- `--uninstall-user` removes installed files
- config and data remain

### Hyprland VM or second machine

```sh
# inside a Hyprland session
gdbus introspect --session --dest org.freedesktop.portal.Desktop \
  --object-path /org/freedesktop/portal/desktop | grep portal.RemoteDesktop

tests/local/run.sh hyprland messaging
tests/local/run.sh hyprland attachments
tests/local/run.sh hyprland ui
tests/local/run.sh hyprland call
PORTAL_REQUIRE=1 tests/local/run.sh hyprland remote
tests/local/run.sh hyprland all
```

Expected:

- overlay uses layer-shell
- messaging, attachments, and calls pass
- portal consent appears for remote sessions when RemoteDesktop exists
- missing RemoteDesktop produces a clear error, not a vague timeout

Then run the true LAN check on two Hyprland or VM nodes.

---

## Risks and decisions to confirm

1. `ticonnect version` prints the binary version; `ticonnect cli version` keeps daemon-version behavior.
2. The installer hardcodes `~/.local/bin` for the wrapper.
3. The tarball stops being a user-facing download and is kept only for Arch packaging.
4. Flatpak stays experimental and out of release gates.
5. Hyprland remote desktop requires a portal with `org.freedesktop.portal.RemoteDesktop`; no capture fallback will be added.
6. AppImage self-install extracts once to `$XDG_DATA_HOME/ticonnect/app/`; reinstall replaces it.

---

## Implementation order

1. Record baseline failures with the host validation commands.
2. Refactor the unified dispatcher and top-level CLI.
3. Add `ensure_daemon` and fix overlay spawn.
4. Implement `--install-user` and `--uninstall-user`.
5. Fix packaging consistency and the release workflow.
6. Update docs, tests, and CI.
7. Re-run host validation.
8. Validate on Hyprland VM or second machine.
9. Validate a true two-machine LAN session.
