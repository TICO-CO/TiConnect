# D-Bus IPC

The daemon exposes one session-bus service. Clients never depend on daemon
internals.

## Service

- Bus name: `org.ticonnect.Daemon`
- Object path: `/org/ticonnect/Daemon`
- Interface: `org.ticonnect.Daemon`

The daemon requests the name without queueing or replacement. A second instance
fails to acquire the name and exits non-zero.

## Methods

| Method | Signature | Behavior |
|---|---|---|
| `Ping` | `() -> s` | Returns `pong`. |
| `GetVersion` | `() -> s` | Returns the daemon package version. |
| `GetStatus` | `() -> a{sv}` | Returns live daemon status. |
| `GetIdentity` | `() -> a{sv}` | Returns node id, fingerprint, and display name. |
| `GetSetting` | `(s) -> v` | Returns one effective setting; unknown keys return `NotFound`. |
| `ListSettings` | `() -> a{sv}` | Returns all effective settings. |
| `ListPeers` | `() -> aa{sv}` | Returns known peers; empty before pairing. |
| `ListCandidates` | `() -> aa{sv}` | Returns current discovery candidates. |
| `GetDiscoveryStatus` | `() -> a{sv}` | Returns discovery backend state and counters. |
| `StartPairing` | `() -> a{sv}` | Starts a pairing session and returns the PIN, QR payload, and expiry. |
| `PairWithPin` | `(ss) -> b` | Pairs with a discovered node using a PIN. |
| `PairWithQr` | `(s) -> b` | Pairs using a QR payload. |
| `CancelPairing` | `() -> ()` | Cancels the active pairing session. |
| `ForgetPeer` | `(s) -> b` | Removes trust, local history, and queued outbox rows; preserves sequence state and closes an active session. |
| `ListTrustedPeers` | `() -> aa{sv}` | Returns trusted peers. |
| `SendMessage` | `(ss) -> s` | Queues a text message and returns its message id. |
| `ListMessages` | `(s u u) -> aa{sv}` | Returns message history for a conversation. |
| `CountMessages` | `(s) -> u` | Returns the number of stored text messages in a conversation. |
| `MarkRead` | `(s as) -> ()` | Marks inbound messages read and queues receipts. |
| `GetMessageState` | `(s) -> s` | Returns the current state for a message id. |
| `SetPeerPolicy` | `(ss) -> ()` | Sets `accept`, `mute`, or `ignore` for one peer. |
| `SetDnd` | `(b) -> ()` | Toggles global do-not-disturb mode and persists it. |
| `Shutdown` | `() -> ()` | Requests a clean shutdown. |

Settings writes do not use D-Bus. Any process may update `config.toml` through
`ticonnect-settings`; the daemon reloads it and emits `SettingsChanged`.

## UI service

The GTK UI owns `org.ticonnect.UI` on the session bus.

- Bus name: `org.ticonnect.UI`
- Object path: `/org/ticonnect/UI`
- Interface: `org.ticonnect.UI`

| Method | Signature | Behavior |
|---|---|---|
| `Activate` | `(s) -> ()` | Raises the window and switches to `peers`, `chat`, `settings`, or `pairing`. Empty means `chat`; unknown views return `InvalidArgs`. |

The tray uses `Activate("chat")`. Closing the UI does not affect the daemon.
Starting a second UI forwards `Activate("chat")` to the running instance and exits.

## Properties

| Property | Signature | Access |
|---|---|---|
| `Version` | `s` | read |

## Signals

| Signal | When |
|---|---|
| `DaemonStarted` | After the bus name and objects are ready. |
| `DaemonStopping` | Before the bus name is released. |
| `SettingsChanged(s)` | Once per setting changed by a config reload. |
| `CandidateFound(a{sv})` | When discovery adds or refreshes a candidate identity. |
| `CandidateLost(s)` | When a candidate expires, is removed, or discovery stops. |
| `PairingStarted(a{sv})` | When a pairing session starts. |
| `PairingCompleted(ss)` | With the peer id and fingerprint after trust is committed. |
| `PairingFailed(s)` | With a stable reason when pairing fails. |
| `PeerConnected(a{sv})` | When a trusted QUIC session becomes active. |
| `PeerDisconnected(a{sv})` | When a trusted QUIC session closes. |
| `MessageReceived(a{sv})` | When an inbound message is stored. |
| `MessageStateChanged(ss)` | When a message state changes. |
| `SyncCompleted(su)` | When gap sync finishes for a peer. |
| `PolicyChanged(ss)` | When a peer policy changes. |

## Status keys

`GetStatus` returns these keys:

| Key | Type | Meaning |
|---|---|---|
| `uptime_seconds` | `u64` | Seconds since the daemon started. |
| `peer_count` | `u32` | Rows in the peers table. |
| `outbox_depth` | `u32` | Queued messages; `0` before messaging lands. |
| `transport_state` | `s` | `idle` with no sessions, `online` with at least one session. |
| `active_sessions` | `u32` | Number of active trusted QUIC sessions. |

