# TiConnect Roadmap

LAN-only daemon for messaging, images, audio, voice calls, and remote desktop.
Linux first. Background-first: it receives messages and calls even when the UI
was never opened.

Source documents:

- `main.md` — product intent and constraints.
- `architecture.md` — process model, stack, protocol, security, data model.

## Independence rule

Every stage is a shippable, testable increment. A stage may depend on earlier
stages, but never on a later stage. At the end of each stage the repository must
build, the test suite must pass, and the stage demo must work with all later
stages absent.

If a stage cannot be demoed without future work, it is scoped too large and must
be split.

## Guiding principles

- Daemon-first. The UI is optional at runtime.
- One transport: QUIC for messages, files, audio, and video. No WebRTC.
- Pure-Rust media. Software H.264. No GStreamer.
- No root. Portals for capture and input injection.
- Nothing is reachable before pairing. Certificates are pinned.
- Secrets live in the user config directory with restrictive permissions.
- Lightweight while idle. The daemon stays small and headless.
- X11 and Wayland are both first-class.

## Stage overview

| # | Stage | Outcome | Depends on | Main risk |
|---|---|---|---|---|
| 1 | Foundation | Workspace builds, config/logging ready, typed errors, CI green | — | Scope creep into features |
| 1.5 | Architecture contracts & graph | Component registry, canonical relations, generated graphs, architecture CI | 1 | Graph drift |
| 2 | Daemon & IPC | Headless daemon on D-Bus, CLI, systemd unit, autostart | 1, 1.5 | D-Bus API churn |
| 3 | Identity & storage | Ed25519 identity, SQLite data schema, TOML settings, peers | 1, 1.5, 2 | Key handling mistakes |
| 4 | Discovery | mDNS + UDP fallback candidate discovery | 1, 1.5, 3 | mDNS availability on LANs |
| 5 | Pairing & transport | SPAKE2 pairing, QUIC/TLS with pinning, trust table | 1, 1.5, 3, 4 | Cryptographic correctness |
| 6 | Messaging | Signed replica messages, outbox, gap sync, receipts, policy, tray | 1, 1.5, 3, 5 | Ordering and expiry edge cases |
| 7 | GTK4 UI | Main window: peers, threads, settings, pairing dialog | 1, 1.5, 2, 6 | UI/daemon coupling |
| 8 | Overlay & notifications | Message cards, X11 fallback, activation, system notifications | 1, 1.5, 2, 6 | layer-shell portability |
| 9 | Attachments | Content-addressed store, resumable transfer, media rendering | 1, 1.5, 3, 5, 6, 7 | Large-file and resume state |
| 10 | Voice calls | Call state machine, ringtone, Opus over datagrams, jitter buffer | 1, 1.5, 3, 5, 6 | Audio latency and device quirks |
| 11 | Remote desktop | Portal capture, H.264, input injection, consent and kill switch | 1, 1.5, 3, 5, 7 | Portal/desktop variance |
| 12 | Packaging & performance | Flatpak, hardened units, idle/perf pass, security review | 1–11 | Release polish time |

Future extraction: binaries currently duplicate the `ticonnect-settings` +
`ticonnect-logging` bootstrap in their entrypoints. `ticonnect-runtime` will
own that composition, process lifecycle, and daemon wiring, so binaries depend
on `runtime` instead. This keeps `ticonnect-core` free of settings and logging
dependencies. Errors stay crate-local; `core` never depends on `settings` or
`logging`, and no crate imports a catch-all workspace error enum.

## Dependency graph

```
1 Foundation
└── 1.5 Architecture contracts & graph
    └── 2 Daemon & IPC
        ├── 3 Identity & storage
        │   ├── 4 Discovery
        │   │   └── 5 Pairing & transport
        │   │       ├── 6 Messaging
        │   │       │   ├── 7 GTK4 UI
        │   │       │   │   ├── 8 Overlay & notifications (message cards)
        │   │       │   │   └── 11 Remote desktop
        │   │       │   ├── 9 Attachments
        │   │       │   └── 10 Voice calls
        │   │       └── (5 feeds 6, 9, 10, 11)
        │   └── (3 feeds 4, 5, 6, 9, 10, 11)
        └── (2 feeds 7, 8)
12 Packaging & performance (after all)
```

Call and transfer cards extend the overlay in Stages 10 and 9 respectively.
Stage 8 ships message cards only so it remains demoable with earlier stages.


## Definition of Done (every stage)

- [ ] `cargo build --workspace` succeeds with no warnings.
- [ ] Components touched by the stage exist in `docs/systems/graph/` with an
  owner, stage, contract, test seam, limits, and failure mode.
- [ ] Cross-system edges are declared and the generated component graph is
  regenerated.
