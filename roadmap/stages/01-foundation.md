# Stage 1 — Foundation

## Goal

Create the Cargo workspace, crate skeleton, shared configuration, logging,
error handling, and CI. No networking, no UI, no D-Bus yet. This stage proves
the repository builds and the tooling is in place.

## Systems and components

Component IDs and relations: [stage component map](../../docs/systems/generated/stage-map.md#stage-1).
System contracts: [docs/systems](../../docs/systems/README.md).

## Scope

- Workspace and crate layout exactly as described in `architecture.md`.
- Typed error and result alias per library crate.
- Binary entry-point scaffolds using `anyhow` for top-level error reporting.
- Configuration loading from TOML with defaults and environment overrides.
- `tracing` setup with configurable level and file/stdout output.
- CI workflow running build, test, clippy, fmt, and audit.
- Test harness and a couple of smoke tests.

## Non-goals

- No daemon process, D-Bus, systemd unit, or CLI behavior. The four binaries are
  stubs that start and exit successfully.
- No identity, storage, discovery, transport, or UI code.
- No protocol implementation beyond empty module placeholders.

## Prerequisites

None.

## Deliverables

- `Cargo.toml` workspace with members:
  - `crates/core`
  - `crates/net`
  - `crates/media`
  - `crates/desktop`
  - `crates/daemon`
  - `crates/ui`
  - `crates/overlay`
  - `crates/cli`
- `crates/core/src/lib.rs`
- `crates/settings/src/{lib.rs,error.rs,overrides.rs,settings/,storage/}`
- `crates/logging/src/{lib.rs,error.rs,config.rs,settings.rs,logging.rs,output.rs,types.rs}`
- `crates/daemon/src/main.rs` (binary `ticonnectd`)
- `crates/cli/src/main.rs` (binary `ticonnect-cli`)
- `crates/ui/src/main.rs` (binary `ticonnect-ui`)
- `crates/overlay/src/main.rs` (binary `ticonnect-ovl`)
- `.github/workflows/ci.yml`
- `README.md` with build and test commands.
- `rust-toolchain.toml` pinned to a stable channel.

## Setup

- [x] Run `cargo new --lib` for each crate or create manifests by hand.
- [x] Add workspace `[workspace.package]` for version, edition, license.
- [x] Add `[workspace.dependencies]` for `tokio`, `serde`, `postcard`, `tracing`,
  `thiserror`, `figment`, and `directories`.
- [x] Add `anyhow` for binary entry points only, never for library APIs.
- [x] Pin Rust toolchain in `rust-toolchain.toml`.
- [x] Add `.gitignore` for `target/`, editor files, and local config.
- [x] Use default `rustfmt` and `clippy` rules; add config only for non-default
  rules.

## Implementation

- [x] `core` exposes domain types only and defines no crate-wide `Error` type;
  fallible `core` modules define local errors when they land.
- [x] Each fallible library crate owns its own `Error` and `Result` types and
  maps dependency failures into them; there is no workspace-wide catch-all
  error enum.
- [x] Each binary has `fn main() -> anyhow::Result<()>` and a placeholder
  `run() -> anyhow::Result<()>`; only `src/main.rs` uses `anyhow`.
- [x] Entry points convert crate errors to `anyhow` and add context around each
  startup step.
- [x] Binary targets are named `ticonnectd`, `ticonnect-cli`, `ticonnect-ui`,
  and `ticonnect-ovl`.
- [x] `ticonnect-settings` is a leaf crate that embeds config privately and
  exposes only `Settings`, `Error`, and `Result`. Logging domain types stay in
  `ticonnect-logging`.
- [x] `Settings` defines persisted settings:
  - [x] `log_level` default `info`.
  - [x] `log_file` optional.
  - [x] `log_format` default human-readable, with optional JSON output.
  - [x] `data_dir` optional.
- [x] `Settings` owns file behavior internally: default location, read, write,
  save, reset. `Config` and its store are not exported.
- [x] `TICONNECT_CONFIG_DIR` is bootstrap-only: it selects the user settings file
  location and is never merged into, or persisted as, a setting.
- [x] `Settings` applies `TICONNECT_*` overrides internally. Callers see one
  effective value per setting, with no exposed runtime/persisted distinction.
- [x] Load order for effective values: built-in defaults, user file, environment
  variables prefixed `TICONNECT_`. Environment always wins until explicitly set.
- [x] Explicit `Settings::set_*` calls change the effective value immediately and
  are persisted only by `Settings::save`.
- [x] `Settings::save` persists explicit changes only; untouched environment
  values are never persisted implicitly.
- [x] Saves preserve unknown keys and skip writing when nothing changed.
- [x] Settings writes take an in-process mutex and an OS advisory lock, and
  replace the file atomically.
- [x] `Settings` can load from, save, and reset an explicit file; missing files
  are a no-op.
- [x] `ticonnect-logging` depends on neither `ticonnect-settings` nor
  `ticonnect-core`; it owns its own `Error`/`Result`, parses the log level into
  `LogLevel` and validates `LogFormat` at `LogSettings::new`, and initializes
  `tracing-subscriber` with `EnvFilter`.
- [x] Each binary entrypoint originally loaded `Settings`, built `LogSettings`,
  and called `Logging::init`. `ticonnect-runtime` now centralizes this
  composition.
- [x] `core` depends on neither `ticonnect-settings` nor `ticonnect-logging`
  and never exposes logging-specific error variants.
- [x] Logging supports human-readable output and optional JSON output.
- [x] `ticonnect-runtime` was reserved in this stage and is now the composition
  root for `settings`, `logging`, process lifecycle, and the daemon binary.
- [x] `core` exposes `version()` from `CARGO_PKG_VERSION`.
- [x] Each dependent crate has an empty `lib.rs` that compiles and depends on
  `core` per the dependency direction rule.
- [x] No crate depends on a crate above it in the dependency graph.

## Tests

- [x] Unit test: default settings values are correct.
- [x] Unit test: environment override wins for effective settings values.
- [x] Unit test: explicit `Settings::set_*` changes the effective value and is
  not written until `save`.
- [x] Unit test: `config_dir` selects the user settings file without becoming
  settings data.
- [x] Unit test: `data_dir` environment override is not persisted.
- [x] Unit test: reset removes the user settings file and restores defaults.
- [x] Unit test: save persists explicit changes but not environment values.
- [x] Unit test: save preserves unknown keys and clears optional fields.
- [x] Unit test: concurrent saves leave valid TOML.
- [x] Unit test: invalid settings return a typed error, not a panic.
- [x] Smoke test: every workspace crate builds and links.
- [x] Smoke test: `cargo build --workspace --bins` succeeds.
- [x] Smoke test: each placeholder binary exits successfully.
- [x] Test that logging initialization is idempotent or safely guarded.
- [x] Unit test: invalid log levels or formats return `ticonnect-logging`'s own
  typed error from `LogSettings::new`.

## Security

- [x] Add `cargo audit` to CI.
- [x] Add `#![forbid(unsafe_code)]` to `core` and all crates unless a crate has
  a documented reason to allow unsafe.
- [x] Confirm no dependency pulls in a duplicate runtime by accident.
- [x] No secrets or personal paths in committed files.

## Docs and acceptance

- [x] `README.md` documents workspace layout and commands.
- [x] `docs/dependency-direction.md` explains the dependency direction rule.
- [x] CI badge or status noted in `README.md`.
- [x] No rustdoc comments in source; user-facing docs live in `README.md` and
  `docs/`.

## Acceptance criteria

- `cargo build --workspace` succeeds.
- `cargo build --workspace --bins` succeeds.
- `cargo test --workspace` passes.
- `cargo clippy --workspace --all-targets -- -D warnings` passes.
- `cargo fmt --all -- --check` passes.
- A config file can be loaded and a log line is emitted at the configured level.

## Demo

1. Run each placeholder binary and confirm it exits successfully; no daemon,
   CLI, UI, or overlay behavior exists yet.
2. Show `cargo test -p ticonnect-settings` passing.
3. Show CI workflow file and local clippy/fmt results.

## Risks and mitigations

- Risk: workspace grows into feature code. Mitigation: keep placeholders empty
  and enforce non-goals in review.
- Risk: dependency bloat. Mitigation: workspace dependency table and audit.

## Works without later stages

The workspace builds and tests run with no daemon, network, or UI code present.
