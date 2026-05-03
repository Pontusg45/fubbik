# Feature Flag Knowledge Overlays

## Summary

A delta/overlay system that lets users associate chunk modifications with named features. Each feature stores field-level deltas on top of base chunks. Features have priority ordering — when multiple features modify the same field, the highest-priority feature wins. Features can be toggled on/off globally via a nav switcher, merged permanently into base chunks, or kept as persistent layers.

## Core Concepts

### Features

A named grouping that represents a body of work (e.g., "OAuth Migration", "Dark Mode", "Billing v2"). Features have:

- A **priority** (integer, unique per user) that determines override order when multiple features touch the same chunk field
- A **status**: `active`, `inactive`, `merged`, `archived`
- Optional **codebase associations** — a feature can be linked to one or more codebases, or none (global)

### Deltas

A delta is a JSONB object containing only the chunk fields that a feature modifies. For example, if feature "OAuth Migration" changes only the `content` of a chunk, the delta is `{ "content": "Updated OAuth flow description..." }`.

Deltas are stored one per chunk per feature. They are not full snapshots — they are sparse overlays.

### Resolution

When a user views a chunk, the system computes the **resolved** version:

1. Start with the base chunk
2. Fetch all deltas for this chunk from the user's active features
3. Apply deltas in ascending priority order (lowest first, highest last)
4. The highest-priority feature's value wins for any given field

This is equivalent to `Object.assign(base, ...deltasAscByPriority)`.

### Feature Lifecycle

- **Create** — define a name, description, priority, optional codebase links
- **Activate/Deactivate** — toggle in the nav switcher, controls what the user sees
- **Merge** — permanently apply all deltas to base chunks (like merging a git branch), feature status becomes `merged`
- **Archive** — soft-remove from the switcher, deltas preserved but not applicable

## Data Model

### `feature` table

| Column | Type | Constraints |
|--------|------|-------------|
| `id` | text | PK, UUID |
| `name` | text | not null |
| `description` | text | nullable |
| `priority` | integer | not null |
| `status` | text | not null, default `inactive` |
| `color` | text | nullable, hex color for UI |
| `userId` | text | FK → user, not null |
| `createdAt` | timestamp | not null, default now |
| `updatedAt` | timestamp | not null, default now, auto-update |

- Unique constraint on `(userId, name)` — feature names are unique per user
- Unique constraint on `(userId, priority)` — no two features share a priority

### `feature_codebase` table

| Column | Type | Constraints |
|--------|------|-------------|
| `featureId` | text | FK → feature, CASCADE on delete |
| `codebaseId` | text | FK → codebase, CASCADE on delete |

- Composite PK on `(featureId, codebaseId)`

### `chunk_feature_delta` table

| Column | Type | Constraints |
|--------|------|-------------|
| `id` | text | PK, UUID |
| `chunkId` | text | FK → chunk, CASCADE on delete |
| `featureId` | text | FK → feature, CASCADE on delete |
| `delta` | jsonb | not null |
| `createdAt` | timestamp | not null, default now |
| `updatedAt` | timestamp | not null, default now, auto-update |

- Unique constraint on `(chunkId, featureId)` — one delta per chunk per feature

### `user_active_feature` table

| Column | Type | Constraints |
|--------|------|-------------|
| `userId` | text | FK → user, CASCADE on delete |
| `featureId` | text | FK → feature, CASCADE on delete |

- Composite PK on `(userId, featureId)`

### Delta field allowlist

The `delta` JSONB may contain these chunk content fields only:

- `title`
- `content`
- `type`
- `rationale`
- `alternatives`
- `consequences`
- `summary`

Relational data (tags, connections, codebases, appliesTo, fileReferences) is excluded. Overlays are for content, not structure.

## API Design

### Feature CRUD

