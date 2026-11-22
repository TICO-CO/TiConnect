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
Ed25519 identity, SQLite storage, mDNS and broadcast discovery, automatic LAN trust,
pinned QUIC/TLS transport, signed messaging with sync/receipts/policy, the GTK4
window, overlay and notification fallback, resumable BLAKE3 attachments, Opus
voice calls with jitter buffer, and consent-based remote
desktop with portal capture, H.264, input injection, indicator, and kill
switch. Stage 12 adds Flatpak metadata, hardened units, benchmarks, release
documentation, the AppImage user installer, and the unified launcher.

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
| `ticonnect-runtime` | Composition root and daemon entry point |
| `ticonnect-ui` | GTK4 main window |
| `ticonnect-overlay` | Toast overlay |
| `ticonnect-cli` | Command-line client |
| `ticonnect` | Launcher and CLI dispatcher; hidden internal `daemon`, `ui`, `overlay` entries |
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

`tests/local/run.sh` runs local X11, Weston Wayland, KWin Wayland, and Hyprland
integration tests. See `docs/local-testing.md`. `make lan-test` runs the
two-container LAN e2e suite; Docker Compose is required. Set `LAN_TEST_SCENARIO` to a comma-separated list of scenario names
(`control`, `discovery`, `trust`, `transport`, `messaging`, `offline-sync`,
`security`) to run a subset. The GTK4 UI and overlay need development headers:

```sh
sudo apt install libgtk-4-dev libwebkitgtk-6.0-dev libasound2-dev libwayland-dev wayland-protocols cmake
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

## Install

### AppImage

Download `ticonnect-<tag>-x86_64.AppImage`, then install the per-user app
integration:

```sh
chmod +x ticonnect-v0.1.0-x86_64.AppImage
./ticonnect-v0.1.0-x86_64.AppImage --install-user
```

Launch TiConnect from the app menu. `--uninstall-user` removes the app
integration and keeps your configuration, identity, database, attachments, and
logs. Use `--uninstall-user --purge` to remove those too.

### Arch

Install the package from the latest release with `pacman -U`. The package adds
the app-menu entry, icon, systemd user units, and autostart entries.

### Nix

```sh
nix profile install github:TICO-CO/TiConnect#ticonnect
```

The NixOS module is available at `inputs.ticonnect.nixosModules.default`.

## Use

Launch TiConnect from your app menu. Open LAN mode is on by default: discovered
devices on the same LAN connect automatically without a PIN. Send messages,
files, and audio attachments, start calls, or request a remote desktop session.
Remote desktop still requires explicit host consent. TiConnect traffic
stays on your LAN and is encrypted with pinned TLS certificates.

Use user policies to mute, ignore, or block a device. Blocked devices are kept
in a blocklist and cannot reconnect automatically. Open LAN mode can be disabled
with the user blocklist (`ticonnect users forget <user-id>`).

## Command line

The same executable is the command-line client:

```sh
ticonnect status
ticonnect version
ticonnect id show
ticonnect users list
ticonnect send <user-id> "hello"
ticonnect send-file <user-id> <path>
ticonnect download <hash> <destination>
ticonnect transfer status <hash>
ticonnect messages <user-id>
ticonnect settings list
```

`ticonnect --help` lists the user-facing surface. `ticonnect --version` prints
the binary version; `ticonnect version` is an alias. `ticonnect cli ...` is a
hidden compatibility alias, and `ticonnect cli version` reports the daemon
version.

## Development / internals

These entries run individual processes and are hidden from user help:

```sh
ticonnect daemon
ticonnect overlay
ticonnect ui
ticonnect cli status
```

`ticonnect daemon` serves `org.ticonnect.Daemon` on the TiConnect IPC bus until
shutdown. `ticonnect <command>` talks to the daemon for status, identity, users,
discovery, trust, messaging, policy, ping, and quit. The hidden
`ticonnect cli <command>` alias reaches the same client. `settings get`,
`settings set`, and `settings list` read and write `config.toml` directly.
`ticonnect ui` opens the GTK4 window and talks to the daemon only over D-Bus.
`ticonnect overlay` shows corner cards for incoming messages without depending
on the UI process. Running `ticonnect` with no subcommand opens the UI and
starts the daemon when it is absent.

## Release

Every push to `main` compiles and tests the workspace. Merging a `v*` branch
(for example `v0.1.0`) into `main` creates the matching tag automatically, then
builds and publishes `ticonnect-<tag>-x86_64.AppImage`, the Arch package, and
`SHA256SUMS.txt`. The tag must match the `version` in `Cargo.toml`. The binary
tarball is an internal Arch build input and is not published. See
`docs/install.md` for AppImage, `pacman -U`, and Nix instructions.

```sh
git switch -c v0.1.0
# bump Cargo.toml version, commit, push, open PR, merge into main
```

Pushing a tag manually still works:

```sh
git tag v0.1.0
git push origin v0.1.0
```

## UI

The UI runs as the internal `ticonnect ui` process. It never reads keys, the
database, or the network directly. Closing the window stops only the UI; the daemon keeps
receiving messages. The window hosts a WebKitGTK 6.0 webview; the Svelte frontend
talks to Rust through a loopback HTTP + WebSocket bridge. The frontend source lives
in `crates/ui/web`; the committed Vite build in `crates/ui/assets` is embedded in
the binary.

The UI owns `org.ticonnect.UI` on the TiConnect IPC bus and exposes
`Activate(s view)`. Views are `chat`, `users`, `settings`, `remote`, and
`watch-party`; an empty value means `chat`. `Activate("chat:<node-id>")` opens
one conversation; the tray and overlay use it for notification clicks. The tray
uses `Activate("chat")`. Starting a second UI forwards activation to the running
instance.

Frontend styling lives in `crates/ui/web`. Build the bundle with `make
web-build`. Interactive `make lab` starts Vite automatically and hot-reloads
frontend edits; set `LAB_HOT_RELOAD=0` to use the embedded bundle instead. Set
`TICONNECT_DEV_URL=http://127.0.0.1:5173` manually to load the Vite dev server
outside the lab; this disables bridge token checks and must not be used on a
shared machine.

