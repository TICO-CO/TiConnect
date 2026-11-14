# TiConnect

LAN-only daemon for messaging, images, audio, voice calls, and remote desktop.
Linux first. Background-first: the daemon receives messages and calls even when
the UI was never opened.

[![CI](https://github.com/TICO-CO/TiConnect/actions/workflows/ci.yml/badge.svg)](https://github.com/TICO-CO/TiConnect/actions/workflows/ci.yml)

## Status

Stages 1–11 are implemented in automation. Manual systemd/logout, two-machine
LAN, X11/Wayland, notification, audio-device, portal, and accessibility checks
remain release gates. This includes workspace, settings, logging, typed errors,
the architecture graph, the D-Bus daemon and CLI, systemd/autostart packaging,
Ed25519 identity, SQLite storage, mDNS and broadcast discovery, SPAKE2 pairing,
pinned QUIC/TLS transport, signed messaging with sync/receipts/policy, the GTK4
window, overlay and notification fallback, resumable BLAKE3 attachments, Opus
voice calls with daemon ringtone and jitter buffer, and PIN-gated consent-based
remote desktop with portal capture, H.264, input injection, indicator, and kill
switch. Stage 12 adds Flatpak metadata, hardened units, benchmarks, and release
documentation.

See `main.md` for product intent, `architecture.md` for the design,
`docs/systems/` for component contracts and graphs, and `roadmap/roadmap.md` for
staged delivery.

## Workspace

| Crate | Purpose |
| --- | --- |
| `ticonnect-settings` | Persisted and effective settings, leaf crate |
| `ticonnect-logging` | `tracing-subscriber` setup, leaf crate |
| `ticonnect-core` | Shared domain types, identity, fingerprints, node IDs |
| `ticonnect-net` | Discovery and transport |
| `ticonnect-media` | Audio and video codecs |
| `ticonnect-desktop` | Capture and input integration |
| `ticonnect-daemon` | Daemon service library: D-Bus, identity, SQLite |
| `ticonnect-runtime` | Composition root and binary `ticonnectd` |
| `ticonnect-ui` | GTK4 main window, binary `ticonnect-ui` |
| `ticonnect-overlay` | Toast overlay, binary `ticonnect-ovl` |
| `ticonnect-cli` | Command-line client, binary `ticonnect-cli` |
| `xtask` | Repository architecture graph generator and validator |

Dependency direction is documented in `docs/dependency-direction.md`.
Component contracts and relations are documented in `docs/systems/`.

## Build and test

```sh
cargo build --workspace
cargo build --workspace --bins
cargo test --workspace --all-targets
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all -- --check
cargo audit
cargo xtask systems --check
make lan-test
```

`tests/local/run.sh` runs local X11, Weston Wayland, and KWin Wayland
integration tests. See `docs/local-testing.md`. `make lan-test` runs the
two-container LAN e2e suite; Docker Compose is required. Set `LAN_TEST_SCENARIO` to a comma-separated list of scenario names
(`control`, `discovery`, `pairing`, `transport`, `messaging`, `offline-sync`,
`security`) to run a subset. The GTK4 UI and overlay need development headers:

```sh
sudo apt install libgtk-4-dev libadwaita-1-dev libasound2-dev libwayland-dev wayland-protocols cmake
```

Ubuntu 24.04 does not package `libgtk4-layer-shell-dev`; Ubuntu 25.04+, Debian
13, and current Arch/Fedora provide it. On Ubuntu 24.04 build
`gtk4-layer-shell` 1.0.4 from source before compiling the overlay:

```sh
git clone --depth 1 --branch v1.0.4 https://github.com/wmww/gtk4-layer-shell.git
meson setup gtk4-layer-shell/build gtk4-layer-shell
sudo meson install -C gtk4-layer-shell/build
sudo ldconfig
```

Architecture docs are generated. After editing `docs/systems/graph/` or
`docs/systems/state-machines.toml`, run `cargo xtask systems`.

## Run binaries

```sh
cargo run --bin ticonnectd
cargo run --bin ticonnect-cli -- status
cargo run --bin ticonnect-cli -- status --json
cargo run --bin ticonnect-cli -- ping
cargo run --bin ticonnect-cli -- id show
cargo run --bin ticonnect-cli -- settings get dnd.enabled
cargo run --bin ticonnect-cli -- settings set dnd.enabled true
cargo run --bin ticonnect-cli -- settings list
cargo run --bin ticonnect-cli -- peers list
cargo run --bin ticonnect-cli -- discover list
cargo run --bin ticonnect-cli -- discover watch
cargo run --bin ticonnect-cli -- pair start
cargo run --bin ticonnect-cli -- pair with-pin <node-id> <pin>
cargo run --bin ticonnect-cli -- pair with-qr '<payload>'
cargo run --bin ticonnect-cli -- pair cancel
cargo run --bin ticonnect-cli -- pair forget <peer-id>
cargo run --bin ticonnect-cli -- send <peer-id> "hello"
cargo run --bin ticonnect-cli -- messages <peer-id>
cargo run --bin ticonnect-cli -- mark-read <peer-id> <message-id>
cargo run --bin ticonnect-cli -- policy set <peer-id> mute
cargo run --bin ticonnect-cli -- policy dnd on
cargo run --bin ticonnect-cli -- quit
cargo run --bin ticonnect-ui
cargo run --bin ticonnect-ovl
```

`ticonnectd` serves `org.ticonnect.Daemon` on the session bus until shutdown.
`ticonnect-cli` talks to the daemon for status, identity, peers, discovery,
pairing, messaging, policy, ping, and quit.
`settings get`, `settings set`, and `settings list` read and write `config.toml`
directly. `ticonnect-ui` opens the GTK4 window and talks to the daemon only over
D-Bus. `ticonnect-ovl` shows corner cards for incoming messages without depending
on the UI process.

## UI

`ticonnect-ui` is a separate process. It never reads keys, the database, or the
network directly. Closing the window stops only the UI; the daemon keeps
receiving messages. Appearance follows the system by default; the Settings view
can force light or dark through libadwaita's `AdwStyleManager`. `GTK_THEME` is
not supported by libadwaita and overrides its styling.

The UI owns `org.ticonnect.UI` on the session bus and exposes
`Activate(s view)`. Views are `peers`, `chat`, `settings`, and `pairing`; an
empty value means `chat`. The tray uses `Activate("chat")`. Starting a second UI
forwards activation to the running instance.

Keyboard shortcuts:

| Shortcut | Action |
|---|---|
| `Enter` | Send message |
| `Shift+Enter` | Insert a newline |
| `Ctrl+,` | Settings |
| `Ctrl+N` | Pair a device |
| `Ctrl+1` | Peers |
| `Ctrl+2` | Conversation |
| `Ctrl+Q` | Quit the UI |
| `Escape` | Close a dialog |

Settings are written directly to `config.toml` through `ticonnect-settings`; the
daemon reloads the file and emits `SettingsChanged`. Autostart is controlled by
the XDG entry at `$XDG_CONFIG_HOME/autostart/ticonnect.desktop`. The UI never
calls a `SetSetting` D-Bus method.

## Overlay

`ticonnect-ovl` is a separate always-running GTK4 process. It subscribes directly
to `org.ticonnect.Daemon`, resolves sender names through `ListPeers`, and does not
depend on the UI being open.

- Wayland: uses `gtk4-layer-shell` with `Layer::Overlay`, anchored bottom-right,
  and `KeyboardMode::None`. Cards never take keyboard focus; only the card
  surface accepts pointer input.
- Wayland without layer-shell support: falls back to system notifications.
- X11: uses an undecorated, always-on-top, skip-taskbar window positioned at the
  bottom-right of the first monitor. Window flags use EWMH through `x11rb`.
- Message cards show sender, preview, and local time. Up to three cards are
  visible; additional cards queue. Cards auto-dismiss after
  `overlay.timeout_secs`, pause while hovered, and deduplicate by message id.
- Clicking a card calls `org.ticonnect.UI.Activate("chat")`. If the UI is not
  running, the overlay starts `ticonnect-ui` from `PATH` and retries once.
- Mute, ignore, and DND are enforced by the daemon: suppressed messages arrive
  with `notify=false` and produce no card or notification.
- When `overlay.enabled` is false or no surface can initialize, the same events
  use `notify-rust` system notifications instead. The overlay never sends both.
- The process is single-instance through the GTK application id
  `org.ticonnect.Overlay`. Idle memory is accepted at roughly 40 MB.
- Autostart entry: `$XDG_CONFIG_HOME/autostart/ticonnect-overlay.desktop`.

## Configuration

- User config file: `$XDG_CONFIG_HOME/ticonnect/config.toml`.
- User data directory: `$XDG_DATA_HOME/ticonnect`.
- `TICONNECT_CONFIG_DIR` selects the config directory before settings load. It is
  bootstrap-only and is never persisted.
- `TICONNECT_*` environment variables override settings at runtime. They are not
  persisted. Nested keys use a double underscore delimiter, for example
  `TICONNECT_DND__ENABLED=true`.
- The discovered config directory is created with mode `0700`; `config.toml` is
  written with mode `0600`.
- Settings writes use an in-process mutex, an OS advisory lock, and atomic
  replacement. Saves persist explicit changes only. Concurrent writers are
  last-writer-wins per save.
- The daemon watches the config directory and reloads `config.toml` after
  external writes. Each changed setting is reported with `SettingsChanged`.

Settings keys:

| Key | Type | Default | Notes |
| --- | --- | --- | --- |
| `log_level` | string | `info` | `off`, `error`, `warn`, `info`, `debug`, `trace` |
| `log_file` | optional path | unset | Empty value clears it |
| `log_format` | string | `human` | `human` or `json` |
| `data_dir` | optional path | XDG data dir | Empty value clears it |
| `identity.display_name` | string | empty | Shown by `id show` and `GetIdentity` |
| `discovery.enabled` | bool | `true` | Enables discovery backends |
| `discovery.mdns` | bool | `true` | Advertise and browse `_ticonnect._udp.local.` |
| `discovery.broadcast` | bool | `true` | UDP broadcast fallback |
| `discovery.beacon_interval_secs` | integer | `5` | Valid range `1..=3600` |
| `transport.port` | integer | `45871` | UDP port for QUIC session and pairing; valid range `1..=65535`; restart required |
| `overlay.enabled` | bool | `true` | Show corner cards; false uses system notifications |
| `overlay.timeout_secs` | integer | `5` | Card auto-dismiss timeout; valid range `1..=60` |
| `read_receipts.enabled` | bool | `true` | Send delivered and read receipts |
| `outbox.ttl_days` | integer | `7` | Valid range `1..=365` |
| `attachments.max_size_mib` | integer | `2048` | Valid range `1..=102400` |
| `calls.timeout_secs` | integer | `30` | Ringing timeout; valid range `5..=300` |
| `calls.input_device` | string | empty | Optional input device name |
| `calls.output_device` | string | empty | Optional output device name |
| `remote.session_timeout_secs` | integer | `0` | 0 disables automatic timeout |
| `remote.pin_attempts` | integer | `3` | Valid range `1..=10` |
| `remote.pin_ttl_secs` | integer | `300` | Valid range `30..=3600` |
| `video.bitrate_kbps` | integer | `2500` | Valid range `256..=20000` |
| `video.fps` | integer | `30` | Valid range `5..=60` |
| `video.keyframe_interval` | integer | `60` | Valid range `1..=600` |
| `ringtone.path` | optional path | unset | Empty value clears it |
| `dnd.enabled` | bool | `false` | Global do-not-disturb |
| `appearance.color_scheme` | string | `system` | UI appearance: `system`, `light`, or `dark` |

## Identity

- Private key: `$XDG_CONFIG_HOME/ticonnect/identity.key`, exactly 32 raw bytes,
  mode `0600`.
- Identity metadata: `$XDG_CONFIG_HOME/ticonnect/identity.toml`, mode `0600`.
  It contains the UUIDv7 `node_id`; the public key is derived from the private
  key.
- The config directory is mode `0700`. A key with any group or other permission
  bit is rejected at startup.
- The fingerprint is BLAKE3 of the Ed25519 public key, rendered as lowercase hex
  with groups of four for display.
- Deleting `identity.key` creates a new key and a new node ID on the next start.
  If the key exists but `identity.toml` is missing, startup fails rather than
  silently changing the node ID.
- Key rotation and key backup/export are out of scope for v1. Losing the key
  makes this node untrusted to existing peers.

## Data storage

- Database: `$XDG_DATA_HOME/ticonnect/ticonnect.db`, mode `0600`; its directory is
  mode `0700`.
- SQLite uses WAL mode and foreign keys.
- Schema version is stored in `schema_version` and migrations run on every open.
  Migration failure refuses startup.
- Stage 3 creates `peers`, `conversations`, `messages`, `outbox`, `attachments`,
  and `peer_state`. Stage 6 adds `peers.public_key` and `messages.state`. Stage 9
  adds attachment names and message attachment metadata plus
  `transfer_partials` for resumable transfers.
- Configuration lives only in `config.toml`; SQLite stores no configuration.
- Changing `data_dir` while the daemon is running does not move an existing database;
  it applies on the next start.

## Messaging

- Each conversation is the pair of node IDs. Its conversation ID is BLAKE3 over
  the sorted node IDs, encoded as a UUID string.
- The signed canonical record uses domain `message-record` and contains the
  protocol version, message ID, conversation ID, sender ID, sequence, send
  timestamp, kind, target ID, body, and attachment hash. Optional fields use an
  explicit presence byte.
- A sender assigns a monotonic `seq` per conversation and signs the canonical
  record with its Ed25519 identity key using the shared signed-envelope format.
  The receiver checks the sender's stored public key, signature, conversation,
  and sequence before storage.
- Message bodies are limited to 32 KiB. Duplicate message IDs and duplicate
  `(conversation, sender, seq)` records are ignored.
- Text is the only user message kind in this stage. Ack records use kind `ack`,
  a target message ID, and body `delivered` or `read`.
- Undelivered records queue in the outbox. Retry uses exponential backoff after
  a failed stream write and expires after `outbox.ttl_days`. Expired rows are
  removed and the message state becomes `expired`. Expired and failed records
  stay in local history but are not served during gap sync.
- On every authenticated session each side exchanges `last_sent_seq` and
  `last_recv_seq`, requests missing sequence ranges in batches of 128, verifies
  and inserts the records, advances its receive high-water mark, and flushes its
  outbox. Synced records use the same verification path as live records.
- Delivered receipts are queued when an accepted message is stored. Read
  receipts are queued by `MarkRead`. Receipts are signed ack records, coalesced
  for 500 ms, and sent through the outbox. Mute suppresses delivered receipts;
  ignore suppresses all receipts.
- Message state is `queued`, `sent`, `delivered`, `read`, `failed`, or `expired`.
  Inbound messages have no local outbound state.
- `accept` stores and may notify; `mute` stores silently and suppresses
  delivered receipts; `ignore` drops inbound records before verification,
  storage, receipt creation, or notification. DND suppresses notifications but
  not storage.
- The tray is optional. When StatusNotifierItem support is unavailable the
  daemon continues headless.

D-Bus messaging methods are `SendMessage`, `ListMessages`, `MarkRead`,
`GetMessageState`, `SetPeerPolicy`, and `SetDnd`. Signals are `MessageReceived`,
`MessageStateChanged`, `SyncCompleted`, and `PolicyChanged`.

## Attachments

- Files are hashed with BLAKE3 and stored under
  `$XDG_DATA_HOME/ticonnect/attachments/<first2>/<hash>`; identical content
  dedupes across messages and peers.
- Messages carry name, size, MIME, and hash. Transfers use a dedicated QUIC
  bidirectional stream, 64 KiB chunks, persisted missing ranges, and a final
  hash check before publication.
- `ticonnect-cli send-file <peer> <path> [--mime TYPE]` sends an attachment.
- `ticonnect-cli download <hash> <destination>` saves it.
- `ticonnect-cli transfer status <hash>` and `transfer cancel <hash>` inspect or
  cancel a transfer.
- Inbound `ignore` peers are dropped before transfer; `mute` stores silently.

## Voice calls

- The daemon owns `idle`, `ringing`, `in-call`, and `ended` states and rings with
  the UI closed. Only one call is active per node.
- Audio uses 48 kHz mono Opus with 20 ms frames over QUIC datagrams, a bounded
  jitter buffer, packet-loss concealment, and a playback ring. Microphones open
  only after call acceptance.
- Ringtone path comes from `ringtone.path`; a bundled generated tone is the
  fallback. DND suppresses ringing but still reports the call.
- D-Bus: `StartCall`, `AcceptCall`, `DeclineCall`, `HangupCall`,
  `GetCallState`; signals `IncomingCall`, `CallStateChanged`, `CallEnded`.

## Remote desktop

- A viewer requests a session from a trusted online peer. The host shows the
  viewer fingerprint and a fresh PIN, then consents. The viewer enters the PIN.
- Sessions are scoped to one peer and require per-session consent. A visible
  indicator stays on while active, and `EndRemoteSession` is a hard kill switch.
- Capture uses the XDG RemoteDesktop portal and PipeWire fd; input uses libei
  when available and X11 XTEST otherwise. H.264 encodes at 1080p in software.
- D-Bus: `RequestRemoteSession`, `AcceptRemoteSession`, `DenyRemoteSession`,
  `EndRemoteSession`, `GetRemoteSession`; signals `RemoteSessionRequested`,
  `RemoteSessionStarted`, `RemoteSessionEnded`, `RemoteIndicatorChanged`.

## Pairing and transport

- Pairing uses SPAKE2 over the `ticonnect-pair/1` ALPN. Session transport uses
  `ticonnect/1` on the same UDP port.
- `ticonnect-cli pair start` prints a six-digit PIN, a QR payload, and the
  expiry time. PINs and QR tokens expire after 120 seconds. A pairing session
  allows three PIN failures.
- `ticonnect-cli pair with-pin <node-id> <pin>` uses a discovery candidate.
  `ticonnect-cli pair with-qr '<payload>'` uses the address and port in the QR
  payload and does not require discovery.
- The self-signed TLS certificate reuses the Ed25519 identity key. The identity
  fingerprint and certificate fingerprint are the same BLAKE3 value. Certificates
  are pinned to the trusted peer table.
- `ticonnect-cli pair forget <peer-id>` removes trust, deletes local history
  and queued outbox rows, and closes the active session. Sequence high-water
  marks are retained so re-pairing continues sequence numbers instead of
  reusing them. Trusted peers are reconnected automatically when rediscovered.
- Unpaired peers can reach only the pairing ALPN. Session ALPN connections are
  rejected before handler dispatch unless the certificate fingerprint is trusted.

## Discovery

- Discovery advertises this node over mDNS service type `_ticonnect._udp.local.`
  and browses for peers on the LAN.
- TXT keys are `v` (protocol version), `id` (node id), `fp` (fingerprint),
  `name` (display name), and `port` (configured QUIC transport port).
- UDP broadcast is the IPv4 fallback. Beacons use port `45870` and are sent to
  each interface broadcast address every `discovery.beacon_interval_secs`.
- Candidates expire after three beacon intervals. Stopping discovery or losing a
  candidate emits `CandidateLost`.
- Discovery data is untrusted input. Candidates are hints only and never imply
  pairing, trust, or authentication. Candidate caps, packet size limits, field
  validation, self-filtering, and protocol version checks are always enforced.
- A malformed packet is dropped and counted. mDNS failure does not prevent the
  broadcast backend from running, and disabling mDNS leaves broadcast fallback.
- Firewall expectations: allow UDP port `5353` for mDNS and UDP port `45870` for
  broadcast discovery on the local network. Discovery never opens a transport
  connection.
- `ticonnect-cli discover list` prints current candidates. `discover watch`
  prints `found` and `lost` events until interrupted.

## Documentation

User-facing documentation lives in this file and `docs/`. Rustdoc comments are
not used in this repository. See `docs/install.md`, `docs/performance.md`, `docs/remote-desktop.md`, and
`docs/release-checklist.md` for release operations.
