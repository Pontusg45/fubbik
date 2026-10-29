# Legacy

This folder holds the retired TypeScript code — the Node/Bun backend (`server/`, `api/`, `auth/`), the CLI (`cli/`) and the MCP server (`mcp/`) — and the Node → Rust migration evidence (`migration/`). Nothing in the running product depends on it. The Rust `fubbik` binary replaces the CLI, and `fubbik mcp` replaces the MCP server.

Two live checks read it, and both skip when the folder is absent: the `tool_catalog_matches_the_migration_contract` test in `crates/fubbik-mcp` reads `legacy/migration/rust-migration.json` at runtime, and `scripts/test/test-rust.sh` only runs the parity checks in `legacy/migration/scripts/` when that folder exists. CI's `legacy-removable` job proves the rest of the repo builds and type-checks without it.

To delete this folder, a fork only needs to remove the `check:rust-migration` and `test:rust-migration` scripts from the root `package.json`, which point at `legacy/migration/scripts/`.
