# Legacy

This folder holds the retired TypeScript code — the Node/Bun backend (`server/`, `api/`, `auth/`), the CLI (`cli/`), the MCP server (`mcp/`) and the Node database seed (`db-seed/`, moved from `packages/db/src/seed`; archived and not runnable, since its relative imports point outside `legacy/`) — and the Node → Rust migration evidence (`migration/`). Nothing in the running product depends on it. The Rust `fubbik` binary replaces the CLI, and `fubbik mcp` replaces the MCP server.

A few dev-time checks and scripts still read it.

These skip automatically when the folder is absent:

- the `tool_catalog_matches_the_migration_contract` test in `crates/fubbik-mcp`, which reads `legacy/migration/rust-migration.json` at runtime
- `scripts/test/test-rust.sh`, which only runs the parity checks in `legacy/migration/scripts/` when that folder exists

A fork removes these by hand when deleting this folder:

- the `check:rust-migration` and `test:rust-migration` scripts in the root `package.json`, which point at `legacy/migration/scripts/`

CI's `legacy-removable` job proves the rest of the repo builds and type-checks without this folder.
