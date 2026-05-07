---
tags:
  - guide
  - views
  - kanban
description: Kanban board view organized by chunk type with drag-and-drop
---

# Kanban Board

The kanban view at `/chunks` (toggle from list view) displays chunks as cards organized into columns by type.

## Columns

Each chunk type gets its own column:
- **Notes** — general knowledge, conventions
- **Documents** — architecture docs, guides
- **References** — API docs, specifications
- **Schemas** — data models, type definitions
- **Checklists** — procedures, runbooks

## Drag and Drop

Drag a chunk card between columns to change its type. This is a quick way to reclassify chunks without opening the edit page.

## Filtering

All standard filters (codebase, tags, search) apply to the kanban view. Cards show title, truncated content preview, and tag badges.
