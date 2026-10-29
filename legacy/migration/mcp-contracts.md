# Rust MCP contract verification

The Rust `fubbik mcp` command serves the retained 53-tool catalog over stdio. The migration checker compares the Rust tool declarations with `executableContracts.mcpTools.expected` in `legacy/migration/rust-migration.json`; the MCP crate also checks the live catalog against that manifest, including unique names and object input schemas.

`cargo test -p fubbik-mcp` exercises initialization and representative tool calls across chunks, context snapshots, requirements, quick tasks, plans, matrices, and coordination against mock HTTP responses. `cargo test -p fubbik --test cli mcp_stdio` starts the shipped binary and verifies newline-delimited JSON-RPC responses for initialization, tool listing, unknown methods, malformed JSON, and a notification that must not produce a response.

These tests verify the retained catalog and Rust stdio protocol. They do not assert byte-for-byte equivalence of every tool's formatted text with the legacy TypeScript implementation; the migration inventory and focused tool contract tests remain the compatibility boundary for those results.
