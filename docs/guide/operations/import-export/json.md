---
tags:
  - guide
  - import
  - export
  - json
description: Bulk import/export of chunks in JSON format
---

# JSON Import and Export

## Importing from JSON

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

## Exporting to JSON

Export all chunks as JSON:

```bash
fubbik export --format json > chunks.json
```

The JSON export includes all chunk metadata: title, content, type, tags, rationale, alternatives, consequences, file references, and more.

## Markdown Export

Export one file per chunk:

```bash
fubbik export --format markdown --output-dir ./exported/
```
