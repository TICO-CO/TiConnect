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

Scenarios: `messaging`, `attachments`, `ui`, `call`, `remote`, `all`.

## Interactive two-node lab

Inside Zellij:

```sh
make lab
```

This adds a `ticonnect-lab-*` tab with two panes. Each pane is a separate node
with its own private D-Bus, daemon, config, data directory, transport port, and
daemon log. The original pane stays attached to the lab supervisor. Closing the
lab tab, running `lab-stop` in either pane, or pressing Ctrl-C in the supervisor
pane stops both nodes.

Pane commands:

```text
ticonnect status      talk to this node's daemon
peer                  print the peer node id
send "hello"          send to the peer
inbox                 show messages from the peer
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

Open LAN mode is enabled by default, so the two nodes trust each other
automatically and the peer list populates without pairing. Set
`LAB_OPEN_LAN=0 make lab` to test manual pairing instead; the `pair` helper is
still available in that mode.

Manual pairing remains available:

```sh
# node-a pane
ticonnect pair start
# copy the PIN, then in node-b
ticonnect pair with-pin <node-a-id> <pin>
```

The lab sets distinct display names and UI window titles for the two nodes.
Both UIs can run at the same time against their separate buses. Both overlays
anchor bottom-right and overlap, so run one `ovl` at a time for clear testing.

Set `LAB_KEEP=1 make lab` to keep the work directory and logs after the lab
stops. Run the automated harness check with:

```sh
make lab-smoke
```

On Nix, `nix develop` supplies the ALSA, GTK, WebKitGTK, Bun, and Node development
outputs together with `pkg-config`, while keeping the Rust toolchain selected
by `rust-toolchain.toml`:

```sh
nix develop
make lab
```

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

## What is checked

- private per-node D-Bus; no global daemon name collision
- discovery, PIN pairing, trusted session
- text round trip
- attachment transfer and content storage on disk
- UI and overlay process under X11 and Wayland
- call signaling and two-way Opus frame counters through real PipeWire devices
- remote consent, PIN, active state, indicator, kill switch
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
