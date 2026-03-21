# Embedding Refresh on Edit Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ensure chunk embeddings stay fresh by reliably regenerating them when content changes, with visibility into embedding status.

**Architecture:** The update flow already triggers `enrichChunk()` fire-and-forget when title/content changes. The gaps are: (1) no visibility when enrichment fails silently, (2) no way to see which chunks have stale embeddings, (3) no retry mechanism. Add an `embeddingUpdatedAt` column, track enrichment failures, and surface stale embeddings in health checks.

**Tech Stack:** Drizzle ORM, Effect, Elysia, pgvector

**Key insight:** `updateChunk` in `service.ts` already calls `enrichChunk()` on title/content changes. The issue is fire-and-forget with `.catch(() => {})` — failures are silently swallowed.

---

## File Structure

### Files to modify:
- `packages/db/src/schema/chunk.ts` — Add `embeddingUpdatedAt` column
- `packages/db/src/repository/chunk.ts` — Update enrichment to set timestamp
- `packages/api/src/enrich/service.ts` — Track failures, set timestamp on success
- `packages/api/src/chunks/service.ts` — Log enrichment failures instead of silencing
- `packages/api/src/knowledge-health/service.ts` — Add stale embedding detection
- `packages/db/src/repository/knowledge-health.ts` — Query for stale embeddings
- `apps/web/src/routes/knowledge-health.tsx` — Show stale embeddings section

---

## Task 1: Add embeddingUpdatedAt Column

**Files:**
- Modify: `packages/db/src/schema/chunk.ts`

- [ ] **Step 1: Read chunk schema**

Read `packages/db/src/schema/chunk.ts` to find where embedding-related columns are.

- [ ] **Step 2: Add column**

Add after the existing `embedding` column:
```ts
embeddingUpdatedAt: timestamp("embedding_updated_at"),
```

No default — null means "never embedded" or "embedding predates tracking."

- [ ] **Step 3: Push schema**

Run: `pnpm db:push`

- [ ] **Step 4: Commit**

```bash
git commit -m "feat: add embeddingUpdatedAt column to chunk table"
```

---

## Task 2: Track Embedding Freshness in Enrichment

**Files:**
- Modify: `packages/api/src/enrich/service.ts`
- Modify: `packages/db/src/repository/chunk.ts`

- [ ] **Step 1: Read enrichment service**

Read `packages/api/src/enrich/service.ts` — understand `enrichChunk` and `updateChunkEnrichment`.

- [ ] **Step 2: Set embeddingUpdatedAt on successful enrichment**

In `packages/db/src/repository/chunk.ts`:

**First**, add `embeddingUpdatedAt?: Date` to the `EnrichChunkParams` interface — otherwise TypeScript will error when setting it on the update clause.

**Then**, modify `updateChunkEnrichment` to also set `embeddingUpdatedAt: new Date()` when embedding is provided:

```ts
// In EnrichChunkParams interface, add:
embeddingUpdatedAt?: Date;

// In the function body:
if (params.embedding) {
    setClause.embedding = params.embedding;
    setClause.embeddingUpdatedAt = new Date();
}
```

- [ ] **Step 3: Improve error handling in chunk service**

In `packages/api/src/chunks/service.ts`, replace the silent catch in `updateChunk`:
```ts
// Before (silent):
Effect.runPromise(enrichChunk(chunkId)).catch(() => {});

// After (logged):
Effect.runPromise(enrichChunk(chunkId)).catch((err) => {
    console.error(`[enrich] Failed to re-enrich chunk ${chunkId}:`, err);
});
```

Same for `createChunk`:
```ts
Effect.runPromise(enrichChunkIfEmpty(id)).catch((err) => {
    console.error(`[enrich] Failed to enrich new chunk ${id}:`, err);
});
```

- [ ] **Step 4: Commit**

```bash
git commit -m "feat: track embeddingUpdatedAt and log enrichment failures"
```

---

## Task 3: Stale Embedding Health Check

**Files:**
- Modify: `packages/db/src/repository/knowledge-health.ts`
- Modify: `packages/api/src/knowledge-health/service.ts`
- Modify: `apps/web/src/routes/knowledge-health.tsx`

- [ ] **Step 1: Add stale embedding query**

In `packages/db/src/repository/knowledge-health.ts`, add a function that finds chunks where `updatedAt > embeddingUpdatedAt` (content changed after last embedding):

```ts
export function getStaleEmbeddings(userId: string, codebaseId?: string) {
    return Effect.tryPromise({
        try: async () => {
            const conditions = [
                eq(chunk.userId, userId),
                isNotNull(chunk.embedding),
                sql`${chunk.updatedAt} > ${chunk.embeddingUpdatedAt}`,
            ];
            // add codebase filtering if needed (same pattern as other health queries)

            const results = await db
                .select({
                    id: chunk.id,
                    title: chunk.title,
                    type: chunk.type,
                    updatedAt: chunk.updatedAt,
                    embeddingUpdatedAt: chunk.embeddingUpdatedAt,
                })
                .from(chunk)
                .where(and(...conditions))
                .orderBy(desc(chunk.updatedAt))
                .limit(50);

            return { chunks: results, count: results.length };
        },
        catch: cause => new DatabaseError({ cause }),
    });
}
```

- [ ] **Step 2: Add to health service**

In `packages/api/src/knowledge-health/service.ts`, add `staleEmbeddings` to the `Effect.all()`:

```ts
staleEmbeddings: getStaleEmbeddings(userId, codebaseId),
```

- [ ] **Step 3: Add to health page**

In `apps/web/src/routes/knowledge-health.tsx`, add a "Stale Embeddings" card section. Each item shows chunk title + how long since embedding was last updated. Add a "Re-enrich" button that calls `POST /chunks/:id/enrich`.

Read existing health sections for the exact card pattern.

- [ ] **Step 4: Commit**

```bash
git commit -m "feat: detect and surface stale embeddings in knowledge health"
```
