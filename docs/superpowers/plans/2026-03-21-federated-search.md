# Federated Search Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A single search that spans all codebases, showing which project each result comes from.

**Architecture:** New API endpoint `GET /api/chunks/search/federated` that queries across all codebases without the usual codebase filter, returning results annotated with their codebase name. Web UI adds a "Search all" toggle to the chunk list and command palette. Reuses existing `listChunks` repo by omitting the codebase filter.

**Tech Stack:** Drizzle ORM, Effect, Elysia, React, Eden treaty

**Key insight:** The `listChunks` repo already supports no-codebase-filter mode (just don't pass `codebaseId` or `globalOnly`). The main work is annotating results with codebase names and adding UI toggles.

---

## File Structure

### New files:
- `packages/api/src/chunks/federated-search.ts` — Service for federated search with codebase annotation

### Files to modify:
- `packages/api/src/chunks/routes.ts` — Add federated search endpoint
- `packages/db/src/repository/chunk.ts` — Add codebase name annotation to results
- `apps/web/src/routes/chunks.index.tsx` — Add "Search all codebases" toggle
- `apps/web/src/features/command-palette/command-palette.tsx` — Add federated search mode

---

## Task 1: Codebase-Annotated Chunk Query

**Files:**
- Modify: `packages/db/src/repository/chunk.ts`

- [ ] **Step 1: Read the existing listChunks function**

Understand how it returns chunks and where codebase filtering happens.

- [ ] **Step 2: Add codebase name annotation**

Create a new function `listChunksWithCodebase` (or modify `listChunks` to optionally annotate). After fetching chunks, join with `chunkCodebase` + `codebase` to get the codebase name for each chunk:

```ts
export function listChunksWithCodebase(params: Omit<ListChunksParams, "codebaseId" | "globalOnly">) {
    return Effect.tryPromise({
        try: async () => {
            // Same query as listChunks but without codebase filter
            // Then for each chunk, look up its codebase name
            // NOTE: Import getTableColumns from drizzle-orm — it's not imported in chunk.ts by default
            // import { getTableColumns } from "drizzle-orm";
            const chunks = await db.select({
                ...getTableColumns(chunk),
                codebaseName: codebase.name,
            })
            .from(chunk)
            .leftJoin(chunkCodebase, eq(chunkCodebase.chunkId, chunk.id))
            .leftJoin(codebase, eq(codebase.id, chunkCodebase.codebaseId))
            .where(/* userId + search + type + tags conditions */)
            .orderBy(/* sort */)
            .limit(params.limit ?? 20)
            .offset(params.offset ?? 0);

            // Chunks may appear multiple times if in multiple codebases
            // Deduplicate, keeping first codebase name
            const seen = new Map();
            for (const c of chunks) {
                if (!seen.has(c.id)) {
                    seen.set(c.id, { ...c, codebaseName: c.codebaseName ?? "Global" });
                }
            }
            const deduped = Array.from(seen.values());

            // IMPORTANT: Count must use a separate query WITHOUT the join
            // to avoid inflated totals from multi-codebase chunks.
            // Use: SELECT COUNT(DISTINCT chunk.id) FROM chunk WHERE <conditions>
            const [{ count: total }] = await db
                .select({ count: sql<number>`count(*)` })
                .from(chunk)
                .where(and(...baseConditions)); // same conditions minus the join

            return { chunks: deduped, total };
        },
        catch: cause => new DatabaseError({ cause }),
    });
}
```

Read existing `listChunks` carefully — replicate its search/filter/sort logic but skip codebase filter and add the join.

- [ ] **Step 3: Commit**

```bash
git commit -m "feat: add listChunksWithCodebase for federated search"
```

---

## Task 2: Federated Search API Endpoint

**Files:**
- Create: `packages/api/src/chunks/federated-search.ts`
- Modify: `packages/api/src/chunks/routes.ts`

- [ ] **Step 1: Create federated search service**

```ts
// packages/api/src/chunks/federated-search.ts
import { Effect } from "effect";
import { listChunksWithCodebase } from "@fubbik/db/repository";

export function federatedSearch(userId: string, query: {
    search?: string;
    type?: string;
    tags?: string;
    limit?: string;
    offset?: string;
    sort?: string;
}) {
    return listChunksWithCodebase({
        userId,
        search: query.search,
        type: query.type,
        tags: query.tags ? query.tags.split(",") : undefined,
        sort: (query.sort as any) ?? "updated",
        limit: Math.min(Number(query.limit) || 20, 50),
        offset: Number(query.offset) || 0,
    });
}
```

- [ ] **Step 2: Add route**

In `packages/api/src/chunks/routes.ts`, add before the `/:id` routes:

```ts
.get(
    "/chunks/search/federated",
    ctx => Effect.runPromise(
        requireSession(ctx).pipe(
            Effect.flatMap(session => federatedSearch(session.user.id, ctx.query))
        )
    ),
    {
        query: t.Object({
            search: t.Optional(t.String()),
            type: t.Optional(t.String()),
            tags: t.Optional(t.String()),
            limit: t.Optional(t.String()),
            offset: t.Optional(t.String()),
            sort: t.Optional(t.String()),
        }),
    }
)
```

- [ ] **Step 3: Commit**

```bash
git commit -m "feat: add /chunks/search/federated endpoint"
```

---

## Task 3: Web UI — "Search All Codebases" Toggle on Chunk List

**Files:**
- Modify: `apps/web/src/routes/chunks.index.tsx`

- [ ] **Step 1: Add toggle state**

Add `allCodebases` to the route's `validateSearch` schema. **Important:** The `validateSearch` return type is an explicit TypeScript interface — you must update both the return value AND the type annotation:

```ts
allCodebases: (search.allCodebases as string) || "",
```

Add a toggle button near the search input:

```tsx
<Button
    variant={allCodebases ? "default" : "outline"}
    size="sm"
    onClick={() => updateSearch({ allCodebases: allCodebases ? "" : "true" })}
>
    {allCodebases ? "All codebases" : "This codebase"}
</Button>
```

- [ ] **Step 2: Switch query when toggle is active**

When `allCodebases` is set, call the federated search endpoint instead of the normal chunks endpoint:

```tsx
const chunksQuery = useInfiniteQuery({
    queryKey: ["chunks", allCodebases ? "federated" : "normal", ...],
    queryFn: async ({ pageParam = 1 }) => {
        if (allCodebases) {
            return unwrapEden(await api.api.chunks.search.federated.get({ query: { search, type, tags, ... } }));
        }
        return unwrapEden(await api.api.chunks.get({ query: { ... } }));
    },
});
```

- [ ] **Step 3: Show codebase name on results**

When in federated mode, show a codebase badge on each chunk row:

```tsx
{allCodebases && chunk.codebaseName && (
    <Badge variant="outline" size="sm" className="text-[10px]">
        {chunk.codebaseName}
    </Badge>
)}
```

- [ ] **Step 4: Commit**

```bash
git commit -m "feat(web): add 'Search all codebases' toggle to chunk list"
```

---

## Task 4: Command Palette Federated Search

**Files:**
- Modify: `apps/web/src/features/command-palette/command-palette.tsx`

- [ ] **Step 1: Add federated mode**

When the query starts with `*` (e.g., `*auth middleware`), search across all codebases using the federated endpoint:

```tsx
const isFederatedSearch = query.startsWith("*");
const searchQuery = isFederatedSearch ? query.slice(1).trim() : query;

// When federated, call the federated search endpoint
const federatedQuery = useQuery({
    queryKey: ["command-palette-federated", searchQuery],
    queryFn: async () => {
        return unwrapEden(
            await api.api.chunks.search.federated.get({ query: { search: searchQuery, limit: "5" } })
        );
    },
    enabled: open && isFederatedSearch && searchQuery.length > 1,
});
```

Show results with codebase badges and a hint in the placeholder: "Search chunks (* for all codebases, # for tags)".

- [ ] **Step 2: Commit**

```bash
git commit -m "feat(web): federated search in command palette with * prefix"
```
