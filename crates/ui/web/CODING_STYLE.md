# Coding style — TiConnect web

Normative for TypeScript, JavaScript, and Svelte under `crates/ui/web`. Ported
from the TiCrafter TypeScript style and adapted to a browser UI package.

## 1. Result pattern

Use the Result tuple from `src/lib/result.ts`:

```ts
export type Result<T, E = unknown> = [value: T | null, err: E | null];
```

Return `Result` for expected failures: validation, parsing, guards, bridge
requests, and protocol replies.

```ts
const [bootstrap, bootstrapFailure] = await client.request<Bootstrap>('bootstrap');
if (bootstrapFailure) {
    return bootstrapFailure.message;
}
if (bootstrap === null) {
    return 'bootstrap response was empty';
}

return bootstrap.version;
```

- Destructure as `const [value, err] = ...`.
- Check `err` first and early-return.
- Never read `value` when `err` is non-null.
- Type `E` narrowly, usually a discriminated failure union.
- Keep the destructure, the `err` check, and the `null` check adjacent, with no
  blank lines between them.

## 2. Exceptions only at the boundary

- Do not write `try-catch` or `throw` for domain failures or control flow.
- At throwing boundaries (`JSON.parse`, `WebSocket`, DOM APIs, network calls),
  use `attempt` or `attemptAsync` from `src/lib/result.ts`.
- The only allowed `try` lives inside `src/lib/result.ts`.
- The only allowed `throw` lives inside `src/lib/assert.ts` for
  `assertNever(value: never)`.
- Never swallow errors. Convert them into a typed failure.

## 3. Formatting

Prettier is the source of truth. `.prettierrc.json` is normative:

- `semi: true`
- `singleQuote: true`
- `tabWidth: 4`
- `printWidth: 100`
- `trailingComma: all`
- `endOfLine: lf`

Run the formatter before finishing a task:

```sh
bun run format
bun run format:check
```

Do not hand-format. Do not add `prettier-ignore` unless generated output
requires it, and justify it when used.

## 4. Naming

- Use complete, descriptive names for variables, parameters, methods, types,
  and constants.
- Never use single-letter or cryptic identifiers. Banned examples: `v`, `l`,
  `e`, `i`, `n`, `p`, `ctx`, `req`, `res`, `msg`, `obj`, `fn`, `cb`, `idx`,
  `len`, `tmp`, `val`, `dir`, `src`, `dst`.
- Prefer full words: `database` not `db`, `environment` not `env`,
  `currentWorkingDirectory` not `cwd`, `context` not `ctx`, `message` not
  `msg`.
- Loop variables use `index`, `eventIndex`, `requestIndex`, and similar names.
- Test fixtures use descriptive keys.
- Exceptions: coordinate axes `x`, `y`, `z`; standard acronyms `id`, `url`,
  `apiKey`; and `value`/`err` in the Result tuple.
- Wire-format fields from the daemon protocol keep their exact names, including
  `snake_case` fields such as `node_id`, `display_name`, `transport_state`, and
  `peer_count`. Do not translate them at the transport boundary.

## 5. Airtight TypeScript

- Use `satisfies` for configs, registries, and maps.
- Give objects fully typed fields; do not rely on widened inference.
- Model failures as discriminated unions, not loose strings or `any`.
- Do not use `any`. Narrow `unknown` with type guards.
- Do not use `!` non-null assertions.
- Prefer exhaustive `switch` matching over `if` / `else if` chains for union
  kinds. Close with `assertNever` when the union is closed.
- No silent fallbacks. Fail closed with a `Result`.

## 6. Imports and facades

- `src/lib/index.ts` is the module facade. Code outside `src/lib` imports from
  `src/lib`, not from implementation files inside it.
- Files inside the same directory may import each other directly.
- File-local helpers stay unexported.
- Keep `simple-import-sort` order. Local imports run before parent imports and
  side-effect imports.
- Vite resolves extensionless relative imports in this package.

## 7. No code comments

- Do not write `//`, `/* */`, `/** */`, or Svelte/HTML comments in code.
- Code must be self-explanatory through names, small methods, and types.
- Markdown documentation may use prose.

## 8. Low complexity, flat indentation

- Keep one method focused on one decision family.
- Never nest deeper than two levels; prefer one.
- Use guard clauses and early returns.
- Do not use `else if`; use `switch` for union kinds and early returns for
  booleans.
- Avoid `else` after `return` or `continue`.
- Extract nested iteration into a helper.
- Prefer `.map`, `.filter`, `.find`, `.reduce`, `.forEach`, `.some`, and
  `.every` over loops for collection processing.
- Never mutate inputs.
- Prefer files under 150 lines; split a file that outgrows that target.

## 9. Svelte

- Use Svelte 5 runes: `$state`, `$derived`, `$effect`.
- Keep components thin. Protocol and state transitions belong in `src/lib`.
- Use `onMount` cleanup for listeners, sockets, and timers.
- Do not add logic to markup that belongs in a class or helper.
- Template markup may use double quotes for attributes even when TypeScript
  uses single quotes.

## 10. Centralize failure payloads

Failure unions must not be constructed with inline discriminant literals at
call sites.

Forbidden:

```ts
return [null, { type: 'NOT_CONNECTED', message: 'bridge is not connected' }];
```

Required:

```ts
return [null, BridgeFailures.notConnected()];
```

- The factory owns the discriminant and repeated messages.
- Name factory methods after the domain condition, not after the literal.
- Do not add a generic `createFailure(type, payload)` helper.
- Add colocated factory tests for exact `type` and `message` payloads.
- A new failure variant requires a union arm, a factory method, and a test in
  the same change.

## 11. Toolchain and tests

- Bun is the package manager, runner, and test runner.
- Required checks:

```sh
bun run format:check
bun run lint
bun run check
bun run test
bun run build
```

- Colocate unit tests as `*.test.ts` next to the unit.
- New logic needs a test: guards, failure factories, parsing, and Result flow.
- Use Vitest (`describe`, `it`, `expect`).
- The repository-level `make web-check` runs the web checks from the root.

## Exceptions

- Rust code is governed by `cargo fmt`, `cargo clippy`, and the repository Rust
  conventions. This document applies only to `crates/ui/web`.
- TiCrafter DDD, schema-ownership, SQLite, agency, and Minecraft rules do not
  apply here.
- The daemon wire protocol is an external contract. Keep its field names,
  including `snake_case`, exactly as defined by the daemon.
- Triple-slash compiler directives in `src/vite-env.d.ts` are configuration,
  not code comments, and are exempt from the no-comments rule.
