# Chunk Retrieval Enhancement Design

## Goal

Improve the chunk data model for AI agent consumption by adding retrieval-enhancing metadata fields, local embeddings via Ollama, and
semantic search.

## Context

The current chunk schema has: id, title, content, type, tags, userId, timestamps. Search is trigram-based (keyword matching only). AI agents
need better ways to find, filter, and disambiguate chunks.

## Design Decisions

- Primary consumer: AI agents (CLI, external API callers)
- All new fields on the chunk table directly (no separate metadata table, no JSONB blob) for query simplicity
- Ollama for local embeddings (nomic-embed-text, 768 dimensions) — no API cost
- AI-suggested metadata on save, manually overridable
- Enrichment is optional — everything works without Ollama

---

## Schema Changes

New columns on `chunk` table:

| Column    | Type                           | Default | Description                                                  |
| --------- | ------------------------------ | ------- | ------------------------------------------------------------ |
| summary   | text                           | null    | 1-2 sentence AI-generated TL;DR                              |
| aliases   | jsonb (string[])               | []      | Alternative names, abbreviations, search terms               |
| not_about | jsonb (string[])               | []      | Exclusion terms for disambiguation                           |
| scope     | jsonb (Record<string, string>) | {}      | Key-value pairs, e.g. {"package": "api", "layer": "backend"} |
| embedding | vector(768)                    | null    | nomic-embed-text embedding vector                            |

### Indexes

- GIN on `aliases` — containment queries (`@>`)
- GIN on `not_about` — exclusion filtering
- GIN on `scope` — key-value filtering
- HNSW on `embedding` — approximate nearest neighbor (cosine distance)

### pgvector

- `CREATE EXTENSION IF NOT EXISTS vector`
- Railway Postgres supports pgvector natively
- Local dev: standard with Postgres 16+

---

## AI Enrichment Pipeline

### What gets generated

- `summary` — 1-2 sentence TL;DR
- `aliases` — 3-8 alternative names/terms
- `not_about` — 2-5 exclusion terms (what the chunk could be confused with)
- `embedding` — vector from nomic-embed-text on concatenated title + summary + content

### When it runs

- On create (POST /api/chunks) — fire-and-forget after response
- On update (PATCH /api/chunks/:id) — regenerate if title or content changed
- On explicit trigger (POST /api/chunks/:id/enrich) — always overwrites
- Seed script enriches all chunks if Ollama available

### Manual override

- PATCH with explicit summary/aliases/notAbout preserves user values
- Auto-generation only fills null/empty fields
- /enrich endpoint always overwrites

### Configuration

- OLLAMA_URL env var (default: http://localhost:11434)
- Enrichment silently skipped if Ollama unreachable
- Embedding model: nomic-embed-text (hardcoded)
- Generation model: configurable, defaults to available model

---

## Query & Retrieval API

### New query params on GET /api/chunks

| Param   | Type                          | Description                                     |
| ------- | ----------------------------- | ----------------------------------------------- |
| exclude | string (comma-separated)      | Filter out chunks with matching not_about terms |
| scope   | string (key:value, comma-sep) | Filter by scope pairs                           |
| alias   | string                        | Match against aliases array                     |

### New endpoint: GET /api/chunks/search/semantic

Semantic search via embedding similarity:

- `q` (required) — natural language query
- `limit` — max results (default 5)
- `exclude`, `scope` — same filters as list endpoint
- Returns chunks ordered by cosine similarity with score

Flow:

1. Embed query via Ollama nomic-embed-text (with "search_query:" prefix)
2. pgvector `<=>` cosine distance query
3. Apply scope/exclude filters
4. Return chunks with similarity score

### Response changes

All chunk responses include summary, aliases, notAbout, scope. Embedding vector is NOT returned (too large, server-side only).

---

## CLI Additions

- `fubbik search --semantic` — uses semantic search endpoint
- `fubbik list --scope <key:value>` — scope filtering
- `fubbik list --exclude <terms>` — exclusion filtering
- `fubbik enrich [id]` — trigger enrichment for one chunk
- `fubbik enrich --all` — backfill enrichment for all chunks

---

## Migration & Backfill

1. Enable pgvector extension
2. Add columns with nullable/default values (non-breaking)
3. Add GIN indexes
4. Add HNSW index on embedding
5. POST /api/chunks/enrich-all for backfill (sequential, don't overwhelm Ollama)
6. Seed script calls enrich for seeded chunks

---

## Ollama Setup (Dev)

```
ollama pull nomic-embed-text
```

No Ollama = no enrichment, no semantic search. Everything else works.
