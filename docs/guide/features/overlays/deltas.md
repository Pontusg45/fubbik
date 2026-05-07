---
tags:
  - guide
  - features
  - overlays
  - deltas
description: Editing chunks within a feature context — how deltas work
---

# Working with Deltas

A delta is a sparse JSONB object containing only the fields that a feature changes on a specific chunk. Supported delta fields:

- `title`
- `content`
- `type`
- `rationale`
- `alternatives`
- `consequences`
- `summary`

## Creating Deltas

When features are active and you edit a chunk, a "Save to Feature" dialog appears offering to save your changes as a delta rather than modifying the base chunk directly.

You can also manage deltas via the API:

```
PUT /api/chunks/:id/deltas/:featureId
```

Body: `{ "delta": { "content": "New proposed content..." } }`

## Viewing Deltas

- **Chunk detail page** — feature overlay indicators show which fields are modified by active features
- **Feature detail page** — lists all deltas with affected chunks
- **API**: `GET /api/features/:id/deltas` or `GET /api/chunks/:id/deltas`

## How Overlays Apply

The enrichment pipeline applies active feature overlays automatically. All context paths (for-file, for-plan, about) get overlays applied, so AI tools always see the feature-modified version when features are active.
