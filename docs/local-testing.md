# Local integration tests

`tests/local/run.sh` runs the daemon, CLI, UI, and overlay locally against a
private D-Bus bus and isolated config/data directories.

## Environments

```sh
tests/local/run.sh x11 all
tests/local/run.sh weston all
tests/local/run.sh kwin all
tests/local/run.sh hyprland all
```

- `x11`: current X11 session, real XTEST, real PipeWire/Pulse devices.
- `weston`: nested headless Weston; validates GTK Wayland and non-layer-shell
  fallback. Weston does not implement layer-shell, so overlay fallback exit is
  expected.
- `kwin`: nested KWin Wayland; validates layer-shell overlay, portal services,
  and Wayland call/attachment/session paths.
- `hyprland`: current Hyprland session; validates layer-shell overlay, portal
  services, and the Wayland session paths.

Scenarios: `messaging`, `attachments`, `ui`, `call`, `call-ui`, `remote`, `all`.
Set `TICONNECT_TEST_AUDIO=null` for headless or CI runs. The `call-ui` scenario
requires Bun and drives both main-window bridges through a full call.

## Interactive two-node lab

Inside Zellij:

```sh
make lab
```

This adds a `ticonnect-lab-*` tab with two panes. Each pane is a separate node
with its own private IPC bus, daemon, config, data directory, transport port, and
daemon log. The UI and overlay keep using the host session bus for GTK, WebKit,
portals, and notifications. The original pane stays attached to the lab
supervisor. Closing the lab tab, running `lab-stop` in either pane, or pressing
Ctrl-C in the supervisor pane stops both nodes.

Interactive labs start the Vite dev server at `http://127.0.0.1:5173` and point
both UIs at it. Edits under `crates/ui/web` hot-reload without rebuilding the
Rust binary. The Vite log is `$LAB_DIR/vite.log`. An existing TiConnect dev
server on port `5173` is restarted automatically. Use `LAB_HOT_RELOAD=0 make lab`
to load the embedded bundle instead. Automated `make lab-smoke` always uses the
embedded bundle.

Pane commands:

```text
ticonnect status      talk to this node's daemon
user                  print the user node id
send "hello"          send to the user
inbox                 show messages from the user
ui                    start this node's UI
stop-ui               stop this node's UI
ovl                   start this node's overlay
stop-ovl              stop this node's overlay
logs                  show the daemon log
help-lab              list commands
```

The `ui` and `ovl` helpers call the hidden internal `ticonnect ui` and
`ticonnect overlay` entries. The rest use the same unified `ticonnect` binary as
a normal install.

Automatic LAN trust is always enabled, so the two nodes trust each other
automatically and the user list populates without manual pairing.

The lab sets distinct display names and UI window titles for the two nodes.
Both UIs can run at the same time against their separate buses. Both overlays
anchor top-right and overlap, so run one `ovl` at a time for clear testing.

Call audio uses null devices by default because two nodes on one host share the
same microphone and speakers. Real devices create an acoustic feedback loop.
For real call audio, use headphones and run `call-audio hardware` in both lab
panes, or start the lab with `LAB_CALL_AUDIO=hardware make lab`. Use
`call-audio silent` to return to null devices.

On Nix, the lab exports the Nix ALSA plugin directory so daemon sounds such as
message chimes, ringtones, and dial tones work with the host PipeWire/Pulse
server. Call voice audio still follows the `LAB_CALL_AUDIO` setting above.

Group watch parties need at least three nodes. The daemon integration test
starts three nodes itself:

```sh
cargo test -p ticonnect-daemon --test watch_party_group
```

For manual group testing, run `make lab` twice from the same workspace only
after stopping the first lab, or use the integration test. Voice recording and
YouTube playback need the GStreamer plugins exported by `nix develop`
(`GST_PLUGIN_SYSTEM_PATH_1_0`). Check with:

```sh
nix develop -c gst-inspect-1.0 appsink
nix develop -c gst-inspect-1.0 pulsesrc
```

