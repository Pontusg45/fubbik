---
tags:
  - guide
  - features
  - overlays
  - lifecycle
description: Feature lifecycle — creating, activating, deactivating, and priority ordering
---

# Creating and Managing Features

## Creating a Feature

At `/features`, click "New Feature" and provide:

- **Name** — a descriptive label (e.g., "Auth Rewrite Proposal")
- **Description** — what this feature explores or changes
- **Priority** — unique per user; higher priority wins on same-field conflicts
- **Color** — visual identifier shown in the UI
- **Codebases** — optional codebase associations

## Feature Status

| Status | Meaning |
|--------|---------|
| `inactive` | Created but not active — deltas exist but aren't applied |
| `active` | Currently enabled — deltas overlay on base chunks |
| `merged` | Permanently applied to base chunks |
| `archived` | Discarded — no longer visible |

## Activating Features

Use the feature switcher in the nav bar (alongside the codebase switcher) to toggle features on and off. Multiple features can be active simultaneously — priority determines which delta wins when two features modify the same field on the same chunk.

## Priority and Conflict Resolution

When multiple active features modify the same field on a chunk, the feature with the higher priority number wins. The resolution formula:

```
Object.assign(baseChunk, ...deltasAscByPriority)
```

Deltas are applied in ascending priority order, so the last (highest) wins.
