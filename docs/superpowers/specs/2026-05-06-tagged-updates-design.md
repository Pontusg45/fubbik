# Tagged Chunk Updates

**Date:** 2026-05-06
**Scope:** Add per-edit update tags to the chunk version system, with cross-codebase querying and diff support

## Overview

Extend the existing `chunk_version` table with an `updateTag` field so each chunk edit (or creation) can be labeled with a tag like "feature-x". A query endpoint returns all updates with a given tag across codebases, including before/after diffs. CLI and MCP tools support passing and querying tags.

## Approach

**Extend `chunk_version` (Approach A):** Add columns to the existing version table rather than creating new tables. The version system already snapshots chunk state on every edit — adding a tag column and expanding the snapshot fields is minimal.

## Design

### 1. Schema changes

The `chunk_version` table gains these columns:

- `updateTag` (text, nullable) — the label for this edit. Indexed for fast querying.
- `scope` (jsonb, nullable) — pre-edit scope snapshot
- `rationale` (text, nullable) — pre-edit rationale snapshot
- `alternatives` (text array, nullable) — pre-edit alternatives snapshot
- `consequences` (text, nullable) — pre-edit consequences snapshot

The version row continues to capture the **pre-edit state**. When a tag is provided during an update, it's stored on the version that records what changed. No new tables.

Index: `chunk_version_update_tag_idx` on `(updateTag)` where `updateTag IS NOT NULL`.

#### Files

- `packages/db/src/schema/chunk-version.ts` — add columns

### 2. API changes

**Update endpoint** — `PATCH /api/chunks/:id` gains an optional `updateTag` body field. When provided, the version created during the update stores the tag.

**Create endpoint** — `POST /api/chunks` gains an optional `updateTag` body field. When provided, a version-0 row is created with all before fields null and the tag stored, representing a tagged creation.

**Query endpoint** — `GET /api/chunks/updates?tag=feature-x&codebaseId=<optional>` returns all versions with that tag. Response:

```typescript
{
    updates: Array<{
        versionId: string;
        chunkId: string;
        chunkTitle: string;
        updateTag: string;
        version: number;
        createdAt: string;
        before: {
            title: string | null;
            content: string | null;
            type: string | null;
            rationale: string | null;
            alternatives: string[] | null;
            consequences: string | null;
            scope: Record<string, string> | null;
        };
        after: {
            title: string;
            content: string;
            type: string;
            rationale: string | null;
            alternatives: string[] | null;
            consequences: string | null;
            scope: Record<string, string>;
        };
    }>;
}
```

- `before`: the version row fields (null for version-0 creations)
- `after`: the next version's fields for that chunk, or the chunk's current live state if this is the most recent version

**Tag listing endpoint** — `GET /api/chunks/updates/tags?codebaseId=<optional>` returns distinct update tags with counts:

```typescript
{ tags: Array<{ tag: string; count: number }> }
```

#### Files

- `packages/api/src/chunks/routes.ts` — add `updateTag` to PATCH/POST body schemas, add new query endpoints
- `packages/db/src/repository/chunk-version.ts` — add query functions for tagged versions

### 3. Service layer

**`updateChunk`** — accepts optional `updateTag?: string`. Passes it to `createVersion()`. The version snapshot now also captures `scope`, `rationale`, `alternatives`, `consequences` from the existing chunk state.

**`createChunk`** — accepts optional `updateTag?: string`. When provided, creates a version-0 row (before fields null, tag stored) after chunk creation.

**`listUpdatesByTag(userId, tag, codebaseId?)`** — new service function. Queries versions with the given tag, filtered to chunks owned by (or accessible to) the user via a join through `chunk.userId`. For each, computes `after` by either:
- Finding the next version row for the same chunk (via `LEAD` window function or a correlated subquery)
- Falling back to the chunk's current state if this is the most recent version

**`listUpdateTags(userId, codebaseId?)`** — returns distinct tags with counts from the user's chunk versions.

#### Files

- `packages/api/src/chunks/service.ts` — modify `updateChunk`, `createChunk`; add `listUpdatesByTag`, `listUpdateTags`
- `packages/db/src/repository/chunk-version.ts` — add `createVersion` with expanded fields, `getVersionsByTag`, `getDistinctTags`

### 4. Chunk creation tagging

When `POST /api/chunks` includes an `updateTag`, after the chunk is created:
- A version row is inserted with `version: 0`, all snapshot fields null, and the tag
- The query endpoint treats version-0 rows as "created" events (before is empty, after is the chunk's initial state)

This ensures `GET /api/chunks/updates?tag=feature-x` shows both created and modified chunks.

### 5. CLI and MCP support

**CLI commands:**
- `fubbik update <id> --tag feature-x` — passes tag to PATCH endpoint
- `fubbik add --tag feature-x` / `fubbik quick "title" --tag feature-x` — passes tag to POST endpoint
- `fubbik updates --tag feature-x` — lists all updates with that tag (calls query endpoint), shows chunk title + diff summary
- `fubbik updates --tags` — lists all distinct update tags with counts

**MCP tools:**
- `update_chunk` — gains optional `updateTag` parameter
- `create_chunk` — gains optional `updateTag` parameter
- New `list_updates` tool — accepts `tag` parameter, returns tagged updates for current codebase

#### Files

- `apps/cli/src/commands/update.ts` — add `--tag` option
- `apps/cli/src/commands/add.ts` — add `--tag` option
- `apps/cli/src/commands/quick.ts` — add `--tag` option
- Create: `apps/cli/src/commands/updates.ts` — new command for querying tagged updates
- `packages/mcp/src/tools.ts` — add `updateTag` to update/create tools, add `list_updates` tool

## Testing

- **Version creation tests:** Verify `updateTag` is stored on the version row when provided. Verify expanded fields (scope, rationale, etc.) are captured.
- **Creation tagging tests:** Verify version-0 row created with null before-fields and tag stored.
- **Query tests:** Verify `listUpdatesByTag` returns correct before/after pairs. Verify version-0 entries show null before. Verify cross-codebase querying works. Verify codebaseId filter narrows results.
- **Tag listing tests:** Verify distinct tags with correct counts. Verify codebaseId filtering.
- **CLI tests:** Verify `--tag` flag passes through to API. Verify `updates` command output format.
