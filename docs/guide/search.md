---
tags:
  - guide
  - search
description: Full-text, semantic, and federated search
---

# Search

Fubbik provides multiple ways to find knowledge across your chunks and documents.

## Full-Text Search

The primary search at `/search` searches across all entity types — chunks, tags, connections, requirements, plans, and documents.

Type in the search bar or press `Cmd+K` to open the command palette for quick search.

### Chunk List Search

The chunks list at `/chunks` has its own search that filters by title and content. Combine with type, tag, and codebase filters for precise results.

### Document Search

The docs browser at `/docs` has full-text search across all imported document sections. Results are grouped by document with highlighted snippets. Click a result to navigate to that section.

## Semantic Search

With Ollama running, fubbik supports meaning-based search using vector embeddings. This finds chunks that are conceptually related even if they don't share keywords.

```bash
# CLI semantic search
fubbik search "how do we handle authentication" --semantic
```

In the web UI, semantic search is available on the search page when embeddings have been generated.

### How It Works

1. Each chunk gets a 768-dimensional vector embedding (via `nomic-embed-text`)
2. Your search query is embedded using the same model
3. Results are ranked by cosine similarity
4. The most semantically similar chunks are returned

### Generating Embeddings

Embeddings are generated automatically when chunks are created or updated (if Ollama is running). To bulk-generate:

```bash
fubbik enrich --all
```

## Federated Search

Federated search finds chunks across all codebases simultaneously, showing which codebase each result belongs to. Access via:

```
GET /api/chunks/search/federated?q=authentication
```

This is useful for finding patterns and conventions that exist across multiple projects.

## Search Tips

- **Use specific terms** — "JWT session expiry" finds more relevant results than "auth"
- **Search by tag** — filter the chunk list by tags to narrow results before searching
- **Combine filters** — type + tag + codebase + search text gives the most precise results
- **Try semantic search** — when keyword search misses conceptually related content
- **Use aliases** — AI-generated aliases make chunks findable by alternative names
