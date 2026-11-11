# TiConnect Architecture

LAN-only daemon for messaging, images, audio, voice calls, and remote desktop.
Background-first: it must receive messages and calls even when the UI was never
opened. Linux only for now; Windows is out of scope.

Constraints:

- Linux, X11 and Wayland.
- Lightweight while idle.
- Mute/ignore incoming messages per peer.
- No root privileges.

## 1. Process model

```
ticonnectd     headless daemon: discovery, transport, storage, calls,
               capture, ringtone
ticonnect-ui   GTK4 application: main window
ticonnect-ovl  corner overlay agent: toast cards
ticonnect-cli  debugging and scripting
```

The daemon is a **systemd user unit** plus an XDG autostart entry. It never runs
as root. The UI is a separate process so the daemon stays small and can run
without any graphical stack.

D-Bus is the control plane:

- `org.ticonnect.Daemon` — owned by `ticonnectd`. Methods for sending messages,
  managing peers, starting calls, granting remote sessions, and reading settings.
  Config writes go through `ticonnect-settings`, not D-Bus. Signals for inbound
  events and config reloads.
- `org.ticonnect.UI` — owned by `ticonnect-ui`. The overlay activates it by
  calling `Activate(view)`, which raises the window on the relevant screen.
- The overlay subscribes to daemon signals directly and does not depend on the
  UI being open.

The overlay agent is intentionally always running. Its idle memory cost of
roughly 40MB is accepted in exchange for custom corner toasts. If disabled,
events fall back to system notifications.

## 2. Workspace layout

```
Cargo.toml                  workspace
crates/
  settings/                 persisted + effective settings (leaf)
  logging/                  tracing setup (leaf)
  runtime/                  composition root, settings + logging + ticonnectd
  core/                     protocol types, identity, pairing, crypto
  net/                      mDNS discovery, QUIC transport, sessions
  media/                    cpal audio, opus, openh264, jitter buffer
  desktop/                  portals, screencast, libei/XTEST input
  daemon/                   D-Bus service, identity, SQLite (library)
  ui/                       GTK4 main window
  overlay/                  layer-shell toast agent
  cli/                      command-line client
```

Component contracts, state ownership, and relation graphs live in
`docs/systems/`. The graph is canonical and generated views are checked in CI.

Dependency direction: `settings` and `logging` are leaf crates. `logging` is a
pure tracing wrapper and depends on neither `settings` nor `core`. `core`
depends on neither. `net`, `media`, `desktop` depend on `core`. `daemon`
composes them into the D-Bus service library. `runtime` is the composition
root: it owns settings/logging bootstrap, identity and database composition,
supervision, and the `ticonnectd` binary. `ui`, `overlay`, and `cli` use runtime
bootstrap and never import daemon internals directly. `core` stays free of
settings and logging.

Each fallible library crate owns its `Error` and `Result` types and maps
failures from its dependencies into them. `core` exposes domain types only and
defines no crate-wide error type; fallible `core` modules define local errors
when they need them. There is no workspace-wide catch-all error enum.

Binary entry points (`ticonnectd`, `ticonnect-cli`, `ticonnect-ui`,
`ticonnect-ovl`) use `anyhow::Result` to report the full error chain once from
`main` and exit failure. They add context around each startup step and let
`anyhow` aggregate crate errors. Only `src/main.rs` uses `anyhow`; library
modules keep typed errors.

## 3. Stack

| Concern | Choice |
|---|---|
| Async runtime | `tokio` |
| Discovery | `mdns-sd`, UDP broadcast fallback |
| Transport | `quinn` over `rustls` (TLS 1.3, self-signed pinned certs) |
| Wire format | `postcard` + `serde` |
| Identity | `ed25519-dalek`, `blake3` fingerprints |
| Pairing | `spake2` with a 6-digit PIN or QR |
| IPC | `zbus` on the session bus |
| Data storage | `rusqlite` (bundled) |
| Audio I/O | `cpal` |
| Audio codec | `opus`, 20ms frames |
| Video codec | `openh264` (software), 1080p target |
| Screen capture | `ashpd` RemoteDesktop portal + `pipewire-rs` |
| Input injection | `reis` (libei) on Wayland, `x11rb` XTEST on X11 |
| Ringtone | `rodio` + `symphonia` decoders |
| UI | `gtk4`, `libadwaita`, `relm4` |
| Toast overlay | `gtk4-layer-shell` |
| Notifications | `notify-rust` |
| Tray | `ksni` (StatusNotifierItem) |
| Config | `figment` + TOML, `directories` (`ticonnect-settings`) |
| Logging | `tracing` (`ticonnect-logging`) |

