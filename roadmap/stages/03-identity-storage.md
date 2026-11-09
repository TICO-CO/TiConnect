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

- [ ] Generate an Ed25519 keypair on first run using `ed25519-dalek`.
- [ ] Store the private key at `$XDG_CONFIG_HOME/ticonnect/identity.key`.
- [ ] Store the public key and node id alongside or derive them deterministically.
- [ ] Derive fingerprint as BLAKE3 of the public key, rendered as lowercase hex
  with grouping for display.
- [ ] Generate node id as UUIDv7 or a truncated fingerprint; document the choice.
- [ ] Create config dir with mode `0700` and key file with mode `0600`.
- [ ] Refuse to start with a world-readable key; log a clear error.
- [ ] Support key rotation explicitly out of scope for v1; document it.

## Storage

- [ ] Open SQLite at `$XDG_DATA_HOME/ticonnect/ticonnect.db`.
- [ ] Enable WAL mode and foreign keys.
- [ ] Add a `schema_version` table and a migration runner.
- [ ] Migration `0001_init.sql` creates:
  - [ ] `peers`
  - [ ] `conversations`
  - [ ] `messages`
  - [ ] `outbox`
  - [ ] `attachments`
  - [ ] `peer_state`
- [ ] Keep configuration out of SQLite; there is no `settings` table.
- [ ] Use `rusqlite` with the `bundled` feature.
- [ ] Wrap writes in transactions where multiple rows change.
- [ ] Store timestamps as Unix milliseconds in `INTEGER`.
- [ ] Add indexes on `messages(conversation_id, sent_at)` and
  `messages(conversation_id, sender_id, seq)`.
- [ ] Add a storage error type mapping SQLite errors to the daemon's storage
  error.

## Settings

- [ ] Store configuration at `$XDG_CONFIG_HOME/ticonnect/config.toml`; SQLite
  never stores configuration.
- [ ] Extend the `ticonnect-settings` schema with keys:
  `identity.display_name`, `discovery.enabled`, `overlay.enabled`,
  `read_receipts.enabled`, `outbox.ttl_days`, `ringtone.path`, `dnd.enabled`.
- [ ] Defaults live in code; TOML stores explicit values only.
- [ ] Validate `outbox.ttl_days` is positive and bounded in `ticonnect-settings`.
- [ ] Any process may update settings through `ticonnect-settings`. Writes take
  an in-process mutex and an OS advisory lock, then replace `config.toml`
  atomically.
- [ ] Concurrent writers are last-writer-wins per save; the lock prevents
  corruption, not lost updates.
- [ ] `TICONNECT_*` environment overrides apply at runtime only and are never
  persisted.
- [ ] The daemon watches `config.toml`, reloads on change, and emits
  `SettingsChanged`.
- [ ] CLI `settings get` and `settings set` read and write TOML directly through
  `ticonnect-settings`.

## Peers

- [ ] `peers` CRUD: insert, get by id, get by fingerprint, list, update policy,
  update last_seen, delete.
- [ ] Policy enum: `accept`, `mute`, `ignore`.
- [ ] Reject duplicate fingerprints with a typed conflict error.
- [ ] Do not store peers before pairing; pairing populates this table.

## D-Bus additions

- [ ] `GetIdentity() -> a{sv}` with node id, fingerprint, display name.
- [ ] `GetSetting(s) -> v`, `ListSettings() -> a{sv}`.
- [ ] `ListPeers() -> aa{sv}` returning empty until pairing exists.
- [ ] Signal `SettingsChanged(s)` after the daemon reloads a changed config file.
- [ ] No `SetSetting`; config writes go through `ticonnect-settings`.

## Tests

- [ ] Key generation produces a valid keypair and stable fingerprint.
- [ ] Loading an existing key returns the same fingerprint.
- [ ] Key file permissions are enforced on load.
- [ ] Migration runner applies from empty and is idempotent on rerun.
- [ ] TOML settings round-trip for string, bool, integer, path, and optional
  values.
- [ ] Invalid setting value returns a typed error.
- [ ] Peer CRUD and policy update tests.
- [ ] Concurrent reads and one writer do not lock under WAL in tests.
- [ ] Concurrent config writers leave valid TOML and one complete snapshot.
- [ ] Daemon reloads config after an external write and emits `SettingsChanged`.
- [ ] Environment overrides are not persisted by save.

## Security

- [ ] Key material never appears in logs or D-Bus responses.
- [ ] Fingerprint display is deterministic and documented.
- [ ] Database file permissions are `0600`; directory `0700`.
- [ ] SQL uses parameters, never string interpolation.
- [ ] Backup/export of the key is out of scope; document the consequence.

## Docs and acceptance

- [ ] Document identity file locations and permissions.
- [ ] Document schema and migration policy.
- [ ] Document settings keys, types, defaults, and ranges.
- [ ] Document the config/data split: TOML configuration versus SQLite
  application data.
- [ ] Document cross-process config write semantics and daemon reload behavior.
- [ ] Document that deleting the identity file creates a new node.

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
