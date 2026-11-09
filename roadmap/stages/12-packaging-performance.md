# Stage 12 — Packaging & Performance

## Goal

Ship TiConnect as a Flatpak and as distro-installable binaries with hardened
systemd user units, complete documentation, and measured idle and runtime
performance. Close the release checklist and the security review.

## Systems and components

Component IDs and relations: [stage component map](../../docs/systems/generated/stage-map.md#stage-12).
Architecture checks: [docs/systems](../../docs/systems/README.md).

## Scope

- Flatpak manifest for daemon, UI, overlay, and CLI.
- Build and install documentation.
- systemd unit hardening review.
- Idle memory and CPU measurement and tuning.
- Runtime performance pass for messaging, transfers, calls, and remote desktop.
- Dependency audit and license review.
- Release checklist and versioning.

## Non-goals

- No new product features.
- No Windows or macOS packaging.
- No 4K support.
- No auto-update service beyond Flatpak's normal mechanism.

## Prerequisites

Stages 1, 1.5, and 2–11 complete.

## Deliverables

- `packaging/flatpak/org.ticonnect.TiConnect.yml`
- `packaging/flatpak/` metadata and desktop files.
- `packaging/systemd/` hardened units.
- `packaging/autostart/` final entries.
- `docs/install.md`, `docs/performance.md`, `docs/security.md`.
- `CHANGELOG.md` and release checklist.
- Benchmark scripts under `scripts/bench/`.

## Flatpak

- [ ] Manifest builds all four binaries in one build.
- [ ] Runtime pinned to a supported Freedesktop or GNOME runtime.
- [ ] Permissions are minimal:
  - [ ] `--share=network` for LAN transport.
  - [ ] `--socket=session-bus` for D-Bus.
  - [ ] `--socket=wayland` and `--socket=fallback-x11`.
  - [ ] `--device=dri` only if required by capture/encoding.
  - [ ] No filesystem access beyond the app data and config dirs.
  - [ ] No `--socket=pulseaudio` unless required; document why if added.
- [ ] Portal usage works inside the sandbox (RemoteDesktop, notifications,
  autostart, tray as available).
- [ ] Flatpak builds and runs on GNOME and KDE, X11 and Wayland.
- [ ] Desktop entries and icons installed.
- [ ] Autostart works inside and outside Flatpak.
- [ ] Document Flatpak-specific limitations.

## systemd hardening

- [ ] `NoNewPrivileges=yes`.
- [ ] `PrivateTmp=yes`.
- [ ] `ProtectSystem=strict`.
- [ ] `ProtectHome=read-only` with explicit `ReadWritePaths` for config and data.
- [ ] `ProtectKernelTunables=yes`, `ProtectKernelModules=yes`,
  `ProtectControlGroups=yes`.
- [ ] `RestrictAddressFamilies=AF_UNIX AF_INET AF_INET6`.
- [ ] `RestrictNamespaces=yes` where compatible with portals.
- [ ] `SystemCallFilter=@system-service` with a tested allowlist.
- [ ] Unit passes `systemd-analyze security` with a documented score.
- [ ] Overlay and UI units or autostart entries are separate and optional.
- [ ] No unit runs as root.

## Performance

- [ ] Idle daemon memory under 30 MB.
- [ ] Overlay idle memory measured and documented at about 40 MB.
- [ ] Idle CPU under 1% on a modern laptop.
- [ ] Messaging send/receive latency measured on LAN.
- [ ] Attachment transfer throughput measured at several file sizes.
- [ ] Voice call CPU and latency measured on LAN.
- [ ] Remote desktop 1080p CPU and FPS measured on GNOME and KDE.
- [ ] Startup time measured for daemon, UI, and overlay.
- [ ] No busy loops; verify with `strace` or `perf` sampling while idle.
- [ ] Memory growth checked over a 24-hour soak test.
- [ ] Document tuning knobs and their defaults.

## Reliability

- [ ] Daemon survives UI and overlay crashes.
- [ ] Daemon restarts cleanly after system suspend/resume.
- [ ] Network change (Wi-Fi to Ethernet) triggers rediscovery and reconnect.
- [ ] Outbox and transfers resume after restart.
- [ ] Database integrity check passes after soak test.
- [ ] Logs rotate or are bounded.
- [ ] Crash handling produces a useful log without secrets.

## Security review

- [ ] Threat model reviewed end to end.
- [ ] Pairing and pinning reviewed.
- [ ] Key storage and permissions verified.
- [ ] Policy enforcement verified for accept, mute, ignore, and DND.
- [ ] Remote desktop consent, indicator, and kill switch verified.
- [ ] `cargo audit` clean or documented exceptions.
- [ ] License review for all dependencies.
- [ ] No secrets in logs, crash dumps, or Flatpak metadata.
- [ ] Flatpak permissions reviewed line by line.

## Documentation

- [ ] Install from Flatpak.
- [ ] Install from source with systemd user units.
- [ ] Enable and disable autostart for daemon, UI, and overlay.
- [ ] Pairing guide with PIN and QR.
- [ ] Troubleshooting: daemon not starting, no discovery, portal denial, audio
  device issues, tray missing.
- [ ] Performance and resource usage.
- [ ] Security model and limitations.
- [ ] Uninstall and data removal instructions.

## Release checklist

- [ ] Version bumped in workspace and Flatpak metadata.
- [ ] `CHANGELOG.md` updated.
- [ ] CI green on stable Rust.
- [ ] Flatpak build reproducible.
- [ ] Install, upgrade, and uninstall tested on a clean VM.
- [ ] Two-node smoke test: pairing, messaging, file, call, remote desktop.
- [ ] X11 and Wayland smoke tests.
- [ ] Soak test passed.
- [ ] Release notes written.

## Tests

- [ ] Full workspace test suite passes.
- [ ] Flatpak build test in CI or a documented manual job.
- [ ] Clean-VM install and uninstall test.
- [ ] Upgrade test from previous version keeps identity and history.
- [ ] Soak test with periodic messages and one call.
- [ ] Performance benchmarks recorded with hardware and environment noted.

## Security

- [ ] Final review signed off.
- [ ] No new permissions without documented justification.
- [ ] Unit hardening verified on the target distro.
- [ ] Dependency audit exceptions documented with expiry.

## Acceptance criteria

- Flatpak installs and all four processes run inside the sandbox.
- systemd user units start the daemon at login and pass the hardening review.
- Idle memory, CPU, and startup targets are met and documented.
- 24-hour soak test shows stable memory and no database errors.
- Install, upgrade, and uninstall work on a clean VM.
- Full two-node feature smoke test passes on X11 and Wayland.

## Demo

1. Install the Flatpak on a clean VM.
2. Pair two nodes and run the full smoke test.
3. Show `systemd-analyze security` and idle resource measurements.
4. Show benchmark results and soak-test summary.
5. Uninstall and confirm no leftover services or autostart entries.

## Risks and mitigations

- Risk: Flatpak sandbox blocks portals or tray. Mitigation: test early in the
  stage and document required permissions.
- Risk: performance regressions found late. Mitigation: measure at earlier
  stages and keep benchmark scripts in the repo.
- Risk: security review finds blocking issues. Mitigation: schedule the review
  before release freeze and keep Stages 5 and 11 reviewed incrementally.

## Works without later stages

This is the final stage. It adds packaging, hardening, measurement, and
documentation without changing the feature set built in Stages 1–11.
