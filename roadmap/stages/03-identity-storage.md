# Stage 3 — Identity & Storage

## Goal

Give each node a persistent Ed25519 identity, a BLAKE3 fingerprint, and a SQLite
database with migrations for application data. Extend the TOML settings schema
and add the peer table. The daemon exposes identity and read-only settings over
D-Bus; any process may change settings through `ticonnect-settings`.

## Systems and components

Component IDs and relations: [stage component map](../../docs/systems/generated/stage-map.md#stage-3).
Component graphs: [ID](../../docs/systems/generated/components/id.md), [DB](../../docs/systems/generated/components/db.md), [CFG](../../docs/systems/generated/components/cfg.md).

## Scope

- Ed25519 key generation, loading, and storage.
- BLAKE3 fingerprint derivation.
- SQLite schema, migrations, and typed access layer for application data.
- TOML settings schema extensions with typed getters and validation.
- Peer table CRUD used by later stages.
- D-Bus methods for identity and read-only settings.

## Non-goals

- No discovery, pairing, or transport.
- No message tables beyond the schema sketch (tables may exist but stay unused).
- No UI.

## Prerequisites

Stages 1, 1.5, and 2 complete.

## Deliverables

- `crates/core/src/identity.rs`
- `crates/core/src/fingerprint.rs`
- `crates/daemon/src/storage/mod.rs`
- `crates/daemon/src/storage/migrations/0001_init.sql`
- `crates/daemon/src/storage/peers.rs`
- `crates/daemon/src/config_watch.rs`
- `crates/settings/src/settings/schema.rs` (Stage 3 keys)
- `crates/cli` subcommands `id show`, `settings get`, `settings set`,
  `peers list`.

## Identity

- [x] Generate an Ed25519 keypair on first run using `ed25519-dalek`.
- [x] Store the private key at `$XDG_CONFIG_HOME/ticonnect/identity.key`.
- [x] Store the public key and node id alongside or derive them deterministically.
- [x] Derive fingerprint as BLAKE3 of the public key, rendered as lowercase hex
  with grouping for display.
- [x] Generate node id as UUIDv7 or a truncated fingerprint; document the choice.
- [x] Create config dir with mode `0700` and key file with mode `0600`.
- [x] Refuse to start with a world-readable key; log a clear error.
- [x] Support key rotation explicitly out of scope for v1; document it.

## Storage

- [x] Open SQLite at `$XDG_DATA_HOME/ticonnect/ticonnect.db`.
- [x] Enable WAL mode and foreign keys.
- [x] Add a `schema_version` table and a migration runner.
- [x] Migration `0001_init.sql` creates:
  - [x] `peers`
  - [x] `conversations`
  - [x] `messages`
  - [x] `outbox`
  - [x] `attachments`
  - [x] `peer_state`
- [x] Keep configuration out of SQLite; there is no `settings` table.
- [x] Use `rusqlite` with the `bundled` feature.
- [x] Wrap writes in transactions where multiple rows change.
- [x] Store timestamps as Unix milliseconds in `INTEGER`.
- [x] Add indexes on `messages(conversation_id, sent_at)` and
  `messages(conversation_id, sender_id, seq)`.
- [x] Add a storage error type mapping SQLite errors to the daemon's storage
  error.

## Settings

- [x] Store configuration at `$XDG_CONFIG_HOME/ticonnect/config.toml`; SQLite
  never stores configuration.
- [x] Extend the `ticonnect-settings` schema with keys:
  `identity.display_name`, `discovery.enabled`, `overlay.enabled`,
  `read_receipts.enabled`, `outbox.ttl_days`, `ringtone.path`, `dnd.enabled`.
- [x] Defaults live in code; TOML stores explicit values only.
- [x] Validate `outbox.ttl_days` is positive and bounded in `ticonnect-settings`.
- [x] Any process may update settings through `ticonnect-settings`. Writes take
  an in-process mutex and an OS advisory lock, then replace `config.toml`
  atomically.
- [x] Concurrent writers are last-writer-wins per save; the lock prevents
  corruption, not lost updates.
- [x] `TICONNECT_*` environment overrides apply at runtime only and are never
  persisted.
- [x] The daemon watches `config.toml`, reloads on change, and emits
  `SettingsChanged`.
- [x] CLI `settings get` and `settings set` read and write TOML directly through
  `ticonnect-settings`.

## Peers

- [x] `peers` CRUD: insert, get by id, get by fingerprint, list, update policy,
  update last_seen, delete.
- [x] Policy enum: `accept`, `mute`, `ignore`.
- [x] Reject duplicate fingerprints with a typed conflict error.
- [x] Do not store peers before pairing; pairing populates this table.

## D-Bus additions

- [x] `GetIdentity() -> a{sv}` with node id, fingerprint, display name.
- [x] `GetSetting(s) -> v`, `ListSettings() -> a{sv}`.
- [x] `ListPeers() -> aa{sv}` returning empty until pairing exists.
- [x] Signal `SettingsChanged(s)` after the daemon reloads a changed config file.
- [x] No `SetSetting`; config writes go through `ticonnect-settings`.

## Tests

- [x] Key generation produces a valid keypair and stable fingerprint.
- [x] Loading an existing key returns the same fingerprint.
- [x] Key file permissions are enforced on load.
- [x] Migration runner applies from empty and is idempotent on rerun.
- [x] TOML settings round-trip for string, bool, integer, path, and optional
  values.
- [x] Invalid setting value returns a typed error.
- [x] Peer CRUD and policy update tests.
- [x] Concurrent reads and one writer do not lock under WAL in tests.
- [x] Concurrent config writers leave valid TOML and one complete snapshot.
- [x] Daemon reloads config after an external write and emits `SettingsChanged`.
- [x] Environment overrides are not persisted by save.

## Security

- [x] Key material never appears in logs or D-Bus responses.
- [x] Fingerprint display is deterministic and documented.
- [x] Database file permissions are `0600`; directory `0700`.
- [x] SQL uses parameters, never string interpolation.
- [x] Backup/export of the key is out of scope; document the consequence.

## Docs and acceptance

- [x] Document identity file locations and permissions.
- [x] Document schema and migration policy.
- [x] Document settings keys, types, defaults, and ranges.
- [x] Document the config/data split: TOML configuration versus SQLite
  application data.
- [x] Document cross-process config write semantics and daemon reload behavior.
- [x] Document that deleting the identity file creates a new node.

## Acceptance criteria

- First daemon start creates identity and database.
- `ticonnect-cli id show` prints node id and fingerprint.
- `ticonnect-cli settings set dnd.enabled true` writes `config.toml`; the daemon
  applies it without restart and emits `SettingsChanged`.
- Restarting the daemon does not change the fingerprint.
- Database reopens cleanly and migrations are a no-op on second run.
- SQLite contains no configuration table.

## Demo

1. Remove local state, start daemon, show created files and permissions.
2. `ticonnect-cli id show`.
3. Set a setting, show the change in `config.toml`, and observe the daemon
   reload and `SettingsChanged`.
4. Restart daemon and confirm fingerprint and setting persist.

## Risks and mitigations

- Risk: key loss makes the node untrusted. Mitigation: document clearly; no
  silent regeneration over an existing peer table.
- Risk: schema churn. Mitigation: migrations from day one and tests that apply
  all migrations to an empty database.
- Risk: concurrent config writers lose updates. Mitigation: keep writes atomic
  and locked, document last-writer-wins, and keep runtime config changes rare.

## Works without later stages

Identity, SQLite data storage, TOML settings, and peer CRUD work with no
discovery, pairing, transport, or UI.
