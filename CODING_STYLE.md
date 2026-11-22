# Coding style — TiConnect

Normative for Rust code in this repository. The web package keeps its own rules
in `crates/ui/web/CODING_STYLE.md`.

## Conversion errors

Never discard a failed conversion. `TryFrom`, `FromStr`, and numeric narrowing
must propagate a typed error with `?` or an explicit `match`.

Forbidden on data crossing a boundary:

- `.ok()` followed by a fallback value.
- `.unwrap_or_default()` or `.unwrap_or(...)` after `try_from`.
- `as` casts that narrow, change signedness, or change units.
- Enum fallbacks that turn an unknown wire value into a valid domain value.

Optional fields may default only when absent. A present but wrong-typed or
out-of-range value is an error. Distinguish missing data from invalid data.

Saturating conversions are allowed only for deliberate resource bounds, never
for protocol, persistence, or IPC data.

## Fail fast

Prefer failing fast with a typed error over falling back after an error was
detected. A fallback that hides the failure creates silent errors and uncertain
behavior, especially across persistence, IPC, and media boundaries.

- Return the error to the caller instead of guessing a replacement value.
- If a fallback is truly required, make it explicit, observable, and covered by
  a test; document why the fallback is safe.
- Do not continue with partial or stale state after a failed operation.

## Checks

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```
