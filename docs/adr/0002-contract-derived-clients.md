# ADR 0002: Client contracts derive from OpenAPI

- Status: Accepted
- Date: 2026-09-12

## Context

The TypeScript client derives types from `openapi.json`, while the Rust CLI
currently maintains hand-written response models. Route registration and the
central OpenAPI declaration can also drift independently.

## Decision

OpenAPI is the protocol seam for first-party network adapters. Both TypeScript
and Rust client contracts derive from it, and automated checks fail when the
committed artifacts are stale. Feature registration must make discrepancies
between routable and documented paths observable in tests.

## Consequences

- Transport adapters may keep ergonomic, domain-specific helpers.
- Hand-written duplicate wire models are migrated incrementally.
- Generator output is reviewed and committed reproducibly.

