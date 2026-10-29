# Legacy

This folder holds the retired Node/Bun backend (`server/`, `api/`, `auth/`) and the Node → Rust migration evidence (`migration/`). Nothing in the running product depends on it.

Three live checks still read it: `crates/fubbik-mcp` embeds `legacy/migration/rust-migration.json` in one test, `scripts/test/test-rust.sh` runs the parity checks in `legacy/migration/scripts/`, and the `check:rust-migration` and `test:rust-migration` scripts in the root `package.json` point at `legacy/migration/scripts/`. Remove all three before deleting this folder.

Forks can delete the folder once they've done that.
