# TiConnect

LAN-only daemon for messaging, images, audio, voice calls, and remote desktop.
Linux first. Background-first: the daemon receives messages and calls even when
the UI was never opened.

[![CI](https://github.com/TICO-CO/TiConnect/actions/workflows/ci.yml/badge.svg)](https://github.com/TICO-CO/TiConnect/actions/workflows/ci.yml)

## Status

Stages 1, 1.5, and 2 are implemented: workspace, settings, logging, typed
errors, binary scaffolds, the canonical architecture graph, the D-Bus daemon,
the CLI, and systemd/autostart packaging. Identity, discovery, transport,
messaging, and UI stages are not implemented yet.

See `main.md` for product intent, `architecture.md` for the design,
`docs/systems/` for component contracts and graphs, and `roadmap/roadmap.md` for
staged delivery.

## Workspace

| Crate | Purpose |
| --- | --- |
| `ticonnect-settings` | Persisted and effective settings, leaf crate |
| `ticonnect-logging` | `tracing-subscriber` setup, leaf crate |
| `ticonnect-core` | Shared domain types |
| `ticonnect-net` | Discovery and transport |
| `ticonnect-media` | Audio and video codecs |
| `ticonnect-desktop` | Capture and input integration |
| `ticonnect-daemon` | Background daemon, binary `ticonnectd` |
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
```

Architecture docs are generated. After editing `docs/systems/graph/` or
`docs/systems/state-machines.toml`, run `cargo xtask systems`.

## Run binaries

```sh
cargo run --bin ticonnectd
cargo run --bin ticonnect-cli -- status
cargo run --bin ticonnect-cli -- ping
cargo run --bin ticonnect-cli -- quit
cargo run --bin ticonnect-ui
cargo run --bin ticonnect-ovl
```

`ticonnectd` serves `org.ticonnect.Daemon` on the session bus until shutdown.
`ticonnect-cli` exposes `status`, `version`, `ping`, and `quit`. The UI and
overlay binaries still load settings, initialize logging, and exit.

## Configuration

- User config file: `$XDG_CONFIG_HOME/ticonnect/config.toml`.
- User data directory: `$XDG_DATA_HOME/ticonnect`.
- `TICONNECT_CONFIG_DIR` selects the config directory before settings load. It is
  bootstrap-only and is never persisted.
- `TICONNECT_*` environment variables override settings at runtime. They are not
  persisted.
- The discovered config directory is created with mode `0700`; `config.toml` is
  written with mode `0600`.
- Settings writes use an in-process mutex, an OS advisory lock, and atomic
  replacement. Saves persist explicit changes only.

## Documentation

User-facing documentation lives in this file and `docs/`. Rustdoc comments are
not used in this repository.