### Config and data rule

Configuration lives only in `config.toml` under the user config directory. Any
process may read or write it through `ticonnect-settings`. Writes take an
in-process mutex and an OS advisory lock, then replace the file atomically.
Concurrent writers are last-writer-wins per save; the lock prevents corruption,
not lost updates. Environment overrides are runtime-only and never persisted.

The daemon watches `config.toml`, reloads changed settings, and emits
`SettingsChanged`. SQLite under the user data directory holds application data
only: peers, messages, outbox, attachment metadata, and related state. It never
stores configuration.

### Transport rule

**QUIC for everything; no WebRTC.** One transport carries messages, files,
audio, and video. Reliable streams handle messages and file transfer;
unreliable QUIC datagrams (RFC 9221) carry audio frames and remote input.
This avoids the maturity problems of `webrtc-rs` and keeps a single NAT-free
LAN path.

### Media rule

Pure-Rust media stack. Software H.264. No GStreamer. Target is 1080p on a
LAN; 4K support is explicitly out of scope.

## 4. Discovery, identity, pairing

1. Node starts and generates or loads an Ed25519 identity.
2. `net` advertises `_ticonnect._udp.local.` over mDNS with a TXT record
   carrying protocol version and fingerprint. UDP broadcast is the fallback
   when mDNS is unavailable.
3. Discovery produces candidates, not trust. Nothing is exchanged until pairing.
4. Pairing uses SPAKE2 keyed by a 6-digit PIN shown on one side and typed on
   the other, or a QR code containing node id, address, and fingerprint.
5. Successful pairing binds the peer fingerprint into the trusted-peer table
   in SQLite. Subsequent TLS connections use certificate pinning against that
   fingerprint.

All traffic is encrypted and authenticated. Unpaired peers can only reach the
pairing endpoint.

## 5. Messaging and files

### Replica model

Each node stores a full local replica of every conversation it participates in.
There is no server and no shared authority. A conversation is the peer pair;
group chats are out of scope in v1. Each side is authoritative only for the
messages it sent, and the receiver cannot delete or modify the sender's copy.

### Message records

- The sender generates a UUIDv7 id, signs the canonical record with its Ed25519
  identity key, and assigns a monotonic `seq` per conversation per sender.
- Messages are immutable. Edits and corrections are out of scope in v1.
- There is no global ordering. Render by sender timestamp, falling back to the
  local `received_at` when clocks skew.
- The receiver verifies signature and `seq` before insert; duplicates are
  ignored.

### Outbox

- Undelivered messages queue in `outbox` and retry on peer rediscovery.
- Undelivered messages expire after **7 days by default**, configurable per
  node. `expires_at` is stored per row; expired rows are dropped and surfaced
  in the UI as failed.
- LAN-only means delivery is delayed, never relayed through a third party.

### Gap sync on reconnect

1. Transport is authenticated by pinned fingerprint.
2. Both sides swap `peer_state` (`last_sent_seq` / `last_recv_seq`).
3. Each requests the missing ranges and streams them.
4. Records are verified and inserted; duplicates are ignored.
5. The outbox for that peer is flushed.

### Read receipts (v1)

- Receipts are signed ack records stored like messages: kinds `delivered` and
  `read`, keyed by the target message id.
- `delivered` means the receiver's daemon stored the message. `read` means the
  conversation was visible in the UI and the message was scrolled into view.
- Acks are coalesced over a short interval to avoid ack storms.
- Receipts are suppressed for peers under `mute`/`ignore` unless the message
  was actually read.
- A global config toggle disables sending read receipts; disabling it still
  honors inbound receipts.
- The UI shows per-message state: queued, sent, delivered, read, failed or
  expired.

### Files

- Text, images, audio clips, and arbitrary files.
- Attachments ride QUIC streams, chunked and resumable, in a blake3
  content-addressed store. Messages reference the hash, so duplicates dedupe
  naturally. Partial upload state is persisted.

### Policy

- Per-peer `accept` / `mute` / `ignore`, plus a global DND mode.
  `mute` stores messages silently; `ignore` drops them with no notification.
- Policy changes are exposed over D-Bus and through the tray menu.

### Schema sketch

