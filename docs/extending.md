# Extending fubbik

What you can build on, and what may move under you. If you track upstream, stay on the stable list.

## Stable extension points

### Add a review check

The `review_harness::Check` trait and the `review-rules.toml` format are stable. Most rules are one regex: add a `[[rule]]` table to
`review-rules.toml`. For anything a pattern cannot express, implement `Check` in `crates/review-harness/src/checks/`, add the module, and
list it in `all()` in `crates/review-harness/src/checks/mod.rs`. Details are in
[`crates/review-harness/README.md`](../crates/review-harness/README.md).

### Add an `fk` subcommand

`fk` subcommands are stable. In `crates/kit`:

1. Add a module under `crates/kit/src/` and declare it in `crates/kit/src/lib.rs`. Return a serializable outcome so `--json` works, and
   make any destructive action opt-in or confirmed, with a `--dry-run` where it makes sense.
2. Add a variant to the `Command` enum in `crates/kit/src/main.rs` and a match arm in `run`.
3. Add tests under `crates/kit/tests/`.

### Add a feature group to `fubbik.toml`

Feature switches are stable. To add a switchable group:

1. Add a variant to `Feature`, to `Feature::ALL` and to `key()` (and `requires()` if it depends on another) in
   `crates/fubbik-config/src/lib.rs`.
2. Wrap the group's routers in an `if switches.enabled(Feature::...)` block in `router()` in `crates/fubbik-api/src/lib.rs`. Gate its
   background jobs the same way.
3. Document the key in `fubbik.example.toml`.
4. Add the key to `Feature` and to `FEATURE_BY_ROUTE` in `apps/web/src/features/nav/use-capabilities.ts` so the nav hides it. Gate any
   query a core page makes to the group with `useFeatureEnabled` from the same file, skipping only when it returns `false`.

The MCP tools in `crates/fubbik-mcp` and the `fubbik plan …` / `fubbik matrix …` CLI commands do not consult the switches: when a group is
off they still call the API and get a 404.

The database schema stays complete whatever the switches say; do not make migrations conditional.

### Add an MCP tool

Tools in `crates/fubbik-mcp` are stable. Grouped tools live in `crates/fubbik-mcp/src/tools/` (`plan.rs`, `matrix.rs`, `coordination.rs`):
add the name to the group's `NAMES`, a case in its `call`, and an entry in its `definitions()`. A new group needs `pub(crate) mod` in
`tools/mod.rs`, a `handles`/`call` branch in `crates/fubbik-mcp/src/lib.rs`, and its `definitions()` added where the tool list is built. The
tool should call the HTTP API, not the database.

### Add chunk templates

Built-in templates are seeded from `builtin_templates` in `crates/fubbik-db/src/seed/core-catalog.json` and are read-only. Edit that list to
change the defaults for a fork (a test in `crates/fubbik-db/src/seed/core.rs` asserts the count, so update it). Users can also create their
own at `/templates` or `POST /api/templates`.

### Add an API route module

Route modules are stable in shape: a directory under `crates/fubbik-api/src/<name>/` with `routes.rs` (and a service/DTO layer as its
neighbours do) exposing `router()`, merged in `router()` in `crates/fubbik-api/src/lib.rs`. Register new handlers in
`crates/fubbik-api/src/openapi.rs`. Put it in the always-on block, or behind a feature switch as described above.

## Everything else may change

Internal module layout, database schema details, the web app's components, seed catalogs, the VS Code extension, and function signatures
inside crates can change without notice. Pin to a commit if you depend on any of them, and expect to resolve merge conflicts when you pull
upstream.
