# Contributing to PIDX

## Prerequisites

- Rust stable (`rustup update stable`)
- `cargo` (included with Rust)

## Build

```bash
cargo build --workspace
```

For the MCP server only:

```bash
cargo build -p pidx-mcp
```

## Test

```bash
cargo test --workspace
```

Tests live in `src/ingestion.rs` and `src/models/observation.rs`. All 9 tests should pass on both Windows and Linux.

## Lint

```bash
cargo clippy --workspace -- -D warnings
```

## Schema change protocol

The profile schema is defined in `src/models/`. Follow this sequence when adding or changing fields:

1. **Bump `schema_version`** in `ProfileMeta::new()` (e.g. `"0.1.0"` → `"0.2.0"`)
2. **Add `#[serde(default)]`** to all new fields — never remove or rename existing fields
3. **Run `cargo build --workspace`** — the compiler will flag every match site that needs updating
4. **Regenerate the machine-readable schema:** `cargo run --example emit_schema > docs/pidx-schema.json`
5. **Update `docs/pidx-schema-spec.md`** version header to match
6. **Update `SKILL.md`** version reference line at the top

Steps 5–6 and the regenerated `docs/pidx-schema.json` should all land in the same commit. CI enforces schema drift: it regenerates the schema and diffs against the committed file.

Removing a field from the JSON schema breaks existing profiles on disk. Instead, deprecate by marking with `#[deprecated]` and excluding from output renders.

## Pull request guidelines

- Keep PRs small and focused; link to `docs/pidx-schema-spec.md` if touching models
- If your change adds a new field, include a `#[serde(default)]` and update the schema spec
- Run `cargo test --workspace && cargo clippy --workspace -- -D warnings` before opening a PR
