---
tags:
  - guide
  - import
  - documents
description: Importing markdown as structured, browsable documents
---

# Document Import

Import markdown files as documents that preserve the file's structure and can be browsed as pages:

```bash
# Import as a browsable document (split on H2 headings)
fubbik docs import docs/getting-started.md

# Import a whole directory
fubbik docs import-dir docs/guide/
```

## How It Works

Documents track the original file path and content hash. Each document is split into sections at H2 headings, creating individual chunks that are linked with `part_of` connections.

## Re-Syncing

When the source file changes on disk, re-sync to update:

```bash
fubbik docs sync
```

Only changed files are re-imported (checked via content hash).

## Rendering

Reconstruct an imported document back to markdown:

```bash
fubbik docs render <document-id>
```

This outputs the full document with all sections in order, useful for generating updated documentation from edited chunks.
