# Cross-Codebase Chunk References Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Let users explicitly reference chunks from other codebases, making cross-project knowledge discoverable.

**Architecture:** Connections (`chunkConnection`) already work across codebases — there's no codebase FK on the connection table. The missing piece is UI surfacing: when viewing a chunk in codebase A, connections to chunks in codebase B should show the source codebase. Also add a "Reference from another codebase" search that spans all codebases.

**Tech Stack:** Drizzle ORM, Effect, Elysia, React, Eden treaty

**Key insight:** Cross-codebase connections already work at the data layer. This is primarily a UI/discovery feature, not a schema change.

---

## File Structure

### Files to modify:
- `packages/api/src/chunks/service.ts` — Enrich connection data with codebase info
- `packages/db/src/repository/chunk.ts (where `getChunkConnections` lives — NOT in connection.ts)` — Add codebase info to connection queries
- `apps/web/src/routes/chunks.$chunkId.tsx` — Show codebase badges on cross-codebase connections
- `apps/web/src/features/chunks/link-chunk-dialog.tsx` — Add "Search all codebases" toggle
- `packages/api/src/chunks/routes.ts` — Add `allCodebases` query param to search

---

## Task 1: Enrich Connections with Codebase Info

**Files:**
- Modify: `packages/db/src/repository/chunk.ts (where `getChunkConnections` lives — NOT in connection.ts)`
- Modify: `packages/api/src/chunks/service.ts`

- [ ] **Step 1: Read existing connection query**

Read `packages/db/src/repository/chunk.ts (where `getChunkConnections` lives — NOT in connection.ts)` — find `getChunkConnections` or similar. Understand what's returned for each connection.

- [ ] **Step 2: Add codebase info to connection results**

Modify the connection query to join with `chunkCodebase` + `codebase` tables and return the codebase name for each connected chunk:

```ts
// In the connection query, add a left join:
.leftJoin(chunkCodebase, eq(chunkCodebase.chunkId, /* connected chunk id */))
.leftJoin(codebase, eq(codebase.id, chunkCodebase.codebaseId))
// Add to select: codebaseName: codebase.name
```

The result should include `codebaseName: string | null` for each connection. Global chunks will have `null`.

- [ ] **Step 3: Commit**

```bash
git commit -m "feat: include codebase name in connection query results"
```

---

## Task 2: Show Cross-Codebase Badges on Chunk Detail

**Files:**
- Modify: `apps/web/src/routes/chunks.$chunkId.tsx`

- [ ] **Step 1: Read the connections section**

Find where connections are rendered on the chunk detail page. Each connection shows the linked chunk's title and relation type.

- [ ] **Step 2: Add codebase badge for cross-codebase connections**

When a connected chunk belongs to a different codebase than the current one, show a small badge:

```tsx
{connection.codebaseName && connection.codebaseName !== currentCodebaseName && (
    <Badge variant="outline" size="sm" className="text-[10px]">
        {connection.codebaseName}
    </Badge>
)}
```

This makes it visually clear when a connection crosses codebase boundaries.

- [ ] **Step 3: Commit**

```bash
git commit -m "feat(web): show codebase badges on cross-codebase connections"
```

---

## Task 3: Cross-Codebase Search in Link Dialog

**Files:**
- Modify: `apps/web/src/features/chunks/link-chunk-dialog.tsx`
- Modify: `packages/api/src/chunks/routes.ts`

- [ ] **Step 1: Add `allCodebases` param to chunk list API**

In `packages/api/src/chunks/routes.ts`, add an `allCodebases` query param to `GET /chunks`. When set, skip the codebase filter so the search spans all codebases:

```ts
allCodebases: t.Optional(t.String()),
```

In the service, when `allCodebases === "true"`, don't pass `codebaseId` to the repo.

- [ ] **Step 2: Add toggle in link dialog**

In `link-chunk-dialog.tsx`, add a checkbox/toggle "Search all codebases" above the search input. When checked, pass `allCodebases: "true"` to the search API call. Show codebase name in search results.

- [ ] **Step 3: Commit**

```bash
git commit -m "feat: cross-codebase search in link chunk dialog"
```
