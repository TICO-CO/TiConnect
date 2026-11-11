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

- `ticonnectd` binary in `crates/runtime`.
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

- `crates/runtime/src/bin/ticonnectd.rs` — binary entrypoint with `anyhow` error
  reporting.
- `crates/runtime/src/daemon.rs` — bootstrap and daemon composition.
- `crates/daemon/src/service.rs` — D-Bus object implementation.
- `crates/daemon/src/runtime.rs` — D-Bus lifecycle, shutdown handling.
- `crates/cli/src/main.rs` — replace the Stage 1 stub with subcommands `status`,
  `version`, `ping`, `quit`, keeping `anyhow` error reporting.
- `packaging/systemd/ticonnectd.service`
- `packaging/autostart/ticonnect.desktop`
- `crates/core/src/ipc.rs` — shared D-Bus interface constants and error mapping.

## D-Bus interface (initial)

- [x] Bus name `org.ticonnect.Daemon`.
- [x] Object path `/org/ticonnect/Daemon`.
- [x] Interface `org.ticonnect.Daemon`.
- [x] Method `Ping() -> s` returns `pong`.
- [x] Method `GetVersion() -> s`.
- [x] Method `GetStatus() -> a{sv}` with `uptime_seconds`, `peer_count`,
  `outbox_depth`, `transport_state`.
- [x] Method `Shutdown()` stops the daemon cleanly.
- [x] Property `Version` (read).
- [x] Signal `DaemonStarted()`, `DaemonStopping()`.
- [x] D-Bus errors mapped from daemon and dependency errors to stable error names.

## Setup

- [x] Add `zbus` and `tokio` to the daemon crate.
- [x] Add `clap` to the CLI crate.
- [x] Create a bus-name acquisition helper that exits non-zero if another
  instance owns the name.
- [x] Add a single-instance guard using the well-known bus name.

## Implementation

- [x] Daemon starts, acquires the bus name, and emits `DaemonStarted`.
- [x] Runtime handles SIGTERM, SIGINT, and bus disconnection.
- [x] On shutdown, emit `DaemonStopping`, release the name, and exit `0`.
- [x] `GetStatus` returns live values from a shared state struct.
- [x] CLI `status` prints human-readable output and supports `--json`.
- [x] CLI `ping` exits non-zero with a clear message when the daemon is absent.
- [x] CLI `quit` calls `Shutdown`.
- [x] Daemon logs startup, shutdown, and D-Bus errors via `tracing`.
- [x] Daemon runs correctly with `DISPLAY` and `WAYLAND_DISPLAY` unset.

## systemd and autostart

- [x] Unit is `Type=dbus` or `Type=notify` with correct `BusName`.
- [x] Unit uses `ExecStart` pointing to the installed daemon path.
- [x] Unit sets `Restart=on-failure` and a sane `RestartSec`.
- [x] Unit includes hardening: `NoNewPrivileges=yes`, `PrivateTmp=yes`,
  `ProtectSystem=strict`, `ProtectHome=read-only` with writable config/data
  paths via `ReadWritePaths`.
- [x] Autostart `.desktop` uses `X-GNOME-Autostart-enabled=true` and
  `Hidden=false`.
- [x] Install script or Makefile target installs both files for the current user.
- [x] Document `systemctl --user daemon-reload` and enable steps.

## Tests

- [x] Integration test starts the daemon on a private session bus, calls
  `Ping`, `GetVersion`, `GetStatus`, and `Shutdown`.
- [x] Test second instance fails to acquire the name and exits non-zero.
- [x] Test SIGTERM triggers clean shutdown and `DaemonStopping`.
- [x] Test CLI `status --json` parses and contains expected keys.
- [ ] Manual test: daemon starts from systemd user unit and survives logout/login. Automated tests and install steps are complete; host systemd verification is pending.

## Security

- [x] Unit runs as the user, never root.
- [x] No `--system` service, no polkit rule.
- [x] D-Bus policy does not widen access beyond the session bus default.
- [x] `Shutdown` is allowed only on the session bus.
- [x] Logs contain no environment dumps or secrets.

## Docs and acceptance

- [x] Document the D-Bus interface in `docs/ipc.md`.
- [x] Document install, enable, status, and uninstall commands.
- [x] Document how to run the daemon in the foreground for debugging.

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
