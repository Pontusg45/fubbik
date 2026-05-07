---
tags:
  - guide
  - cli
  - documents
description: CLI commands for importing, syncing, and rendering documents
---

# Document Commands

## Importing Documents

```bash
# Import a single markdown file as a browsable document
fubbik docs import docs/getting-started.md

# Import an entire directory
fubbik docs import-dir docs/guide/

# Import chunks from a file (JSON or markdown)
fubbik import chunks.json
fubbik import docs/architecture.md --server --codebase my-app

# Import a directory of markdown files
fubbik import docs/ --server --codebase my-app
```

## Syncing and Rendering

```bash
# Re-sync changed files from disk
fubbik docs sync

# List imported documents
fubbik docs list

# Render a document back to markdown
fubbik docs render <document-id>
```

## Bulk Export

```bash
# Export all chunks as JSON
fubbik export --format json > chunks.json

# Export one file per chunk
fubbik export --format markdown --output-dir ./exported/
```