`make lab` routes call audio through null devices by default because both nodes
share the host microphone and speakers. For real audio, use headphones and run
`call-audio hardware` in both lab panes, or start with
`LAB_CALL_AUDIO=hardware make lab`.

Keyboard shortcuts:

| Shortcut | Action |
|---|---|
| `Enter` | Send message |
| `Shift+Enter` | Insert a newline |
| `Ctrl+,` | Settings |
| `Ctrl+N` | Open users |
| `Ctrl+1` | Users |
| `Ctrl+2` | Conversation |
| `Ctrl+M` | Mute or unmute the call microphone (configurable) |
| `Ctrl+Shift+M` | Mute or unmute call output (configurable) |
| `Ctrl+Q` | Quit the UI |
| `Escape` | Close a dialog |

Settings are written directly to `config.toml` through `ticonnect-settings`; the
daemon reloads the file and emits `SettingsChanged`. The UI exposes editable
discovery controls and a system/light/dark appearance selector, and can clear
all local message history or one conversation after confirmation. Autostart is
controlled by the XDG entry at `$XDG_CONFIG_HOME/autostart/ticonnect.desktop`.
The UI never calls a `SetSetting` D-Bus method.

## Overlay

The overlay runs as the internal `ticonnect overlay` process. It subscribes
directly to `org.ticonnect.Daemon`, resolves sender names through `ListUsers`,
and does not depend on the UI being open.

- Wayland: uses `gtk4-layer-shell` with `Layer::Overlay` and
  `KeyboardMode::None`. Request and active-session cards anchor top-right;
  message toasts anchor bottom-right above the YouTube player.
- Wayland without layer-shell support: falls back to system notifications.
- X11: uses two undecorated, always-on-top, skip-taskbar windows positioned at
  the top-right and bottom-right of the first monitor. Window flags use EWMH
  through `x11rb`.
- Request cards cover incoming calls, remote consent, watch-party invites, and
  the active call/sharing/party states. Message toasts show sender, preview, and
  local time.
- Up to three cards are visible per surface; additional cards queue. Cards
  auto-dismiss after `overlay.timeout_secs`, pause while hovered, and deduplicate
  by message id.
- Clicking a message toast calls `org.ticonnect.UI.Activate("chat")`. If the UI
  is not running, the overlay starts the current executable with `ui` and retries
  once. Party cards open the `watch-party` view.
