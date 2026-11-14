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
- `ticonnect-daemon` composes the service crates and exposes its library API.
- `ticonnect-runtime` owns settings/logging bootstrap, daemon state composition,
  supervision, and the daemon entry point. It depends on `ticonnect-daemon`.
- `ticonnect-ui`, `ticonnect-overlay`, and `ticonnect-cli` depend on runtime for
  bootstrap and never import daemon internals directly.
- `ticonnect` is the unified binary crate. It depends on runtime, UI, overlay,
  and CLI and only dispatches subcommands.
- `ticonnect-settings` never depends on `ticonnect-logging`.

## Errors

- Each fallible library crate owns its own `Error` and `Result` types.
- Dependency failures are mapped into the owning crate's error type.
- `ticonnect-core` exposes domain types only. Fallible core modules define local
  errors when they need them.
- There is no workspace-wide catch-all error enum.
- Only binary entry points use `anyhow`, adding context around startup steps.

## Composition

`ticonnect-runtime` is the composition root. It loads `Settings`, initializes
`Logging`, composes identity and database state, and supervises the daemon
lifecycle. Binaries use runtime bootstrap instead of duplicating settings and
logging wiring.
