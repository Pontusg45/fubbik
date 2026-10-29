# Legacy

This folder holds the retired Node/Bun backend (`server/`, `api/`, `auth/`) and the Node → Rust migration evidence (`migration/`). Nothing in the running product depends on it.

Two live checks still read it: `crates/fubbik-mcp` embeds `legacy/migration/rust-migration.json` in one test, and `scripts/test/test-rust.sh` runs the parity checks in `legacy/migration/scripts/`. Remove both before deleting this folder.

Forks can delete the folder once they've done that.
