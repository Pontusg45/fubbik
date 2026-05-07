---
tags:
  - guide
  - search
  - semantic
description: Meaning-based search using vector embeddings
---

# Semantic Search

With Ollama running, fubbik supports meaning-based search using vector embeddings. This finds chunks that are conceptually related even if they don't share keywords.

```bash
fubbik search "how do we handle authentication" --semantic
```

## How It Works

1. Each chunk gets a 768-dimensional vector embedding (via `nomic-embed-text`)
2. Your search query is embedded using the same model
3. Results are ranked by cosine similarity
4. The most semantically similar chunks are returned

## Generating Embeddings

Embeddings are generated automatically when chunks are created or updated (if Ollama is running). To bulk-generate:

```bash
fubbik enrich --all
```

## When to Use Semantic Search

- Natural language questions ("how do we deploy to production?")
- Finding conceptually related content across different terminology
- Discovering chunks you didn't know existed
