---
tags:
  - guide
  - import
  - export
description: Importing and exporting knowledge
---

# Import and Export

Fubbik supports multiple ways to get knowledge in and out of the system.

## Importing Chunks

### From JSON

Export chunks from another fubbik instance or create them programmatically:

```bash
fubbik import chunks.json
```

The JSON format:
```json
{
  "chunks": [
    {
      "title": "My Chunk",
      "content": "Markdown content here",
      "type": "note",
      "tags": ["backend", "auth"]
    }
  ]
}
```

### From Markdown Files

Import a single markdown file or an entire directory:

```bash
# Single file
fubbik import docs/architecture.md --server --codebase my-app

# Directory (recursive)
fubbik import docs/ --server --codebase my-app
```

Frontmatter is parsed for metadata:
```markdown
---
title: My Document
type: document
tags:
  - backend
  - architecture
---

# Content starts here
```

### As Structured Documents

Import markdown files as documents that preserve the file's structure and can be browsed as pages:

```bash
# Import as a browsable document (split on H2 headings)
fubbik docs import docs/getting-started.md

# Import a whole directory
fubbik docs import-dir docs/guide/
```

Documents track the original file path and content hash, enabling re-sync when the source changes:

```bash
# Re-import changed files
fubbik docs sync
```

## Exporting

### Bulk Export

Export all chunks as JSON:

```bash
fubbik export --format json > chunks.json
fubbik export --format markdown --output-dir ./exported/
```

The JSON export includes all chunk metadata. The markdown export creates one file per chunk.

### Context Export

Export a token-budgeted selection of the most relevant knowledge:

```bash
# Export up to 4000 tokens of context
fubbik context --max-tokens 4000

# Boost relevance for a specific file
fubbik context --for src/auth/session.ts
```

The context export scores chunks by health, type, connections, and file relevance, then greedily fills a token budget with the highest-scoring content.

### CLAUDE.md Generation

Generate a CLAUDE.md file from tagged chunks:

```bash
# One-time generation
fubbik sync-claude-md

# Watch mode
fubbik sync-claude-md --watch
```

Tag chunks with `claude-md` to include them in the generated file.

### Requirements Export

Export requirements in multiple formats:

```bash
fubbik requirements export --format gherkin   # Cucumber .feature files
fubbik requirements export --format vitest    # TypeScript test scaffolds
fubbik requirements export --format markdown  # Checklist format
```

## Document Rendering

Reconstruct an imported document back to markdown:

```bash
fubbik docs render <document-id>
```

This outputs the full document with all sections in order, useful for generating updated documentation from edited chunks.

## Web UI Import

The `/import` page provides a drag-and-drop interface for importing markdown files with:
- File preview table
- Codebase selection
- Frontmatter extraction preview
- Bulk import with progress tracking
