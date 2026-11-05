# Stage 1 — Foundation

## Goal

Create the Cargo workspace, crate skeleton, shared configuration, logging,
error handling, and CI. No networking, no UI, no D-Bus yet. This stage proves
the repository builds and the tooling is in place.

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
- `crates/core/src/{lib.rs,error.rs}`
- `crates/settings/src/{lib.rs,error.rs,log.rs,settings/}`
- `crates/logging/src/{lib.rs,error.rs}`
- `crates/daemon/src/main.rs` (binary `ticonnectd`)
- `crates/cli/src/main.rs` (binary `ticonnect-cli`)
- `crates/ui/src/main.rs` (binary `ticonnect-ui`)
- `crates/overlay/src/main.rs` (binary `ticonnect-ovl`)
- `.github/workflows/ci.yml`
- `README.md` with build and test commands.
- `rust-toolchain.toml` pinned to a stable channel.

## Setup

- [ ] Run `cargo new --lib` for each crate or create manifests by hand.
- [ ] Add workspace `[workspace.package]` for version, edition, license, authors.
- [ ] Add `[workspace.dependencies]` for `tokio`, `serde`, `postcard`, `tracing`,
  `thiserror`, `figment`, and `directories`.
- [ ] Add `anyhow` for binary entry points only, never for library APIs.
- [ ] Pin Rust toolchain in `rust-toolchain.toml`.
- [ ] Add `.gitignore` for `target/`, editor files, and local config.
- [ ] Add `rustfmt.toml` and `clippy.toml` if non-default rules are wanted.

## Implementation

- [ ] `core::error` defines a typed `Error` enum with `thiserror` and a `Result<T>`
  alias for shared protocol, identity, pairing, and crypto errors.
- [ ] Each library crate owns its own `Error` and `Result` types; there is no
  workspace-wide catch-all error enum.
- [ ] Each binary has `fn main() -> anyhow::Result<()>` and a placeholder
  `run() -> anyhow::Result<()>`; only `src/main.rs` uses `anyhow`.
- [ ] Binary targets are named `ticonnectd`, `ticonnect-cli`, `ticonnect-ui`,
  and `ticonnect-ovl`.
- [ ] `ticonnect-settings` is a leaf crate that embeds config privately and
  exposes only `Settings`, `LogSettings`, `LogFormat`, `Error`, and `Result`.
- [ ] `Settings` defines persisted settings:
  - [ ] `log_level` default `info`.
  - [ ] `log_file` optional.
  - [ ] `log_format` default human-readable, with optional JSON output.
  - [ ] `data_dir` optional.
- [ ] `Settings` owns file behavior internally: default location, read, write,
  save, reset. `Config` and its store are not exported.
- [ ] `TICONNECT_CONFIG_DIR` is bootstrap-only: it selects the user settings file
  location and is never merged into, or persisted as, a setting.
- [ ] `Settings` applies `TICONNECT_*` overrides internally. Callers see one
  effective value per setting, with no exposed runtime/persisted distinction.
- [ ] Load order for effective values: built-in defaults, user file, environment
  variables prefixed `TICONNECT_`. Environment always wins until explicitly set.
- [ ] Explicit `Settings::set_*` calls change the effective value immediately and
  are persisted only by `Settings::save`.
- [ ] `Settings::save` persists explicit changes only; untouched environment
  values are never persisted implicitly.
- [ ] Saves preserve unknown keys, remove legacy runtime-only `config_dir`, and
  skip writing when nothing changed.
- [ ] Settings writes take an in-process mutex and an OS advisory lock, and
  replace the file atomically.
- [ ] `Settings` can load from, save, and reset an explicit file; missing files
  are a no-op.
- [ ] `ticonnect-logging` depends on `ticonnect-settings` only and initializes
  `tracing-subscriber` with `EnvFilter`; it owns its own `Error`/`Result`.
- [ ] `core` depends on neither `ticonnect-settings` nor `ticonnect-logging`
  and never exposes logging-specific error variants.
- [ ] Logging supports human-readable output and optional JSON output.
- [ ] A future `ticonnect-runtime` crate is reserved in the roadmap for composing
  settings and process lifecycle; it is not part of this stage.
- [ ] `core` exposes `version()` from `CARGO_PKG_VERSION`.
- [ ] Each dependent crate has an empty `lib.rs` that compiles and depends on
  `core` per the dependency direction rule.
- [ ] No crate depends on a crate above it in the dependency graph.

## Tests

- [ ] Unit test: default settings values are correct.
- [ ] Unit test: environment override wins for effective settings values.
- [ ] Unit test: explicit `Settings::set_*` changes the effective value and is
  not written until `save`.
- [ ] Unit test: `config_dir` selects the user settings file without becoming
  settings data.
- [ ] Unit test: `data_dir` environment override is not persisted.
- [ ] Unit test: reset removes the user settings file and restores defaults.
- [ ] Unit test: save persists explicit changes but not environment values.
- [ ] Unit test: save preserves unknown keys, clears optional fields, and removes
  legacy `config_dir`.
- [ ] Unit test: concurrent saves leave valid TOML.
- [ ] Unit test: invalid settings return a typed error, not a panic.
- [ ] Smoke test: every workspace crate builds and links.
- [ ] Smoke test: `cargo build --workspace --bins` succeeds.
- [ ] Smoke test: each placeholder binary exits successfully.
- [ ] Test that logging initialization is idempotent or safely guarded.
- [ ] Unit test: invalid log filters return `ticonnect-logging`'s own typed error.

## Security

- [ ] Add `cargo audit` to CI.
- [ ] Add `#![forbid(unsafe_code)]` to `core` and all crates unless a crate has
  a documented reason to allow unsafe.
- [ ] Confirm no dependency pulls in a duplicate runtime by accident.
- [ ] No secrets or personal paths in committed files.

## Docs and acceptance

- [ ] `README.md` documents workspace layout and commands.
- [ ] `docs/` or crate-level docs explain the dependency direction rule.
- [ ] CI badge or status noted in `README.md`.

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
2. Show `cargo test -p ticonnect-core config` passing.
3. Show CI workflow file and local clippy/fmt results.

## Risks and mitigations

- Risk: workspace grows into feature code. Mitigation: keep placeholders empty
  and enforce non-goals in review.
- Risk: dependency bloat. Mitigation: workspace dependency table and audit.

## Works without later stages

The workspace builds and tests run with no daemon, network, or UI code present.
