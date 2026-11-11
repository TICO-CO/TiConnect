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
| `Shutdown` | `() -> ()` | Requests a clean shutdown. |

Settings writes do not use D-Bus. Any process may update `config.toml` through
`ticonnect-settings`; the daemon reloads it and emits `SettingsChanged`.

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

## Status keys

`GetStatus` returns these keys:

| Key | Type | Meaning |
|---|---|---|
| `uptime_seconds` | `u64` | Seconds since the daemon started. |
| `peer_count` | `u32` | Rows in the peers table. |
| `outbox_depth` | `u32` | Queued messages; `0` before messaging lands. |
| `transport_state` | `s` | Transport state; `offline` before transport lands. |

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
`overlay.enabled`, `read_receipts.enabled`, `outbox.ttl_days`, `ringtone.path`,
and `dnd.enabled`. Paths are strings; unset optional paths are empty strings.
Discovery candidate addresses are comma-separated strings.

## Peer keys

`ListPeers` returns one map per peer with `id`, `fingerprint`, `display_name`,
`policy`, `trusted_at`, and `last_seen`. `policy` is `accept`, `mute`, or
`ignore`. `last_seen` is `0` when unset.

## Candidate keys

`ListCandidates` and `CandidateFound` return one map per candidate:

| Key | Type | Meaning |
|---|---|---|
| `node_id` | `s` | Candidate UUIDv7 node id. |
| `fingerprint` | `s` | Lowercase hex BLAKE3 fingerprint. |
| `display_name` | `s` | Advertised display name, possibly empty. |
| `addresses` | `s` | Comma-separated validated IP addresses. |
| `port` | `u32` | Advertised transport port; `0` before Stage 5. |
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
