# Stage 1.5 — Architecture contracts and graph

## Goal

Make component boundaries and relations explicit before daemon, storage, and
transport code lands. Provide a canonical, machine-checked architecture graph and
generated system, component, build, runtime, trust, state, and state-machine
views.

## Scope

- Canonical graph data under `docs/systems/graph/`.
- Canonical state machines in `docs/systems/state-machines.toml`.
- Generated system docs and graph documents.
- `xtask systems` generator and validator.
- Architecture CI check.
- Roadmap stage dependencies reconciled with the graph.

## Non-goals

- No product code changes.
- No new runtime feature.
- No dependency changes outside the dev-only `xtask`.
- No automatic code generation for product modules.

## Prerequisites

Stage 1 complete.

## Deliverables

- `docs/systems/README.md`
- `docs/systems/graph/`
- `docs/systems/state-machines.toml`
- `docs/systems/generated/`
- `docs/systems/01-runtime.md` through `24-assurance.md`
- `xtask/`
- `.cargo/config.toml` with the `xtask` alias
- CI architecture step

## Implementation

- [x] Add `xtask` workspace member and `cargo xtask` alias.
- [x] Define the graph fragment schema and loader.
- [x] Populate systems, components, stores, edges, invariants, build deps, and
      data-driven build rules.
- [x] Add state machines for pairing, outbox, transfer, call, and remote desktop.
- [x] Generate a global system graph and per-system component graphs.
- [x] Generate separate build, runtime, trust, state, registry, and stage views.
- [x] Generate narrative system contract docs from canonical metadata.
- [x] Validate duplicate IDs, unknown nodes, store ownership, gates, stages,
      build dependencies, build cycles, runtime cycles, and state machines.
- [x] Add stale-output detection with `cargo xtask systems --check`.
- [x] Document edit and regeneration workflow.

## Tests and checks

- [x] `cargo xtask systems` succeeds.
- [x] `cargo xtask systems --check` succeeds.
- [x] Build dependency graph matches `cargo metadata`.
- [x] No forbidden dependency direction edge exists.
- [x] Every store has exactly one owner.
- [x] Every component has a contract and test seam.
- [x] Every invariant has a test.
- [x] Every runtime cycle uses only async or event edges.

## Security

- [x] No secret or key material is added to graph data.
- [x] Trust and consent gates are explicit graph edges.
- [x] The graph records that discovery is not trust.

## Docs and acceptance

- [x] `docs/systems/README.md` explains precedence, conventions, and commands.
- [x] Generated docs carry a generated header.
- [x] Generated docs are committed and checked in CI.
- [x] Stage docs point to component IDs and generated graphs.

## Acceptance criteria

- `cargo xtask systems --check` passes.
- `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets --
  -D warnings`, and `cargo test --workspace --all-targets` pass.
- Every system has a component graph.
- Every cross-system edge is visible in both the system view and the edge table.
- No stage depends on a later stage in its demo path.

## Demo

1. Show `docs/systems/generated/systems.md` and a per-system component graph.
2. Run `cargo xtask systems --check`.
3. Change a graph edge locally, show the check fails, then regenerate.
4. Show the build dependency graph and the trust gate graph.

## Risks and mitigations

- Risk: graph data becomes a second source of truth for prose. Mitigation:
  graph owns structure; system docs are generated from it.
- Risk: graph rot. Mitigation: CI stale-output check.
- Risk: over-decomposition. Mitigation: component rule in `docs/systems/README.md`.

## Works without later stages

The graph and generated docs build and validate with no daemon, storage,
discovery, transport, messaging, or UI code present.