- Remote consent is a single host acceptance; no PIN is generated, shown, or
  entered.
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
- `TICONNECT_IPC_BUS_ADDRESS` selects the D-Bus address used for daemon, UI, CLI,
  and overlay IPC. It defaults to the session bus. GTK, WebKit, portals, tray,
  and notifications still use `DBUS_SESSION_BUS_ADDRESS`.
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
| `transport.port` | integer | `45871` | UDP port for QUIC session and trust; valid range `1..=65535`; restart required |
| `overlay.enabled` | bool | `true` | Show corner cards; false uses system notifications |
| `overlay.timeout_secs` | integer | `5` | Card auto-dismiss timeout; valid range `1..=60` |
| `read_receipts.enabled` | bool | `true` | Send delivered and read receipts |
| `typing.enabled` | bool | `true` | Send and show typing indicators |
| `outbox.ttl_days` | integer | `7` | Valid range `1..=365` |
| `attachments.max_size_mib` | integer | `2048` | Valid range `1..=102400` |
| `calls.timeout_secs` | integer | `30` | Ringing timeout; valid range `5..=300` |
| `calls.input_device` | string | empty | Optional input device name |
| `calls.output_device` | string | empty | Optional output device name |
| `calls.mute_shortcut` | string | `Ctrl+M` | Microphone mute shortcut; empty disables |
| `calls.output_mute_shortcut` | string | `Ctrl+Shift+M` | Output mute shortcut; empty disables |
| `remote.session_timeout_secs` | integer | `0` | 0 disables automatic timeout |
| `remote.request_timeout_secs` | integer | `300` | Consent request expiry; valid range `30..=3600` |
| `video.bitrate_kbps` | integer | `2500` | Valid range `256..=20000` |
| `video.fps` | integer | `30` | Valid range `5..=60` |
| `video.keyframe_interval` | integer | `60` | Valid range `1..=600` |
| `dnd.enabled` | bool | `false` | Global do-not-disturb |
| `appearance.color_scheme` | string | `system` | UI appearance: `system`, `light`, or `dark` |

## Identity

- Private key: `$XDG_CONFIG_HOME/ticonnect/identity.key`, exactly 32 raw bytes,
  mode `0600`.
- Identity metadata: `$XDG_CONFIG_HOME/ticonnect/identity.toml`, mode `0600`.
  It contains the UUIDv7 `node_id`; the public key is derived from the private
  key.
- Profile photo: `$XDG_DATA_HOME/ticonnect/profile/self.png`, mode `0600`.
  The UI center-crops and resizes the chosen image to a 192x192 PNG before
  handing it to the daemon. The daemon advertises the BLAKE3 hash over mDNS.
- Users discover the profile over the `ticonnect-profile/1` ALPN. The profile
  payload is signed with the identity key and verified against the pinned TLS
  certificate and advertised avatar hash. Profile data is public LAN data and
  never implies trust.
- Cached user profiles live in the `user_profiles` table; `GetAvatar(fingerprint)`
  returns the cached PNG or the local photo for the local fingerprint.
- The config directory is mode `0700`. A key with any group or other permission
  bit is rejected at startup.
- The fingerprint is BLAKE3 of the Ed25519 public key, rendered as lowercase hex
  with groups of four for display.
- Deleting `identity.key` creates a new key and a new node ID on the next start.
  If the key exists but `identity.toml` is missing, startup fails rather than
  silently changing the node ID.
- Key rotation and key backup/export are out of scope for v1. Losing the key
  makes this node untrusted to existing users.

## Data storage

- Database: `$XDG_DATA_HOME/ticonnect/ticonnect.db`, mode `0600`; its directory is
  mode `0700`.
- SQLite uses WAL mode and foreign keys.
- Schema version is stored in `schema_version` and migrations run on every open.
  Migration failure refuses startup.
- Stage 3 creates `users`, `conversations`, `messages`, `outbox`, `attachments`,
  and `user_state`. Stage 6 adds `users.public_key` and `messages.state`. Stage 9
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
- `MarkRead` always records local read state in `messages.read_at`, even when
  read receipts are disabled. `MarkConversationRead` marks every unread inbound
  message in one conversation. `ListConversationSummaries` returns the last
  message, unread count, and conversation id for sidebar previews.
- Outgoing message state is rendered as clock, single check, double check, and
  blue double check for queued, sent, delivered, and read.
- Typing indicators use a bounded `typing` wire frame over a short
  unidirectional stream. They are not stored, respect `typing.enabled`, and are
  cleared when the composer is idle, loses focus, or sends.
- `accept` stores and may notify; `mute` stores silently and suppresses
  delivered receipts; `ignore` drops inbound records before verification,
  storage, receipt creation, or notification. DND suppresses notifications but
  not storage.
- The daemon plays the bundled `gotmail.mp3` chime for inbound notifiable
  messages, so it rings with the UI closed. Mute, ignore, and DND silence it.
- The tray is optional. When StatusNotifierItem support is unavailable the
  daemon continues headless.

D-Bus messaging methods are `SendMessage`, `ListMessages`, `MarkRead`,
`MarkConversationRead`, `ClearHistory`, `GetMessageState`, `GetStorageUsage`,
`SetUserPolicy`, and `SetDnd`. Signals are `MessageReceived`,
`MessageStateChanged`, `SyncCompleted`, and `PolicyChanged`. `ClearHistory` with
an empty user id clears all local messages and unreferenced attachments; the UI
confirms before calling it.

## Attachments

- Files are hashed with BLAKE3 and stored under
  `$XDG_DATA_HOME/ticonnect/attachments/<first2>/<hash>`; identical content
  dedupes across messages and users.
