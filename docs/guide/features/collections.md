---
tags:
  - guide
  - collections
description: Saved chunk collections with reusable filters
---

# Collections

Collections are saved filter configurations that let you quickly access specific subsets of chunks.

## Creating a Collection

1. Set up your desired filters on the chunk list (type, tags, search, codebase, sort, etc.)
2. Click "Save as Collection"
3. Give it a name

## What's Saved

A collection stores:
- **Type filter** — which chunk types to include
- **Tag filters** — which tags to require
- **Search text** — keyword filter
- **Sort order** — how to order results
- **Enrichment filter** — only enriched or only un-enriched
- **Connection filter** — minimum connection count
- **Origin filter** — manual, AI, or import
- **Review status filter** — pending, approved, rejected

## Use Cases

- **"My backend conventions"** — type=note, tags=backend+convention
- **"Needs review"** — review_status=pending, sort=oldest
- **"Orphans to fix"** — connections=0, sort=health_asc
- **"AI-generated drafts"** — origin=ai, review_status=pending
