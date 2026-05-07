---
tags:
  - guide
  - chunks
description: Chunks section index — types, tags, file references, health, and enrichment
---

# Chunks

Chunks are the building blocks of your knowledge base. Each one represents a discrete unit of knowledge — small enough to be useful on its own, rich enough to capture context.

## Creating Chunks

Navigate to **Chunks > New** or press `n` on any page. Fill in:

- **Title** — a clear, descriptive name
- **Content** — markdown-formatted knowledge
- **Type** — note, document, reference, schema, or checklist
- **Tags** — categorize with tags (e.g., "backend", "auth")

**Templates** pre-fill content structure. Choose from built-in templates or create your own at `/templates`.

**Duplicate detection** warns you if a chunk with similar content already exists (requires Ollama).

## In This Section

- [Types and Decision Context](./types.md) — chunk types and ADR fields
- [Tags and Organization](./tags.md) — categorizing chunks with tags and tag types
- [File References](./file-references.md) — linking chunks to code
- [Health Scores](./health-scores.md) — measuring chunk quality
- [Enrichment](./enrichment/) — AI-generated metadata (summaries, aliases, embeddings, not-about)
