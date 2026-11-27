# D-Bus IPC

The daemon exposes one IPC-bus service. The IPC bus defaults to the session bus;
set `TICONNECT_IPC_BUS_ADDRESS` to use a private bus. Clients never depend on
daemon internals.

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
| `GetIdentity` | `() -> a{sv}` | Returns node id, fingerprint, display name, avatar hash, and identity creation time. |
| `GetAvatar` | `(s) -> ay` | Returns a cached or local PNG avatar for a fingerprint. |
| `SetAvatar` | `(ay) -> ()` | Stores a normalized PNG profile photo. |
| `ClearAvatar` | `() -> ()` | Removes the local profile photo. |
| `GetSetting` | `(s) -> v` | Returns one effective setting; unknown keys return `NotFound`. |
| `ListSettings` | `() -> a{sv}` | Returns all effective settings. |
| `ListUsers` | `() -> aa{sv}` | Returns known users. |
| `ListCandidates` | `() -> aa{sv}` | Returns current discovery candidates. |
| `GetDiscoveryStatus` | `() -> a{sv}` | Returns discovery backend state and counters. |
| `GetNetworkStatus` | `() -> a{sv}` | Returns LAN access state, interfaces, ports, and counters. |
| `RunLanCheck` | `() -> a{sv}` | Sends an immediate discovery probe and waits up to 15 seconds for a peer or session. |
| `GetUser` | `(s) -> a{sv}` | Returns stable identity plus live address, MAC, OS, desktop, and display-server metadata. |
| `ForgetUser` | `(s) -> b` | Removes trust, local history, and queued outbox rows; preserves sequence state and closes an active session. |
| `SendMessage` | `(sss) -> s` | Queues a text message and returns its message id. The third argument is the id of the message being replied to, or empty for no reply. |
| `GetMessage` | `(s) -> a{sv}` | Returns one stored message by id. |
| `ListMessages` | `(s u u) -> aa{sv}` | Returns message history for a conversation. |
| `CountMessages` | `(s) -> u` | Returns the number of stored messages in a conversation. |
| `ClearHistory` | `(s) -> u` | Deletes local messages, queued outbox rows, and unreferenced attachments for one user; an empty user id clears all history. |
| `GetStorageUsage` | `() -> a{sv}` | Returns message count, attachment count and bytes, and SQLite database bytes. |
| `ListConversationSummaries` | `() -> aa{sv}` | Returns last message, unread count, and conversation id per user. |
| `MarkRead` | `(s as) -> ()` | Marks inbound messages read locally and queues receipts when enabled. |
| `MarkConversationRead` | `(s) -> ()` | Marks every unread inbound message in a conversation read and queues receipts when enabled. |
| `SetTyping` | `(sb) -> ()` | Sends a typing indicator to a connected trusted user. |
| `GetMessageState` | `(s) -> s` | Returns the current state for a message id. |
| `RetryMessage` | `(s) -> b` | Requeues a failed or expired outgoing message. |
| `EditMessage` | `(ss) -> ()` | Publishes a signed edit for a local text message. Edits are accepted within 5 minutes of sending. |
| `DeleteMessage` | `(s) -> ()` | Publishes a signed tombstone for a local message. |
| `SetUserPolicy` | `(ss) -> ()` | Sets `accept`, `mute`, or `ignore` for one user. |
| `SetDnd` | `(b) -> ()` | Toggles global do-not-disturb mode and persists it. |
| `StartCall` | `(s) -> s` | Starts an outgoing call and returns its call id. |
| `AcceptCall` | `(s) -> ()` | Accepts an incoming call and opens audio devices. |
| `DeclineCall` | `(s) -> ()` | Declines an incoming ringing call. |
| `HangupCall` | `(s) -> ()` | Ends an active call; also cancels an outgoing connecting or ringing call. |
| `SetCallMuted` | `(sbb) -> ()` | Sets microphone and output mute for the active call. |
| `SetCallVolume` | `(su) -> ()` | Sets the active call's per-user output scale; valid range `0..=100`. |
| `GetCallState` | `() -> a{sv}` | Returns the active call, or an empty map when idle. |
| `ListCallLogs` | `(su) -> aa{sv}` | Returns persisted call logs for one user, or all when the user id is empty. |
| `ListAudioDevices` | `() -> aa{sv}` | Returns discovered input and output devices. |
| `RequestRemoteSession` | `(s) -> a{sv}` | Requests remote desktop consent from one user. |
| `AcceptRemoteSession` | `(s) -> ()` | Grants a pending remote desktop request. |
| `DenyRemoteSession` | `(s) -> ()` | Denies a pending remote desktop request. |
| `EndRemoteSession` | `(s) -> ()` | Ends the active remote desktop session. |
| `SetRemoteInput` | `(sb) -> ()` | Enables or blocks pointer and keyboard input for an active remote session. |
| `GetRemoteSession` | `() -> a{sv}` | Returns the active remote session, or an empty map. |
| `Shutdown` | `() -> ()` | Requests a clean shutdown. |

