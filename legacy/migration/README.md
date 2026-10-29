# Rust migration

The migration is complete when the backend, CLI, MCP server, database maintenance commands, and production web server run without Node or Bun. TypeScript remains the source language for the browser application and VS Code extension; their build toolchains are outside the runtime cutover.

[`rust-migration.json`](./rust-migration.json) is the machine-readable source of truth. Every legacy backend route, CLI command module, and MCP source module must appear in exactly one status group. `node legacy/migration/scripts/check-rust-migration.mjs` fails when a legacy source is added, removed, or listed twice without updating the contract. It reports source inventory separately from runtime cutover: inventory parity alone does not mean the application runs without Node or Bun.

Each cutover gate has a stable ID, `pending` or `complete` status, an evidence path, and a verification command. A completed gate with missing evidence fails validation. Mark a gate complete only after running its verification against the current implementation. The checker reports outstanding gates by name; it does not execute long-running integration or container checks automatically.

## Execution order

1. **Lock the contract.** Keep route, command, MCP, and runtime dependency inventories exhaustive in CI. Record removals with a reason.
2. **Close HTTP parity.** Maintain contract tests for every retained endpoint. Remove the unused code-index, concepts, diagram, and placeholder usage domains after caller scans remain empty.
3. **Complete CLI parity.** Port commands in vertical slices through the existing typed client and output modes. Preserve command names, aliases, JSON shapes, quiet output, and exit behavior.
4. **Complete native source extraction.** The CLI no longer starts Node, JSDoc, TypeDoc, or a JVM. Expand the native parsers beyond their documented declaration subsets and keep differential fixtures for stable `SourceManifest` symbols.
5. **Maintain MCP parity.** Keep the retained 53-tool catalog aligned with the migration manifest and run the stdio protocol and HTTP dispatch tests when tools change.
6. **Move operational commands.** Port seed, scan, and verification workflows to Rust and update local and container entrypoints.
7. **Serve the web build from Rust.** Embed or serve the SPA assets with history fallback, cache policy, and a production image smoke test that confirms Node and Bun are absent.
8. **Cut over and delete.** Remove legacy runtime packages from the workspace, containers, documentation, and CI only after all earlier gates pass.
9. **Ship one artifact.** Produce release binaries and container images, verify shell completions, upgrade behavior, graceful shutdown, and supported-platform installation.

## Required checks

Run `just rust-test` for the canonical database adapter, formatting, strict
Clippy, migration checks, and the full Rust workspace suite. It reports
aggregate test totals and stops the database it started, including on failure.
`just rust-test-fast` runs database-free library and CLI HTTP contract checks
during development.
For an already running test database, set `FUBBIK_TEST_DB_EXTERNAL=true` and
`DATABASE_URL` when invoking `just rust-test`.

Run the HTTP differential suite and production container smoke test before deleting any legacy implementation. The JSON contract's `cutoverGates` array is the definitive removal checklist.
