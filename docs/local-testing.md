# Local integration tests

`tests/local/run.sh` runs the daemon, CLI, UI, and overlay locally against a
private D-Bus bus and isolated config/data directories.

## Environments

```sh
tests/local/run.sh x11 all
tests/local/run.sh weston all
tests/local/run.sh kwin all
```

- `x11`: current X11 session, real XTEST, real PipeWire/Pulse devices.
- `weston`: nested headless Weston; validates GTK Wayland and non-layer-shell
  fallback. Weston does not implement layer-shell, so overlay fallback exit is
  expected.
- `kwin`: nested KWin Wayland; validates layer-shell overlay, portal services,
  and Wayland call/attachment/session paths.

Scenarios: `messaging`, `attachments`, `ui`, `call`, `remote`, `all`.

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

## Requirements

X11: `Xvfb`, `xdotool`, `gtk4-layer-shell`, PipeWire/Pulse.

Wayland: `weston`, `kwin_wayland`, `xdg-desktop-portal`,
`xdg-desktop-portal-kde`, `wtype` (optional), `grim`.

Audio tests use the active PipeWire/Pulse server. The harness links the host
runtime sockets into the isolated runtime directory.