- Messages carry name, size, MIME, and hash. Transfers use a dedicated QUIC
  bidirectional stream, 64 KiB chunks, persisted missing ranges, and a final
  hash check before publication.
- The web UI can record a voice note with the microphone. Voice notes upload as
  `audio/webm` attachments with an `audio/ogg` fallback and play inline; other
  audio files play the same way.
- `ticonnect send-file <user> <path> [--mime TYPE]` sends an attachment.
- `ticonnect download <hash> <destination>` saves it.
- `ticonnect transfer status <hash>` and `ticonnect transfer cancel <hash>`
  inspect or cancel a transfer.
- Inbound `ignore` users are dropped before transfer; `mute` stores silently.

## Voice calls

- The daemon owns `idle`, `ringing`, `in-call`, and `ended` states and receives
  calls with the UI closed. Only one call is active per node.
- Audio uses 48 kHz mono Opus with 20 ms frames over QUIC datagrams, a bounded
  jitter buffer, packet-loss concealment, and a playback ring. Microphones open
  only after call acceptance.
- Incoming calls are shown in the UI and overlay without an audio ringtone.
  DND suppresses notification sounds but still reports the call.
- Microphone and output mute are daemon-owned; the UI and overlay reflect the
  same state. Mute shortcuts are configurable with `calls.mute_shortcut` and
  `calls.output_mute_shortcut`.
- Input and output devices are selected from devices discovered by the daemon.
  An unavailable configured device fails the call with an error instead of
  silently switching devices.
- Call history is stored in SQLite and survives restarts.
- D-Bus: `StartCall`, `AcceptCall`, `DeclineCall`, `HangupCall`,
  `SetCallMuted`, `GetCallState`, `ListCallLogs`, `ListAudioDevices`; signals
  `IncomingCall`, `CallStateChanged`, `CallMuteChanged`, `CallEnded`.

## Remote desktop

- A viewer requests a session from a trusted online user. The host reviews the
  request and consents. Consent is the full grant; no PIN is shown or entered.
- Sessions are scoped to one user and require per-session consent. A visible
  indicator stays on while active, and `EndRemoteSession` is a hard kill switch.
- Capture uses the XDG RemoteDesktop portal and PipeWire fd; input uses libei
  when available and X11 XTEST otherwise. H.264 encodes at 1080p in software.
- D-Bus: `RequestRemoteSession`, `AcceptRemoteSession`, `DenyRemoteSession`,
  `EndRemoteSession`, `GetRemoteSession`; signals `RemoteSessionRequested`,
  `RemoteSessionStarted`, `RemoteSessionEnded`, `RemoteIndicatorChanged`.

## Automatic trust and transport

- Automatic trust uses SPAKE2 over the `ticonnect-trust/1` ALPN. Session transport
  uses `ticonnect/1` on the same UDP port.
- Discovery candidates are trusted automatically with a LAN token derived from
  both fingerprints. There is no manual pairing PIN or QR flow.
- The self-signed TLS certificate reuses the Ed25519 identity key. The identity
  fingerprint and certificate fingerprint are the same BLAKE3 value. Certificates
  are pinned to the user table.
- `ticonnect users forget <user-id>` removes trust, deletes local history
  and queued outbox rows, and closes the active session. Sequence high-water
  marks are retained so re-trust continues sequence numbers instead of
  reusing them. Trusted users are reconnected automatically when rediscovered.
- Blocked users are rejected before the trust ALPN exchange.

## Discovery

- Discovery advertises this node over mDNS service type `_ticonnect._udp.local.`
  and browses for users on the LAN.
- TXT keys are `v` (protocol version), `id` (node id), `fp` (fingerprint),
  `name` (display name), and `port` (configured QUIC transport port).
- UDP broadcast is the IPv4 fallback. Beacons use port `45870` and are sent to
  each interface broadcast address every `discovery.beacon_interval_secs`.
- Candidates expire after three beacon intervals. Stopping discovery or losing a
  candidate emits `CandidateLost`.
- Discovery data is untrusted input. Candidates are hints only and never imply
  trust, trust, or authentication. Candidate caps, packet size limits, field
  validation, self-filtering, and protocol version checks are always enforced.
- A malformed packet is dropped and counted. mDNS failure does not prevent the
  broadcast backend from running, and disabling mDNS leaves broadcast fallback.
- Firewall expectations: allow UDP port `5353` for mDNS and UDP port `45870` for
  broadcast discovery on the local network. Discovery never opens a transport
  connection.
- `ticonnect discover list` prints current candidates. `ticonnect discover watch`
  prints `found` and `lost` events until interrupted.

## Documentation

User-facing documentation lives in this file and `docs/`. Rustdoc comments are
not used in this repository. See `docs/install.md`, `docs/performance.md`, `docs/remote-desktop.md`, and
`docs/release-checklist.md` for release operations.
