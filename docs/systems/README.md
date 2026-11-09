# System contracts and graphs

This directory is the source of truth for component boundaries and relations.

- `graph/` is canonical. Edit TOML there, never the generated files.
- `state-machines.toml` is canonical for state machines.
- `generated/` is generated. Do not edit by hand.
- `01-runtime.md` through `24-assurance.md` are generated from `graph/`.

## Precedence

- `main.md` owns product intent and constraints.
- `architecture.md` owns stack and architectural decisions.
- `docs/systems/` owns component contracts and relations.
- `roadmap/` owns delivery order and stage scope.
- `docs/dependency-direction.md` owns build-time dependency law.

## Component rule

A component is smallest when it has one responsibility, one owned state or one
pure transformation, an independently testable contract, and one writer for
owned state. If splitting it creates no new contract, it is a function.

## Relation kinds

`owns`, `calls`, `invokes`, `emits`, `consumes`, `persists`, `config-read`,
`gates`, `speaks`, `observes`, `build-dep`.

## Modes

`sync`, `async`, `event`, `build`.

Build graphs must be acyclic. Runtime cycles are allowed only through `async`
or `event` edges. Every persistent store has exactly one `owns` edge. Forbidden
build dependencies live in `graph/build-rules.toml`; every real workspace
dependency must also be declared in `graph/build-deps.toml`.

## Graph views

- `generated/systems.md` — system graph and aggregated system edges.
- `generated/runtime.md` — cross-system runtime edges.
- `generated/build-deps.md` — Cargo build graph checked against `cargo metadata`.
- `generated/trust.md` — trust, policy, and consent gates.
- `generated/state.md` — state ownership.
- `generated/components/<system>.md` — inner-system component graph.
- `generated/state-machines/<id>.md` — state machine graph and transition table.
- `generated/registry.md` — full component registry.
- `generated/stage-map.md` — component to stage mapping.

## Commands

```sh
cargo xtask systems
cargo xtask systems --check
```

`cargo xtask systems` regenerates all system docs, registry, graphs, and state
machine docs. `--check` fails when generated output is stale or a relation
violates the rules.

## Updating contracts

1. Edit the relevant file under `graph/` or `state-machines.toml`.
2. Run `cargo xtask systems`.
3. Review generated diffs.
4. Run `cargo xtask systems --check` before committing.
