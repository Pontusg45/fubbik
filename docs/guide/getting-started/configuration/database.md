---
tags:
  - guide
  - configuration
  - database
description: PostgreSQL setup and required extensions
---

# Database Setup

Fubbik uses PostgreSQL with two extensions:

- **pgvector** (0.8.2) — 768-dimensional vector embeddings for semantic search
- **pg_trgm** — trigram-based fuzzy text search

## Docker (Recommended)

The included `docker-compose.yml` sets up PostgreSQL with both extensions pre-installed:

```bash
docker compose up -d db
```

## Schema Management

Schema is defined in TypeScript at `packages/db/src/schema/`. Push schema changes with:

```bash
pnpm db:push
```

Open the database UI for inspection:

```bash
pnpm db:studio
```

## Seed Data

Populate with sample data for development:

```bash
pnpm seed
```

This creates: 1 dev user, 24 chunks, 36 connections, 5 tag types, 40 tags, 1 codebase, 2 use cases, 3 requirements, 1 plan, and more.
