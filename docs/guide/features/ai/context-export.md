---
tags:
  - guide
  - ai
  - context
description: Token-budgeted knowledge delivery for AI tools
---

# Context Export

The most powerful AI integration is context export — serving the right knowledge to AI tools within their token budgets.

## How It Works

1. All chunks are scored by: health, type weight, connection count, review status
2. If a file path is specified, chunks with matching file references get a relevance boost
3. Chunks are greedily selected until the token budget is filled
4. The result is formatted as structured markdown

## Usage

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

## CLAUDE.md Generation

Generate a CLAUDE.md file from tagged chunks:

```bash
# One-time generation
fubbik sync-claude-md

# Watch mode (regenerates on changes)
fubbik sync-claude-md --watch
```

Tag chunks with `claude-md` to include them in the generated file.

## MCP Integration

The MCP server exposes context tools to AI agents:

- `get_conventions` — Get coding conventions relevant to a file
- `search_chunks` — Search knowledge by text
- `get_chunk` — Get full chunk details

This means AI coding assistants automatically receive relevant knowledge when working on your code.