Settings writes do not use D-Bus. Any process may update `config.toml` through
`ticonnect-settings`; the daemon reloads it and emits `SettingsChanged`.

## UI service

The GTK UI owns `org.ticonnect.UI` on the TiConnect IPC bus.

- Bus name: `org.ticonnect.UI`
- Object path: `/org/ticonnect/UI`
- Interface: `org.ticonnect.UI`

| Method | Signature | Behavior |
|---|---|---|
| `Activate` | `(s) -> ()` | Raises the window and switches to `users`, `chat`, `settings`, `remote`, or `watch-party`. Empty means `chat`; unknown views return `InvalidArgs`. |
| `IsActive` | `() -> b` | Reports whether the main window is focused. |

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
| `UserConnected(a{sv})` | When a trusted QUIC session becomes active. |
| `UserDisconnected(a{sv})` | When a trusted QUIC session closes. |
| `MessageReceived(a{sv})` | When an inbound message is stored. |
| `MessageStateChanged(ss)` | When a message state changes. |
| `MessageEdited(sssu)` | When a message body changes from an edit record. |
| `MessageDeleted(ss)` | When a message receives a delete tombstone. |
| `ConversationChanged(s)` | When a conversation summary should be reloaded. |
| `TypingChanged(sb)` | When a user starts or stops typing. |
| `UserProfileChanged(s)` | When a cached user profile or avatar changes. |
| `WatchPartyRequested(a{sv})` | When a YouTube watch-party invite arrives. |
| `WatchPartyChanged(a{sv})` | When watch-party playback state changes. |
| `WatchPartyEnded(ss)` | When a watch party ends. |
| `SelfProfileChanged()` | When the local display name or avatar changes. |
| `SyncCompleted(su)` | When gap sync finishes for a user. |
| `PolicyChanged(ss)` | When a user policy changes. |
| `IncomingCall(a{sv})` | When a trusted user offers a call. |
| `CallStateChanged(ss)` | When the active call state changes. |
| `CallMuteChanged(sbb)` | When call microphone or output mute changes. |
| `CallEnded(ss)` | When the active call ends. |
| `RemoteIndicatorChanged(sb)` | When the remote sharing indicator visibility changes. |
| `RemoteInputChanged(sb)` | When remote pointer and keyboard input is enabled or blocked. |

## Status keys

`GetStatus` returns these keys:

| Key | Type | Meaning |
|---|---|---|
| `uptime_seconds` | `u64` | Seconds since the daemon started. |
| `user_count` | `u32` | Rows in the users table. |
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
| `avatar_hash` | `s` | BLAKE3 hex hash of the local PNG avatar, or empty. |
| `created_at` | `u64` | Unix milliseconds when the local identity was created, or its key metadata fallback. |

## Storage usage keys

`GetStorageUsage` returns:

| Key | Type | Meaning |
|---|---|---|
| `message_count` | `u64` | Stored visible messages; excludes acks and lifecycle records. |
| `attachment_count` | `u64` | Rows in the attachments table. |
| `attachment_bytes` | `u64` | Sum of attachment sizes. |
| `database_bytes` | `u64` | SQLite page count multiplied by page size. |

## Setting keys

`GetSetting` and `ListSettings` use the keys `log_level`, `log_file`,
`log_format`, `data_dir`, `identity.display_name`, `discovery.enabled`,
`discovery.mdns`, `discovery.broadcast`, `discovery.beacon_interval_secs`,
`transport.port`, `overlay.enabled`, `overlay.timeout_secs`,
`read_receipts.enabled`, `outbox.ttl_days`, `dnd.enabled`, and
`appearance.color_scheme`. Paths are strings;
unset optional paths are empty strings. `appearance.color_scheme` is `system`,
`light`, or `dark`. Discovery candidate addresses are comma-separated strings.

