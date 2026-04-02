---
tags:
  - guide
  - ai
  - ollama
description: AI-powered features including enrichment, semantic search, and context export
---

# AI Features

Fubbik uses local AI (via Ollama) for enrichment, semantic search, and intelligent context delivery. All AI features are optional — everything else works without them.

## Setup

Install Ollama and pull the required models:

```bash
# Install Ollama (macOS)
brew install ollama

# Start Ollama
ollama serve

# Pull models
ollama pull nomic-embed-text   # For embeddings (768-dim vectors)
ollama pull llama3.2           # For text generation (enrichment)
```

Configure the URL in your environment:
```
OLLAMA_URL=http://localhost:11434
```

## Chunk Enrichment

When you create or edit a chunk, fubbik can auto-generate:

- **Summary** — A one-line description of the chunk
- **Aliases** — Alternative names the chunk might be known by
- **Not About** — Terms this chunk is explicitly NOT about (improves search precision)

Enrichment runs automatically on create/edit when Ollama is available. To manually enrich:

```bash
# Enrich a single chunk
fubbik enrich <id>

# Enrich all un-enriched chunks
fubbik enrich --all
```

In the web UI, click the "Enrich" button on any chunk's detail page.

## Vector Embeddings

Each chunk gets a 768-dimensional vector embedding using `nomic-embed-text`. These power semantic search — finding chunks by meaning rather than exact keyword matches.

Embeddings are generated:
- Automatically when a chunk is created or its content changes
- In bulk via `fubbik enrich --all`
- On demand via the enrich button

The `embeddingUpdatedAt` field tracks when each chunk's embedding was last refreshed. The health dashboard flags chunks with stale embeddings.

## Semantic Search

Find chunks by meaning:

```bash
fubbik search "how do we handle user authentication" --semantic
```

This embeds your query and finds the most semantically similar chunks using cosine similarity in PostgreSQL (via pgvector).

Semantic search is especially useful for:
- Natural language questions ("how do we deploy to production?")
- Finding conceptually related content across different terminology
- Discovering chunks you didn't know existed

## Duplicate Detection

When creating a new chunk, fubbik can check if similar content already exists:

```bash
POST /api/chunks/check-similar
```

This uses embedding similarity to find chunks that might be duplicates or cover overlapping ground. The web UI shows warnings during chunk creation.

## Context Export for AI Tools

The most powerful AI integration is context export — serving the right knowledge to AI tools within their token budgets.

### How It Works

1. All chunks are scored by: health, type weight, connection count, review status
2. If a file path is specified, chunks with matching file references get a relevance boost
3. Chunks are greedily selected until the token budget is filled
4. The result is formatted as structured markdown

### Usage

```bash
# Export context with token budget
fubbik context --max-tokens 4000

# Boost relevance for a specific file
fubbik context --for src/auth/session.ts

# Generate context for a file with dependency awareness
fubbik context-for src/auth/session.ts --include-deps

# Generate CLAUDE.md for a directory
fubbik context-dir src/auth/
```

### MCP Integration

The MCP server exposes context tools to AI agents:

- `get_conventions` — Get coding conventions relevant to a file
- `search_chunks` — Search knowledge by text
- `get_chunk` — Get full chunk details

This means AI coding assistants automatically receive relevant knowledge when working on your code.

## Working Without AI

If Ollama isn't running:
- Chunk creation and editing work normally
- Full-text search works (keyword-based)
- All graph, connection, and organizational features work
- Enrichment fields (summary, aliases) remain empty until enriched
- Semantic search is unavailable
- Duplicate detection is unavailable