## Identity keys

`GetIdentity` returns:

| Key | Type | Meaning |
|---|---|---|
| `node_id` | `s` | Persistent UUIDv7 node id. |
| `fingerprint` | `s` | Lowercase hex BLAKE3 fingerprint of the Ed25519 public key. |
| `display_name` | `s` | Effective `identity.display_name`, possibly empty. |

## Setting keys

`GetSetting` and `ListSettings` use the keys `log_level`, `log_file`,
`log_format`, `data_dir`, `identity.display_name`, `discovery.enabled`,
`discovery.mdns`, `discovery.broadcast`, `discovery.beacon_interval_secs`,
`transport.port`, `overlay.enabled`, `read_receipts.enabled`, `outbox.ttl_days`,
`ringtone.path`, `dnd.enabled`, and `appearance.color_scheme`. Paths are strings;
unset optional paths are empty strings. `appearance.color_scheme` is `system`,
`light`, or `dark`. Discovery candidate addresses are comma-separated strings.

## Pairing keys

`StartPairing` returns:

| Key | Type | Meaning |
|---|---|---|
| `pin` | `s` | Six-digit pairing PIN. |
| `qr` | `s` | QR payload with node id, address, port, fingerprint, token, and expiry. |
| `expires_at` | `u64` | Unix seconds when the PIN and token expire. |
| `node_id` | `s` | This node's UUIDv7. |
| `port` | `u32` | Advertised QUIC port. |

## Peer keys

`ListPeers` returns one map per peer with `id`, `fingerprint`, `display_name`,
`policy`, `trusted_at`, `last_seen`, and `connected`. `policy` is `accept`,
`mute`, or `ignore`. `last_seen` is `0` when unset. `connected` is true when an
active transport session exists for that node.

## Candidate keys

`ListCandidates` and `CandidateFound` return one map per candidate:

| Key | Type | Meaning |
|---|---|---|
| `node_id` | `s` | Candidate UUIDv7 node id. |
| `fingerprint` | `s` | Lowercase hex BLAKE3 fingerprint. |
| `display_name` | `s` | Advertised display name, possibly empty. |
| `addresses` | `s` | Comma-separated validated IP addresses. |
| `port` | `u32` | Advertised QUIC transport port. |
| `protocol_version` | `u32` | Discovery protocol version. |
| `source` | `s` | `mdns` or `broadcast`. |
| `last_seen` | `u64` | Unix milliseconds of the last sighting. |

## Discovery status keys

`GetDiscoveryStatus` returns:

| Key | Type | Meaning |
|---|---|---|
| `enabled` | `b` | Whether discovery is enabled. |
| `mdns_enabled` | `b` | Whether mDNS is configured. |
| `broadcast_enabled` | `b` | Whether UDP broadcast is configured. |
| `mdns_active` | `b` | Whether the mDNS backend is running. |
| `broadcast_active` | `b` | Whether the broadcast backend is running. |
| `candidate_count` | `u32` | Current candidate count. |
| `dropped_packets` | `u64` | Invalid or discarded packets and candidates. |

## Errors

Stable D-Bus error names live under `org.ticonnect.Daemon.Error.*`. Reserved
names are `Failed` and `NotFound`.

## CLI

`ticonnect-cli` maps the methods directly:

```sh
ticonnect-cli status
ticonnect-cli status --json
ticonnect-cli version
ticonnect-cli ping
ticonnect-cli id show
ticonnect-cli peers list
ticonnect-cli discover list
ticonnect-cli discover watch
ticonnect-cli pair start
ticonnect-cli pair with-pin <node-id> <pin>
ticonnect-cli pair with-qr '<payload>'
ticonnect-cli pair cancel
ticonnect-cli pair forget <peer-id>
ticonnect-cli quit
```

Settings commands use `ticonnect-settings` directly and do not require the
daemon:

```sh
ticonnect-cli settings get dnd.enabled
ticonnect-cli settings set dnd.enabled true
ticonnect-cli settings list
```

`status --json` emits an object with the same status keys. When the daemon is
absent, daemon commands exit non-zero with a typed connection error.

## Packaging

`packaging/systemd/ticonnectd.service` is a `Type=dbus` user unit with the
daemon bus name. `packaging/autostart/ticonnect.desktop` starts the daemon in
desktop sessions without systemd. Both use `@BINDIR@`, substituted by the
Makefile with the install `BINDIR`.

```sh
make install-user
systemctl --user daemon-reload
systemctl --user enable --now ticonnectd.service
systemctl --user status ticonnectd.service
```

Remove the installed binary, unit, and autostart entry with:

```sh
make uninstall-user
```

## Debugging

Run the daemon in the foreground:

```sh
cargo run --bin ticonnectd
```

It keeps running until `ticonnect-cli quit`, SIGTERM, SIGINT, or bus
disconnection. No display server is required.