Private D-Bus failures print the relevant `dbus-bus-*.log`; the harness resolves
the session config next to the `dbus-daemon` binary before falling back to
`--session`, which matters in Nix environments. Set `LAB_KEEP=1 make lab` to keep
the work directory and logs after the lab stops. Run the automated harness check
with:

```sh
make lab-smoke
```

On Nix, `nix develop` supplies the Rust toolchain, ALSA, GTK, WebKitGTK, Bun,
Node, and local integration test tools together with `pkg-config`. The shell uses
the Rust toolchain from nixpkgs; outside the shell, `rust-toolchain.toml` still
selects the host toolchain:

```sh
nix develop
make lab
```

Lab binary builds disable `sccache` by default. Set
`LAB_RUSTC_WRAPPER=sccache` if you want the wrapper back.

On Nix hosts that mix a host Rust toolchain with Nix GTK libraries, the link
step can report unresolved `GLIBC_2.43` math symbols from `graphene`. `make
lab` and `make lab-smoke` detect `NIX_LD` and pass
`-Wl,--allow-shlib-undefined` to the final `ticonnect` link. Force it with
`TICONNECT_ALLOW_UNDEFINED=1`, or disable it with
`TICONNECT_ALLOW_UNDEFINED=0`. A Nix build avoids the workaround:

```sh
nix build .#ticonnect
TICONNECT_BIN="$PWD/result/bin/ticonnect" make lab-smoke
```

## Cross-host dev install

For two real machines, run `make install-dev` on each peer and launch
**TiConnect Dev** from the app menu or run `ticonnect-dev` there. Each install
has its own config, data, state, and private IPC bus, so it can run beside a
normal installation. The dev transport port
defaults to UDP `45881`; allow UDP `5353` and `45870` for discovery plus `45881`
for transport between peers. Stop the normal app on both peers for a clean
two-node test.

## What is checked

- private per-node D-Bus; no global daemon name collision
- discovery, automatic trust, trusted session
- text round trip
- attachment transfer and content storage on disk
- UI and overlay process under X11 and Wayland
- call signaling and two-way Opus frame counters through real or virtual devices
- main-window bridge call accept in both directions
- remote consent, active state, indicator, kill switch
- build and run with real `gtk4-layer-shell`

## Real portal gate

The real XDG RemoteDesktop portal requires a human consent click. Run the
attended gate on KWin Wayland:

```sh
PORTAL_REQUIRE=1 tests/local/run.sh kwin remote
```

Approve the portal dialog on the nested KWin output. The test fails unless the
daemon records `remote desktop portal session started` and the session tears
down on the kill switch.

Run the same gate on a GNOME Wayland session and on X11 with an available
RemoteDesktop portal backend.

## Hyprland

Run the checks inside a Hyprland session:

```sh
tests/local/run.sh hyprland all
PORTAL_REQUIRE=1 tests/local/run.sh hyprland remote
```

Preflight the portal backend:

```sh
gdbus introspect --session --dest org.freedesktop.portal.Desktop \
  --object-path /org/freedesktop/portal/desktop | grep portal.RemoteDesktop
```

Hyprland needs a portal backend exposing
`org.freedesktop.portal.RemoteDesktop`. `xdg-desktop-portal-hyprland` support
landed upstream in 2026; older packages may lack it. The fallback is
`xdg-desktop-portal-luminous` with both RemoteDesktop and ScreenCast routed to
it. The layer-shell overlay is expected to work on Hyprland. Notification and
tray validation depends on `mako` and `waybar`.

## Requirements

X11: `Xvfb`, `xdotool`, `gtk4-layer-shell`, PipeWire/Pulse.

Wayland: `weston`, `kwin_wayland`, `xdg-desktop-portal`,
`xdg-desktop-portal-kde`, `wtype` (optional), `grim`.

Audio tests use the active PipeWire/Pulse server. The harness links the host
runtime sockets into the isolated runtime directory.
