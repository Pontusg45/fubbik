# Rust migration

The migration is complete when the backend, CLI, MCP server, database maintenance commands, and production web server run without Node or Bun. TypeScript remains the source language for the browser application and VS Code extension; their build toolchains are outside the runtime cutover.

[`rust-migration.json`](./rust-migration.json) is the machine-readable source of truth. Every legacy backend route, CLI command module, and MCP source module must appear in exactly one status group. `node scripts/check-rust-migration.mjs` fails when a legacy source is added, removed, or listed twice without updating the contract.

## Execution order

1. **Lock the contract.** Keep route, command, MCP, and runtime dependency inventories exhaustive in CI. Record removals with a reason.
2. **Close HTTP parity.** Maintain contract tests for every retained endpoint. Remove the unused code-index, concepts, diagram, and placeholder usage domains after caller scans remain empty.
3. **Complete CLI parity.** Port commands in vertical slices through the existing typed client and output modes. Preserve command names, aliases, JSON shapes, quiet output, and exit behavior.
4. **Remove extractor subprocesses.** Replace the Node/JSDoc/TypeDoc/JVM source-document path with native parsers behind the versioned `SourceManifest` boundary.
5. **Complete MCP parity.** Expand `fubbik-mcp` from the core tool catalog to context, plans, requirements, tasks, coordination, and matrices. Each slice needs schema comparison, stdio protocol tests, and HTTP dispatch tests.
6. **Move operational commands.** Port seed, scan, and verification workflows to Rust and update local and container entrypoints.
7. **Serve the web build from Rust.** Embed or serve the SPA assets with history fallback, cache policy, and a production image smoke test that confirms Node and Bun are absent.
8. **Cut over and delete.** Remove legacy runtime packages from the workspace, containers, documentation, and CI only after all earlier gates pass.
9. **Ship one artifact.** Produce release binaries and container images, verify shell completions, upgrade behavior, graceful shutdown, and supported-platform installation.

## Required checks

```sh
node scripts/check-rust-migration.mjs
node --test scripts/check-rust-migration.test.mjs
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test --workspace
```

Run the HTTP differential suite and production container smoke test before deleting any legacy implementation. The JSON contract's `cutoverGates` array is the definitive removal checklist.
