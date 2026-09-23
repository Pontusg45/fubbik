---
tags:
    - guide
    - architecture
    - database
description: Database architecture — PostgreSQL schema, extensions, and SQLx migrations
---

# Database Architecture

PostgreSQL is the source of truth. The live schema is managed by SQLx migrations in `crates/fubbik-db/migrations/`; Rust repositories live in `crates/fubbik-db/src/repo/`. The Drizzle schema in `packages/db` remains as a legacy reference and seed adapter.

## In This Section

- [Schema Overview](./schema.md) — core tables and relationships
- [Extensions](./extensions.md) — pgvector and pg_trgm
