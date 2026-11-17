# AGENTS.md

`CODING_STYLE.md` is normative for this package. Read it before changing code.

## Checks

Run the formatter and every check before finishing a task or committing:

```sh
bun run format
bun run format:check
bun run lint
bun run check
bun run test
bun run build
```

From the repository root:

```sh
make web-check
```

Fix formatting with `bun run format`, then re-run lint, check, and test.