| Method | Endpoint | Body/Params | Notes |
|--------|----------|-------------|-------|
| `GET` | `/api/features` | `?codebaseId`, `?status`, `?search` | List features with delta count |
| `POST` | `/api/features` | `{ name, description?, priority, color?, codebaseIds? }` | Create feature |
| `GET` | `/api/features/:id` | | Detail with codebases and all deltas (including chunk titles) |
| `PATCH` | `/api/features/:id` | `{ name?, description?, priority?, status?, color? }` | Update feature |
| `DELETE` | `/api/features/:id` | | Deletes feature + all deltas (CASCADE) |
| `POST` | `/api/features/:id/merge` | | Apply all deltas to base chunks, set status to `merged` |
| `POST` | `/api/features/:id/reorder` | `{ priority }` | Change priority, shift others to make room |

### Feature activation

| Method | Endpoint | Body | Notes |
|--------|----------|------|-------|
| `GET` | `/api/features/active` | | User's active feature IDs |
| `PUT` | `/api/features/active` | `{ featureIds: string[] }` | Set active features |

### Chunk deltas

| Method | Endpoint | Body | Notes |
|--------|----------|------|-------|
| `GET` | `/api/features/:id/deltas` | | All deltas for a feature |
| `GET` | `/api/chunks/:id/deltas` | | All deltas across features for a chunk |
| `PUT` | `/api/chunks/:id/deltas/:featureId` | `{ delta }` | Upsert delta |
| `DELETE` | `/api/chunks/:id/deltas/:featureId` | | Remove delta |

### Modified existing endpoints

These chunk-reading endpoints resolve through active feature deltas:

- `GET /api/chunks` — returns resolved chunks with `_appliedFeatures` metadata
- `GET /api/chunks/:id` — returns resolved chunk with `_deltas` array (all features, not just active)
- `GET /api/chunks/export/context` — resolved content for AI consumption
- `GET /api/chunks/export/claude-md` — resolved content
- `GET /api/chunks/search/semantic` — searches base embeddings (not resolved content)
- `GET /api/chunks/search/federated` — searches base content

Version history (`GET /api/chunks/:id/history`) always returns base chunk history, unaffected by features.

## Backend Architecture

### New files

Following the existing repository → service → route pattern:

**Schema:**
- `packages/db/src/schema/feature.ts` — `feature`, `feature_codebase`, `chunk_feature_delta`, `user_active_feature` table definitions

**Repository:**
- `packages/db/src/repository/feature.ts` — feature CRUD, codebase association, active feature management
- `packages/db/src/repository/chunk-feature-delta.ts` — delta CRUD, batch fetch by chunk IDs + feature IDs

**Service:**
- `packages/api/src/features/service.ts` — business logic, merge flow, priority reordering
- `packages/api/src/features/routes.ts` — Elysia route definitions

**Resolution:**
- `packages/api/src/features/resolve.ts` — `resolveChunk()` and `resolveChunks()` utilities

### Resolution utility

```typescript
// packages/api/src/features/resolve.ts

type Delta = { featureId: string; delta: Record<string, unknown>; priority: number }

function resolveChunk(chunk: Chunk, deltas: Delta[]): ResolvedChunk {
  const sorted = deltas.sort((a, b) => a.priority - b.priority) // ascending
  const resolved = { ...chunk }
  const appliedFeatures: string[] = []
  for (const d of sorted) {
    Object.assign(resolved, d.delta)
    appliedFeatures.push(d.featureId)
  }
  return { ...resolved, _appliedFeatures: appliedFeatures, _hasDeltas: deltas.length > 0 }
}

function resolveChunks(chunks: Chunk[], activeFeatureIds: string[]): ResolvedChunk[] {
  if (activeFeatureIds.length === 0) return chunks // no-op fast path
  const chunkIds = chunks.map(c => c.id)
  const allDeltas = batchFetchDeltas(chunkIds, activeFeatureIds) // single query
  return chunks.map(chunk => {
    const deltas = allDeltas.filter(d => d.chunkId === chunk.id)
    return resolveChunk(chunk, deltas)
  })
}
```

