---
tags:
  - guide
  - ai
  - enrichment
description: Auto-generated summaries, aliases, and metadata via Ollama
---

# Chunk Enrichment

When you create or edit a chunk, fubbik can auto-generate:

- **Summary** — A one-line description of the chunk
- **Aliases** — Alternative names the chunk might be known by
- **Not About** — Terms this chunk is explicitly NOT about (improves search precision)

## Running Enrichment

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
