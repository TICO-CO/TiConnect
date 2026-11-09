# Dependency Direction

TiConnect keeps crate dependencies one-directional. A crate may depend on crates
below it, never above it.

`xtask` is a dev-only tool. It is not part of the product dependency graph.
`cargo xtask systems --check` validates the declared build edges in
`docs/systems/graph/build-deps.toml` against `cargo metadata`.

```
settings   logging        leaf crates
   |          |
   +----+-----+
        |
      core
        |
   +----+-----+----------+
   |    |     |          |
  net  media desktop    (protocol consumers)
   |    |     |
   +----+-----+
        |
      daemon
        |
   +----+-----+----------+
   |    |     |          |
   ui  overlay  cli     binaries
```

## Rules

- `ticonnect-settings` and `ticonnect-logging` are leaf crates.
- `ticonnect-logging` depends on neither `ticonnect-settings` nor
  `ticonnect-core`.
- `ticonnect-core` depends on neither `ticonnect-settings` nor
  `ticonnect-logging`.
- `ticonnect-net`, `ticonnect-media`, and `ticonnect-desktop` depend on
  `ticonnect-core`.
- `ticonnect-daemon` composes the service crates.
- `ticonnect-ui`, `ticonnect-overlay`, and `ticonnect-cli` never depend on
  daemon internals.
- `ticonnect-settings` never depends on `ticonnect-logging`.

## Errors

- Each fallible library crate owns its own `Error` and `Result` types.
- Dependency failures are mapped into the owning crate's error type.
- `ticonnect-core` exposes domain types only. Fallible core modules define local
  errors when they need them.
- There is no workspace-wide catch-all error enum.
- Only binary entry points use `anyhow`, adding context around startup steps.

## Interim composition

Each binary currently loads `Settings`, builds `LogSettings`, and initializes
`Logging` directly. This glue is duplicated across the four entrypoints on
purpose. A future `ticonnect-runtime` crate will own settings, logging, process
lifecycle, and daemon wiring.
