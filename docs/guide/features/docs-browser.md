---
tags:
  - guide
  - documents
  - browser
description: Imported documentation browser with full-text search
---

# Documentation Browser

The docs browser at `/docs` provides a dedicated interface for browsing imported documentation — separate from the chunk list.

## Features

- **Document list** — all imported documents with metadata
- **Full-text search** — search across all document sections with highlighted snippets
- **Section navigation** — documents split on H2 headings with a table of contents
- **Grouped results** — search results grouped by source document

## Importing Documents

Documents can be imported via:
- CLI: `fubbik docs import <file>` or `fubbik docs import-dir <dir>`
- Web UI: `/import` page with drag-and-drop
- API: `POST /api/chunks/import-docs`

See the [Import/Export section](../operations/import-export/) for details.

## API Reference

The docs browser also supports viewing API documentation through Swagger UI at `http://localhost:3000/docs`.