```sql
peers(id TEXT PK, fingerprint TEXT, display_name TEXT, policy TEXT,
      trusted_at INTEGER, last_seen INTEGER)

conversations(id TEXT PK, peer_id TEXT, created_at INTEGER)

messages(id TEXT PK,            -- UUIDv7, sender-generated
         conversation_id TEXT,
         sender_id TEXT,
         seq INTEGER,           -- monotonic per sender per conversation
         sent_at INTEGER,
         received_at INTEGER,
         kind TEXT,             -- text|image|audio|file|ack
         target_id TEXT,        -- for ack records
         body TEXT,
         attachment_hash TEXT,
         signature BLOB,
         UNIQUE(conversation_id, sender_id, seq))

outbox(message_id TEXT PK, attempts INTEGER, next_try INTEGER,
       expires_at INTEGER)

attachments(hash TEXT PK, size INTEGER, path TEXT, mime TEXT)

peer_state(peer_id TEXT PK, last_sent_seq INTEGER, last_recv_seq INTEGER)
```

There is deliberately no `settings` table. Configuration lives in
`config.toml`; SQLite stores application data only.

## 6. Voice calls

- `cpal` captures and plays audio.
- Frames are encoded with Opus at 20ms and sent over QUIC datagrams.
- A jitter buffer with packet-loss concealment handles loss and reordering.
- The daemon owns the call state machine: `idle`, `ringing`, `in-call`,
  `ended`. State changes are D-Bus signals.
- Ringtone playback uses `rodio`, reads a configurable file, and loops until
  the call is answered or declined. It respects mute/DND and ducks under the
  call audio once connected.
- Ringtone runs inside the daemon so calls ring with the UI closed.

## 7. Remote desktop

View and control are both in v1.

1. The host opens a single **RemoteDesktop portal** session through `ashpd`.
   This yields the PipeWire capture stream and the EIS file descriptor for
   input injection in one consent grant.
2. Capture feeds the encoder. `openh264` encodes at 1080p in software.
3. The viewer decodes with `openh264` and renders in the GTK4 remote-desktop
   view.
4. Input is injected with `reis` (libei) over the portal EIS fd on Wayland;
   X11 falls back to XTEST through `x11rb`.
5. Consent is explicit: the host shows a dialog and a persistent "remote
   session active" indicator, and has a hard kill switch. Sessions are
   PIN-gated and scoped to a single trusted peer.

Remote desktop is the highest-risk feature. Pairing, per-session grants, the
visible indicator, and the kill switch are all mandatory, not optional.

## 8. UI

### Main window

- Peers and presence.
- Chat threads with text, images, audio, and file attachments.
- Call view: voice controls plus remote-desktop viewer.
- Settings: per-peer mute/ignore, ringtone file, discovery, autostart,
  overlay enable/disable.
- Pairing dialog (PIN or QR).

### Corner overlay

- Implemented with `gtk4-layer-shell` on Wayland: `Layer::Overlay`, anchored
  bottom-right, transparent window, rounded card.
- `KeyboardMode::None` so it never steals focus; only the card itself takes
  pointer input.
- A card slides in on `MessageReceived`, `IncomingCall`, and `TransferDone`.
- Clicking a card calls `org.ticonnect.UI.Activate(view)` and opens the app on
  the relevant screen.
- Incoming-call cards persist until answered or declined; other cards
  auto-dismiss after a timeout.
- X11 fallback: an undecorated always-on-top GTK4 window, since layer-shell is
  Wayland-only.

## 9. Security

- All traffic encrypted with TLS 1.3 over QUIC, certificates pinned to trusted
  fingerprints.
- No feature is reachable before pairing.
- No root. Screen capture and input injection go through portals.
- Remote control requires a per-session grant, a visible indicator, and a kill
  switch.
- Secrets and keys stay in the user config directory with restrictive
  permissions.

## 10. Phases

1. Workspace skeleton, config, D-Bus daemon, systemd user unit, CLI.
2. Identity, mDNS discovery, SPAKE2 pairing, TLS pinning.
3. Text messaging: signed records, outbox with expiry, gap sync, read receipts,
   SQLite history, mute/ignore, tray.
4. GTK4 main window, corner overlay, ringtone.
5. File/image/audio transfer with resume and content-addressed storage.
6. Voice calls: cpal, Opus, datagrams, jitter buffer, ringtone ducking.
7. Remote desktop: portal capture, openh264, libei/XTEST input, consent flow.
8. Packaging, flatpak, performance pass.

## 11. Risks and limits

- Wayland portals require user consent. Capture and input cannot be silent.
- Software H.264 at 1080p costs CPU; 4K is out of scope.
- `gtk4-layer-shell` exists only on Wayland; X11 needs the fallback window.
- The overlay agent costs about 40MB idle. Accepted; system notifications are
  the fallback when it is disabled.
- `reis`/libei and portal APIs vary across desktop versions; test on GNOME and
  KDE first.
- Outbox expiry loses undelivered messages after the TTL.
- Clock skew can reorder messages; there is no global ordering.
- Deletion is local-only; the sender's or receiver's remote copy cannot be
  removed.
