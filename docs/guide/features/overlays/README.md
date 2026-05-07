---
tags:
  - guide
  - features
  - overlays
description: Knowledge overlays (features) — parallel branches of chunk modifications
---

# Knowledge Overlays (Features)

Features are named entities that track field-level modifications to chunks — like branches for your knowledge base. Each feature stores deltas (sparse objects containing only changed fields) that overlay on top of the base chunks.

Manage features at `/features`.

## In This Section

- [Creating and Managing Features](./creating-features.md) — lifecycle, priority, and activation
- [Working with Deltas](./deltas.md) — editing chunks within a feature context
- [Merging and Archiving](./merging.md) — applying changes permanently or discarding