## User keys

`ListUsers` returns one map per user with `id`, `fingerprint`, `display_name`,
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

## Network status keys

`GetNetworkStatus` and `RunLanCheck` return:

| Key | Type | Meaning |
|---|---|---|
| `state` | `s` | `disabled`, `no_interface`, `bind_failed`, `discovery_down`, `no_peers`, `peers_discovered`, `peers_unreachable`, or `reachable`. |
| `confidence` | `s` | `confirmed`, `probable`, or `unknown`. |
| `hint` | `s` | User-facing next step. |
| `interfaces` | `s` | Comma-separated non-loopback IPv4 interfaces. |
| `interface_count` | `u32` | Number of usable LAN interfaces. |
| `transport_port` | `u32` | Bound QUIC transport port, or `0` when unbound. |
| `transport_bound` | `b` | Whether the transport socket is bound. |
| `mdns_active` | `b` | Whether the mDNS backend is running. |
| `broadcast_active` | `b` | Whether the broadcast backend is running. |
| `candidate_count` | `u32` | Current discovered peers. |
| `active_sessions` | `u32` | Active trusted transport sessions. |
| `beacons_sent` | `u64` | Broadcast beacons sent since daemon start. |
| `beacons_received` | `u64` | Broadcast beacons received since daemon start. |
| `beacon_send_errors` | `u64` | Failed beacon sends. |
| `dropped_packets` | `u64` | Invalid or discarded discovery packets. |
| `last_beacon_sent_at` | `u64` | Unix milliseconds, or `0`. |
| `last_beacon_received_at` | `u64` | Unix milliseconds, or `0`. |
| `last_candidate_at` | `u64` | Unix milliseconds of the newest candidate, or `0`. |
| `last_session_at` | `u64` | Unix milliseconds of the last session, or `0`. |
| `last_failure_at` | `u64` | Unix milliseconds of the last transport failure, or `0`. |
| `transport_failures` | `u64` | Transport failures since daemon start. |

## Errors

Stable D-Bus error names live under `org.ticonnect.Daemon.Error.*`. Reserved
names are `Failed` and `NotFound`.

## CLI

`ticonnect` maps the methods directly:

```sh
ticonnect status
ticonnect status --json
ticonnect version
ticonnect ping
ticonnect id show
ticonnect users list
ticonnect discover list
ticonnect discover watch
ticonnect network status
ticonnect network status --json
ticonnect network check
ticonnect users forget <user-id>
ticonnect quit
```

`ticonnect cli <command>` is a hidden compatibility alias for the same client.
`ticonnect version` prints the binary version; `ticonnect cli version` asks the
daemon for its version.

Settings commands use `ticonnect-settings` directly and do not require the
daemon:

```sh
ticonnect settings get dnd.enabled
ticonnect settings set dnd.enabled true
ticonnect settings list
```

`status --json` emits an object with the same status keys. Daemon commands start
the daemon on demand. If it remains unreachable, they exit non-zero with a typed
connection error.

## Packaging

`packaging/systemd/ticonnectd.service` is a `Type=dbus` user unit with the
daemon bus name. `packaging/autostart/ticonnect.desktop` starts the daemon in
desktop sessions without systemd. Both use `@BINDIR@`, substituted by the
Makefile with the install `BINDIR`.

`make install-user` also installs the desktop entry, icon, metainfo, the overlay
and UI units, and the overlay and UI autostart entries. The AppImage
`--install-user` installer writes the same user integration.

```sh
make install-user
systemctl --user daemon-reload
systemctl --user enable --now ticonnectd.service
systemctl --user status ticonnectd.service
```

Remove the installed user integration with:

```sh
make uninstall-user
```

## Debugging

Run the daemon in the foreground:

```sh
cargo run --bin ticonnect -- daemon
```

The legacy `ticonnectd` bin remains as a compatibility entry point. It keeps
running until `ticonnect quit`, SIGTERM, SIGINT, or bus disconnection. No display
server is required.
