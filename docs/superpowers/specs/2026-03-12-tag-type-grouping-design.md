# Tag Type Grouping in Graph View

## Summary

Add the ability to group graph nodes into visual clusters based on tag types. Tags gain a structured data model (normalized tables) with user-defined tag types (e.g., "feature", "techstack", "domain"). When a tag type is selected for grouping, nodes cluster by their tags of that type, with colored background regions drawn behind each cluster and cross-group edges dimmed.

## Data Model

### New Tables

**tag_type**
- `id`: uuid (PK)
- `name`: text (unique per user) — e.g., "feature", "techstack"
- `color`: text — hex color for visual regions
- `userId`: text (FK → user)

**tag**
- `id`: uuid (PK)
- `name`: text (unique per user) — e.g., "authentication", "react"
- `tagTypeId`: uuid (FK → tag_type, nullable)
- `userId`: text (FK → user)

**chunk_tag** (join table)
- `chunkId`: uuid (FK → chunk)
- `tagId`: uuid (FK → tag)
- PK: (chunkId, tagId)

### Migration

Existing `tags: jsonb` string arrays on the `chunk` table are migrated:
1. Extract unique tag names → insert into `tag` table (no type assigned initially)
2. Create `chunk_tag` rows for each chunk–tag relationship
3. Drop the `tags` jsonb column from `chunk`

## Graph Grouping Behavior

### Activation

A new "Group by Tag Type" section in the existing `GraphFilters` panel (below Types and Relations). Each user-defined tag type is shown as a badge with its color. Clicking toggles grouping by that type.

### Clustering

When one or more tag types are selected for grouping:
1. **Force layout**: Add tag-based attractor forces (similar to existing type centroid clustering in `force-layout.ts`). Nodes sharing the same tag value are attracted to their tag's centroid.
2. **Background regions**: Compute convex hull of each tag group's node positions. Render semi-transparent SVG regions behind the ReactFlow canvas using the tag type's color.
3. **Ungrouped bucket**: Nodes without any tag of the selected type cluster into an "ungrouped" region with a neutral dashed border.

### Multi-Tag Nodes (Duplication)

A chunk with multiple tags of the active type (e.g., tags "authentication" and "search", both type "feature") appears in all matching groups:
- Primary instance: normal rendering
- Ghost instances: dashed border, slightly reduced opacity, to indicate duplication
- Selecting any instance highlights all instances of the same chunk

### Edge Opacity

| Condition | Opacity |
|-----------|---------|
| Both nodes in same active group | 1.0 |
| Nodes in different groups (cross-group) | 0.15 |
| Selected node's direct connections | 1.0 (overrides cross-group dim) |
| No grouping active | 1.0 (current behavior) |

## Architecture

### Backend

- **Schema**: `packages/db/src/schema/` — new `tagType.ts`, `tag.ts`, `chunkTag.ts` schema files
- **Repositories**: `packages/db/src/repository/` — CRUD for tag types, tags, chunk-tag associations
- **Services**: `packages/api/src/` — tag type and tag management services
- **Routes**: `packages/api/src/` — REST endpoints for tag types, tags, chunk-tag management
- **Graph service**: Extend `getUserGraph` to include tag and tag type data in the response

### Frontend

- **Force layout** (`force-layout.ts`): Accept optional tag grouping config. Add tag centroid attractor forces when grouping is active.
- **Layout worker** (`layout.worker.ts`): Pass tag group info to the layout function.
- **Graph filters** (`graph-filters.tsx`): Add "Group by Tag Type" section with tag type badges.
- **Graph view** (`graph-view.tsx`):
  - New state: `activeTagTypes: Set<string>` for selected tag types
  - Node duplication logic for multi-tag chunks
  - Edge opacity computation based on group membership
  - SVG background layer for convex hull regions
- **New component**: `graph-tag-regions.tsx` — SVG overlay rendering convex hull backgrounds with tag type colors and labels.

## Non-Goals

- Tag/tag type CRUD UI (management screens) — separate feature, can use existing patterns
- Drag-to-reparent (moving a node between groups by dragging)
- Nested grouping (grouping by multiple tag types simultaneously with nesting)
