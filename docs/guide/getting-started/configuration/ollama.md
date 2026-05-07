---
tags:
  - guide
  - configuration
  - ollama
description: Setting up Ollama for local AI features
---

# Ollama Setup

Ollama powers fubbik's AI features: enrichment, semantic search, and duplicate detection. All AI features are optional — everything else works without Ollama.

## Installation

```bash
# macOS
brew install ollama

# Start the server
ollama serve

# Pull required models
ollama pull nomic-embed-text   # Embedding model (768-dim vectors)
ollama pull llama3.2           # Generation model for enrichment
```

## Configuration

Set the URL in your environment (default is `http://localhost:11434`):

```
OLLAMA_URL=http://localhost:11434
```

## What Ollama Powers

- **Chunk enrichment** — auto-generated summaries, aliases, and "not about" terms
- **Vector embeddings** — 768-dimensional embeddings for semantic search
- **Duplicate detection** — warns when creating chunks similar to existing ones

## Working Without Ollama

If Ollama isn't running:
- Chunk creation and editing work normally
- Full-text search works (keyword-based)
- All graph, connection, and organizational features work
- Enrichment fields remain empty until enriched
- Semantic search and duplicate detection are unavailable