- [ ] `cargo xtask systems --check` passes; generated architecture docs are not
  stale.
- [ ] State ownership and trust/policy/consent gates are unchanged or explicitly
  updated.
- [ ] `cargo test --workspace` passes; new code has unit or integration tests.
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` is clean.
- [ ] `cargo fmt --all -- --check` is clean.
- [ ] User-facing behavior, configuration, and D-Bus interfaces are documented
  in `README.md` and `docs/`. Rustdoc comments are not used.
- [ ] New config keys are documented with defaults.
- [ ] New errors are typed and logged through `tracing`.
- [ ] Library crates return typed `thiserror` errors; only binary entry points
  use `anyhow` for top-level error reporting.
- [ ] Stage demo steps run on both X11 and Wayland where applicable.
- [ ] No secret, key, or credential is logged or committed.
- [ ] Stage file checkboxes are fully checked.

## Milestones

| Milestone | Stages | Meaning |
|---|---|---|
| M1 Headless messaging | 1–6 | Two paired nodes exchange encrypted text with history and receipts, no GUI. |
| M2 Usable desktop app | 7–8 | Main window and corner toasts work; daemon rings and notifies with UI closed. |
| M3 Media | 9–10 | Files, images, audio clips, and voice calls work. |
| M4 Remote desktop | 11 | View and control with consent, indicator, and kill switch. |
| M5 Release | 12 | Flatpak, hardened units, performance targets met. |

## Cross-cutting tracks

### Architecture

- [x] Canonical system, component, store, relation, and invariant graph exists.
- [x] Generated system, component, build, runtime, trust, state, and state-machine
  views exist.
- [x] `cargo xtask systems --check` runs in CI.
- [ ] Every new component and cross-system edge is declared before code lands.
- [ ] Contract changes are recorded with an ADR when they cross a stage boundary.

### Security

- [ ] Threat model documented before Stage 5 and updated at Stages 6, 10, 11.
- [ ] Pairing, pinning, and trust-table behavior covered by tests.
- [ ] Key files created with mode `0600`; directory `0700`.
- [ ] No feature reachable before pairing.
- [ ] Remote desktop consent, indicator, and kill switch reviewed.
- [x] Dependency audit (`cargo audit`) runs in CI from Stage 1.

### Testing

- [ ] Unit tests for protocol types, signing, and state machines.
- [ ] Integration tests for daemon D-Bus methods and signals.
- [ ] Two-node loopback tests for transport, messaging, and calls.
- [ ] Manual X11 and Wayland checklists per stage.
- [ ] Fuzz or property tests for wire decoding from Stage 5.

### Observability

- [ ] Structured `tracing` spans across D-Bus, transport, and media.
- [ ] Log levels configurable; default quiet while idle.
- [ ] `ticonnect-cli status` reports daemon health, peers, and queue depth.
- [ ] No message bodies or keys in logs by default.

### Performance

- [ ] Idle daemon memory target: under 30 MB.
- [ ] Overlay idle memory target: about 40 MB, accepted.
- [ ] Idle CPU under 1% on a modern laptop.
- [ ] 1080p remote desktop target; 4K explicitly out of scope.
- [ ] Measure at Stages 2, 6, 8, 10, 11, and 12.

## Stage index

1. [Foundation](stages/01-foundation.md)
1.5. [Architecture contracts & graph](stages/015-architecture-graph.md)
2. [Daemon & IPC](stages/02-daemon-ipc.md)
3. [Identity & storage](stages/03-identity-storage.md)
4. [Discovery](stages/04-discovery.md)
5. [Pairing & transport](stages/05-pairing-transport.md)
6. [Messaging](stages/06-messaging.md)
7. [GTK4 UI](stages/07-gtk4-ui.md)
8. [Overlay & notifications](stages/08-overlay-notifications.md)
9. [Attachments](stages/09-attachments.md)
10. [Voice calls](stages/10-voice-calls.md)
11. [Remote desktop](stages/11-remote-desktop.md)
12. [Packaging & performance](stages/12-packaging-performance.md)

## Overall progress

- [x] Stage 1 — Foundation
- [x] Stage 1.5 — Architecture contracts & graph
- [ ] Stage 2 — Daemon & IPC
- [ ] Stage 3 — Identity & storage
- [ ] Stage 4 — Discovery
- [ ] Stage 5 — Pairing & transport
- [ ] Stage 6 — Messaging
- [ ] Stage 7 — GTK4 UI
- [ ] Stage 8 — Overlay & notifications
- [ ] Stage 9 — Attachments
- [ ] Stage 10 — Voice calls
- [ ] Stage 11 — Remote desktop
- [ ] Stage 12 — Packaging & performance
