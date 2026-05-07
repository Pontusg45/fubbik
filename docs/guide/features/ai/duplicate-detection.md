---
tags:
  - guide
  - ai
  - duplicates
description: Embedding-based similarity checking for duplicate prevention
---

# Duplicate Detection

When creating a new chunk, fubbik can check if similar content already exists:

```bash
POST /api/chunks/check-similar
```

This uses embedding similarity to find chunks that might be duplicates or cover overlapping ground.

## How It Works

1. The new chunk's content is embedded using `nomic-embed-text`
2. Cosine similarity is computed against all existing chunk embeddings
3. Chunks above a similarity threshold are returned as potential duplicates

## In the Web UI

The web UI shows duplicate warnings automatically during chunk creation. If similar chunks are found, you'll see them listed with similarity scores before saving. You can then decide to:

- Continue creating (the content is different enough)
- Edit the existing chunk instead
- Merge the content into the existing chunk

## Requirements

Duplicate detection requires Ollama running with the `nomic-embed-text` model. Without it, chunks are created without similarity checking.
