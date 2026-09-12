# ADR 0001: Aggregate writes are atomic

- Status: Accepted
- Date: 2026-09-12

## Context

Chunk and Document mutations update a primary row plus metadata, history, and
relationships. Independent pool calls can preserve early writes when a later
write fails.

## Decision

Every mutation that changes one logical Chunk or Document aggregate runs in a
single PostgreSQL transaction. Transaction handling stays inside the deepest
domain service that understands the complete mutation; HTTP routes do not
assemble transactions. Repository operations used there expose
transaction-aware forms without making ordinary callers manage a transaction.

Tests exercise the public route or domain interface and assert that failed
mutations leave no partial observable state.

## Consequences

- Callers receive all-or-nothing behavior.
- Repository operations used by aggregate mutations must accept the active
  transaction internally.
- Applied migrations remain immutable.
