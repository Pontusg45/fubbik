---
tags:
  - guide
  - review
  - status
description: Chunk review lifecycle and status management
---

# Review Status

Every chunk has a review status that tracks its review lifecycle.

## Statuses

| Status | Meaning |
|--------|---------|
| `pending` | Awaiting review — newly created or imported |
| `approved` | Reviewed and accepted |
| `rejected` | Reviewed and discarded |

## Quick Toggle

On the chunk list page, you can quickly toggle a chunk's review status using the inline row actions — no need to open the detail page.

## Context Export Impact

Review status affects context export scoring. Approved chunks receive a boost, while pending chunks are deprioritized. This ensures AI tools receive reviewed, trusted knowledge.

## Filtering by Status

The chunk list supports filtering by review status, making it easy to find all pending chunks that need attention.
