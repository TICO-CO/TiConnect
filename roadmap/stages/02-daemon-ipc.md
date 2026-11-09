# Stage 2 — Daemon & IPC

## Goal

Turn the skeleton into a headless daemon that owns `org.ticonnect.Daemon` on the
session bus, expose a minimal control API, ship a debugging CLI, and install as
a systemd user unit plus XDG autostart entry. The daemon must run with no UI and
no display server.

## Systems and components

Component IDs and relations: [stage component map](../../docs/systems/generated/stage-map.md#stage-2).
Component graphs: [RT](../../docs/systems/generated/components/rt.md), [IPC](../../docs/systems/generated/components/ipc.md), [CLI](../../docs/systems/generated/components/cli.md).

## Scope

- `ticonnectd` binary in `crates/daemon`.
- `zbus` service with methods, properties, and signals.
- `ticonnect-cli` that talks to the daemon over D-Bus.
- systemd user unit and XDG autostart entry.
- Health, version, and status reporting.
- Graceful shutdown on SIGTERM and D-Bus name loss.

## Non-goals

- No identity, discovery, pairing, or messaging yet.
- No tray icon.
- No GTK4 code.
- No real peers; peer methods may return empty results.

## Prerequisites

Stages 1 and 1.5 complete.

## Deliverables

- `crates/daemon/src/main.rs` — replace the Stage 1 stub with runtime setup and
  `anyhow` error reporting.
- `crates/daemon/src/service.rs` — D-Bus object implementation.
- `crates/daemon/src/runtime.rs` — tokio runtime, shutdown handling.
- `crates/cli/src/main.rs` — replace the Stage 1 stub with subcommands `status`,
  `version`, `ping`, `quit`, keeping `anyhow` error reporting.
- `packaging/systemd/ticonnectd.service`
- `packaging/autostart/ticonnect.desktop`
- `crates/core/src/ipc.rs` — shared D-Bus interface constants and error mapping.

## D-Bus interface (initial)

- [ ] Bus name `org.ticonnect.Daemon`.
- [ ] Object path `/org/ticonnect/Daemon`.
- [ ] Interface `org.ticonnect.Daemon`.
- [ ] Method `Ping() -> s` returns `pong`.
- [ ] Method `GetVersion() -> s`.
- [ ] Method `GetStatus() -> a{sv}` with `uptime_seconds`, `peer_count`,
  `outbox_depth`, `transport_state`.
- [ ] Method `Shutdown()` stops the daemon cleanly.
- [ ] Property `Version` (read).
- [ ] Signal `DaemonStarted()`, `DaemonStopping()`.
- [ ] D-Bus errors mapped from daemon and dependency errors to stable error names.

## Setup

- [ ] Add `zbus` and `tokio` to the daemon crate.
- [ ] Add `clap` to the CLI crate.
- [ ] Create a bus-name acquisition helper that exits non-zero if another
  instance owns the name.
- [ ] Add a single-instance guard using the well-known bus name.

## Implementation

- [ ] Daemon starts, acquires the bus name, and emits `DaemonStarted`.
- [ ] Runtime handles SIGTERM, SIGINT, and bus disconnection.
- [ ] On shutdown, emit `DaemonStopping`, release the name, and exit `0`.
- [ ] `GetStatus` returns live values from a shared state struct.
- [ ] CLI `status` prints human-readable output and supports `--json`.
- [ ] CLI `ping` exits non-zero with a clear message when the daemon is absent.
- [ ] CLI `quit` calls `Shutdown`.
- [ ] Daemon logs startup, shutdown, and D-Bus errors via `tracing`.
- [ ] Daemon runs correctly with `DISPLAY` and `WAYLAND_DISPLAY` unset.

## systemd and autostart

- [ ] Unit is `Type=dbus` or `Type=notify` with correct `BusName`.
- [ ] Unit uses `ExecStart` pointing to the installed daemon path.
- [ ] Unit sets `Restart=on-failure` and a sane `RestartSec`.
- [ ] Unit includes hardening: `NoNewPrivileges=yes`, `PrivateTmp=yes`,
  `ProtectSystem=strict`, `ProtectHome=read-only` with writable config/data
  paths via `ReadWritePaths`.
- [ ] Autostart `.desktop` uses `X-GNOME-Autostart-enabled=true` and
  `Hidden=false`.
- [ ] Install script or Makefile target installs both files for the current user.
- [ ] Document `systemctl --user daemon-reload` and enable steps.

## Tests

- [ ] Integration test starts the daemon on a private session bus, calls
  `Ping`, `GetVersion`, `GetStatus`, and `Shutdown`.
- [ ] Test second instance fails to acquire the name and exits non-zero.
- [ ] Test SIGTERM triggers clean shutdown and `DaemonStopping`.
- [ ] Test CLI `status --json` parses and contains expected keys.
- [ ] Manual test: daemon starts from systemd user unit and survives logout/login.

## Security

- [ ] Unit runs as the user, never root.
- [ ] No `--system` service, no polkit rule.
- [ ] D-Bus policy does not widen access beyond the session bus default.
- [ ] `Shutdown` is allowed only on the session bus.
- [ ] Logs contain no environment dumps or secrets.

## Docs and acceptance

- [ ] Document the D-Bus interface in `docs/ipc.md`.
- [ ] Document install, enable, status, and uninstall commands.
- [ ] Document how to run the daemon in the foreground for debugging.

## Acceptance criteria

- `ticonnectd` runs headless with no display server.
- `ticonnect-cli status` reports version and uptime.
- `ticonnect-cli quit` stops the daemon cleanly.
- systemd user unit starts the daemon after login.
- XDG autostart entry starts the daemon in a desktop session without systemd.

## Demo

1. Start the daemon in a terminal.
2. Run `ticonnect-cli status` and `ticonnect-cli ping`.
3. Stop the daemon with `ticonnect-cli quit`; observe clean logs.
4. Enable the user unit and show `systemctl --user status ticonnectd`.

## Risks and mitigations

- Risk: D-Bus API churn later. Mitigation: keep initial API minimal and version
  the interface; add methods per stage.
- Risk: systemd and autostart both launching the daemon. Mitigation: bus-name
  single-instance guard makes the second launch exit harmlessly.

## Works without later stages

The daemon answers status and ping with no peers, transport, or UI present.