### Active features in request context

An Elysia `derive` plugin reads `user_active_feature` for the authenticated user and injects `activeFeatureIds: string[]` into the request context. All downstream services have access without explicit parameter passing.

### Merge flow

`POST /api/features/:id/merge`:

1. Verify feature exists and belongs to user
2. Fetch all deltas for the feature
3. Begin transaction
4. For each delta:
   - Fetch base chunk
   - Create version snapshot (existing `createVersion` pattern)
   - Apply `Object.assign(baseFields, delta.delta)`
   - Save base chunk via `updateChunkRepo`
   - Delete the delta row
5. Set `feature.status = 'merged'`
6. Commit transaction
7. Fire-and-forget re-enrichment for all affected chunks (regenerate embeddings)

Atomic — rolls back entirely on any failure.

### Performance

- **Batch delta fetching:** `resolveChunks` fetches all deltas in one query using `WHERE chunkId IN (...) AND featureId IN (...)`. No N+1.
- **Zero overhead without features:** If `activeFeatureIds` is empty, resolution is a no-op passthrough.
- **Small table:** One row per chunk × feature. Even aggressive usage (50 features × 1000 chunks) yields at most 50k small JSONB rows.

## UI Design

### Feature switcher (nav)

A dropdown in the top nav bar alongside the codebase switcher:

- Flag/layers icon with badge showing count of active features
- Dropdown lists all non-archived features grouped by status (`active` first, then `inactive`)
- Each row: colored dot (feature color), feature name, priority number, toggle switch
- Toggling calls `PUT /api/features/active`
- "Manage Features" link at bottom navigates to `/features`

### Features page (`/features`)

Dedicated management page:

- List of all features: name, description, priority, status, delta count, linked codebases
- Create new feature form (name, description, priority, color, optional codebase links)
- Inline priority reordering (drag or up/down arrows)
- Per-feature actions: edit, merge, archive, delete
- Merge confirmation dialog listing all chunks that will be permanently modified, with field-level preview

### Chunk list modifications

When features are active:

- Chunks with applied deltas show a colored dot/badge indicating which features affect them
- New filter option: "Feature-modified only" to isolate what a feature touches
- "Features" section appears in the filter bar when features exist

### Chunk detail modifications

- "Feature Overlays" section when any deltas exist for the chunk (active or not)
- Lists each feature modifying this chunk, which fields it changes, and whether it's active
- Toggle to view "base only" vs. "resolved" for comparison

### Editing with active features

The key UX moment — when saving a chunk edit with active features:

1. Edit form shows the **resolved** version (base + active deltas)
2. On save, a dialog appears: **"Save to"**:
   - **Base chunk** — modifies the underlying chunk directly
   - **Feature: [name]** — one option per active feature, saves as a delta
3. When saving to a feature, the system diffs the edited version against the base chunk, storing only changed fields as the delta
4. If a delta already exists for that chunk + feature, it's replaced

### Graph view

- Chunks with active feature deltas get a subtle visual indicator (colored border matching feature color)
- Optional graph filter to highlight/isolate chunks affected by a specific feature

## Out of Scope (This Iteration)

- **Embedding resolution** — semantic search uses base embeddings. Feature deltas don't trigger re-embedding. Merged features do.
- **Connection/tag overlays** — deltas are content-only. Structural relationships stay on the base chunk.
- **MCP/CLI integration** — MCP server and CLI operate on base chunks. Feature awareness can be added later by passing active feature IDs.
- **Git integration** — features are a pure knowledge-layer concept with no git branch coupling.
- **Multi-user feature sharing** — features are per-user. Shared/team features can be added later.
- **Conflict visualization** — when two active features modify the same field, the higher-priority one wins silently. No UI for viewing/resolving the override. Can be added later.
