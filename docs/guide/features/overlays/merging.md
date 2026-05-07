---
tags:
  - guide
  - features
  - overlays
  - merging
description: Merging feature deltas permanently or archiving features
---

# Merging and Archiving

## Merging a Feature

Merging permanently applies all of a feature's deltas to the base chunks:

1. Go to `/features` and find the feature
2. Click "Merge"
3. Confirm — this is irreversible

What happens on merge:
- Each delta is applied to its base chunk
- Version snapshots are created for each modified chunk
- The feature status changes to `merged`
- The feature and its deltas are preserved for history

## Archiving a Feature

If you decide not to proceed with a feature's changes:

1. Click "Archive" on the feature
2. Status changes to `archived`
3. Deltas are preserved but no longer apply

Archived features are hidden from the main list but can still be viewed.

## Deleting a Feature

Permanently removes the feature and all its deltas. This cascades — all delta records are deleted. Use archive instead if you want to preserve history.

## Reordering Priority

Change a feature's priority via "Reorder" to adjust conflict resolution order when multiple features touch the same chunks.
