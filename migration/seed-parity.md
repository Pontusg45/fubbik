# Native seed parity

The Rust seed runs all 16 legacy modules for `demo` and `extended`, and the dependency-closed minimal scenario. The database suite covers repeat runs, `--only`, `--skip`, `--reset=none`, automatic reset, named foreign keys, and transaction rollback.

To reproduce the differential check, create two fresh PostgreSQL 18 databases with the same migrations. Run the legacy seed against one and `fubbik seed --scenario demo` against the other. Then set `LEGACY_DATABASE_URL` and `RUST_DATABASE_URL` and run:

```sh
node scripts/compare-seed-parity.mjs
```

On 2026-09-26, the check passed for 33 table counts and four named relationship sets: chunk connections, chunk tags, requirement-to-chunk links, and workspace-to-space links. It was also rerun after a second native seed with automatic reset and passed again. The six self-documenting chunks retain their legacy titles and graph relationships while their bodies describe the Rust implementation.

The production seed workflow remains a separate cutover gate until the Bun seed image and Compose command are replaced.
