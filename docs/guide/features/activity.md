---
tags:
  - guide
  - activity
  - audit
description: Activity log with action and entity type filtering
---

# Activity Log

The activity view at `/activity` shows a chronological log of all changes in your knowledge base.

## Tracked Actions

| Action | Examples |
|--------|---------|
| `created` | New chunk, connection, tag, plan |
| `updated` | Content edit, status change, tag modification |
| `deleted` | Chunk removal, connection deletion |
| `archived` | Chunk archival |

## Tracked Entities

- Chunks
- Connections
- Tags
- Requirements
- Plans
- Codebases

## Filtering

Filter the log by:
- **Action type** — show only creates, updates, or deletes
- **Entity type** — show only chunks, connections, etc.
- **Date range** — focus on a specific time period

## Use Cases

- **Audit trail** — understand who changed what and when
- **Change review** — review recent modifications before a release
- **Debugging** — trace when a chunk was last modified if its content seems wrong
