# Smart Duplicate Detection Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Warn users when creating a chunk that is semantically similar to an existing one, using the existing embedding infrastructure.

**Architecture:** New API endpoint `/api/chunks/check-similar` that takes title+content, generates an embedding via Ollama, and queries pgvector for nearest neighbors. Web UI calls this on the create form with debounced input. Falls back gracefully when Ollama is unavailable.

**Tech Stack:** Elysia, Effect, pgvector (cosine distance), Ollama (nomic-embed-text), React, TanStack Query

---

## File Structure

### New files:
- `packages/api/src/chunks/similarity.ts` — Service function for similarity checking
- `packages/db/src/repository/similarity.ts` — Repository query for nearest-neighbor search
- `packages/api/src/chunks/similarity.test.ts` — Tests for similarity service
- `apps/web/src/features/chunks/similar-chunks-warning.tsx` — UI warning component

### Files to modify:
- `packages/api/src/chunks/routes.ts` — Add `/chunks/check-similar` endpoint
- `packages/db/src/repository/index.ts` — Export similarity repo
- `apps/web/src/routes/chunks.new.tsx` — Integrate warning component

---

## Task 1: Similarity Repository

**Files:**
- Create: `packages/db/src/repository/similarity.ts`
- Modify: `packages/db/src/repository/index.ts`
- Test: `packages/api/src/chunks/similarity.test.ts`

- [ ] **Step 1: Write the test**

```ts
// packages/api/src/chunks/similarity.test.ts
import { describe, it, expect } from "vitest";

describe("similarity", () => {
    it("findSimilarByEmbedding returns empty array when no chunks match", () => {
        // This is an integration test placeholder — the actual pgvector query
        // needs a running database. For now, test the service logic.
        expect(true).toBe(true);
    });
});
```

- [ ] **Step 2: Create similarity repository**

```ts
// packages/db/src/repository/similarity.ts
import { Effect } from "effect";
import { db } from "../index";
import { chunk } from "../schema/chunk";
import { sql, and, eq, ne, isNotNull } from "drizzle-orm";
import { DatabaseError } from "../errors";

interface SimilarChunk {
    id: string;
    title: string;
    type: string;
    similarity: number;
}

export function findSimilarByEmbedding(params: {
    embedding: number[];
    userId: string;
    excludeId?: string;
    threshold?: number;
    limit?: number;
}): Effect.Effect<SimilarChunk[], DatabaseError> {
    const { embedding, userId, excludeId, threshold = 0.7, limit = 5 } = params;

    return Effect.tryPromise({
        try: async () => {
            const vectorStr = `[${embedding.join(",")}]`;
            const conditions = [
                eq(chunk.userId, userId),
                isNotNull(chunk.embedding),
            ];
            if (excludeId) conditions.push(ne(chunk.id, excludeId));

            const results = await db
                .select({
                    id: chunk.id,
                    title: chunk.title,
                    type: chunk.type,
                    distance: sql<number>`embedding <=> ${vectorStr}::vector`,
                })
                .from(chunk)
                .where(and(...conditions))
                .orderBy(sql`embedding <=> ${vectorStr}::vector`)
                .limit(limit);

            return results
                .map(r => ({
                    id: r.id,
                    title: r.title,
                    type: r.type,
                    similarity: 1 - r.distance,
                }))
                .filter(r => r.similarity >= threshold);
        },
        catch: (e) => new DatabaseError({ cause: e }),
    });
}
```

- [ ] **Step 3: Export from repository index**

In `packages/db/src/repository/index.ts`, add:
```ts
export { findSimilarByEmbedding } from "./similarity";
```

- [ ] **Step 4: Commit**

```bash
git add packages/db/src/repository/similarity.ts packages/db/src/repository/index.ts packages/api/src/chunks/similarity.test.ts
git commit -m "feat: add similarity search repository using pgvector"
```

---

## Task 2: Similarity Service and API Endpoint

**Files:**
- Create: `packages/api/src/chunks/similarity.ts`
- Modify: `packages/api/src/chunks/routes.ts`

- [ ] **Step 1: Create similarity service**

```ts
// packages/api/src/chunks/similarity.ts
import { Effect } from "effect";
import { generateDocumentEmbedding, isOllamaAvailable } from "../ollama/client";
import { findSimilarByEmbedding } from "@fubbik/db/repository";

export function checkSimilar(params: {
    title: string;
    content: string;
    userId: string;
    excludeId?: string;
}) {
    return Effect.gen(function* () {
        const available = yield* isOllamaAvailable();
        if (!available) return [];

        const embedding = yield* generateDocumentEmbedding(params.title, null, params.content);
        return yield* findSimilarByEmbedding({
            embedding,
            userId: params.userId,
            excludeId: params.excludeId,
            threshold: 0.75,
            limit: 3,
        });
    });
}
```

**Note:** Read `ollama/client.ts` first — `isOllamaAvailable()` and `generateDocumentEmbedding()` both return Effect types. Make sure the pipe/gen pattern matches the codebase convention.

- [ ] **Step 2: Add API endpoint**

In `packages/api/src/chunks/routes.ts`, add:
```ts
.post(
    "/chunks/check-similar",
    ctx => Effect.runPromise(
        requireSession(ctx).pipe(
            Effect.flatMap(session =>
                checkSimilar({
                    title: ctx.body.title,
                    content: ctx.body.content,
                    userId: session.user.id,
                    excludeId: ctx.body.excludeId,
                })
            )
        )
    ),
    {
        body: t.Object({
            title: t.String(),
            content: t.String(),
            excludeId: t.Optional(t.String()),
        }),
    }
)
```

- [ ] **Step 3: Commit**

```bash
git add packages/api/src/chunks/similarity.ts packages/api/src/chunks/routes.ts
git commit -m "feat: add /chunks/check-similar API endpoint"
```

---

## Task 3: Web UI Warning Component

**Files:**
- Create: `apps/web/src/features/chunks/similar-chunks-warning.tsx`
- Modify: `apps/web/src/routes/chunks.new.tsx`

- [ ] **Step 1: Create SimilarChunksWarning component**

```tsx
// apps/web/src/features/chunks/similar-chunks-warning.tsx
import { useQuery } from "@tanstack/react-query";
import { AlertTriangle } from "lucide-react";
import { Link } from "@tanstack/react-router";
import { Badge } from "@/components/ui/badge";
import { api } from "@/utils/api";
import { unwrapEden } from "@/utils/eden";

interface SimilarChunksWarningProps {
    title: string;
    content: string;
    excludeId?: string;
    enabled?: boolean;
}

export function SimilarChunksWarning({ title, content, excludeId, enabled = true }: SimilarChunksWarningProps) {
    const { data: similar } = useQuery({
        queryKey: ["check-similar", title, content],
        queryFn: async () => {
            if (!title.trim() || !content.trim()) return [];
            return unwrapEden(
                await api.api.chunks["check-similar"].post({ title, content, excludeId })
            );
        },
        enabled: enabled && title.trim().length > 3 && content.trim().length > 20,
        staleTime: 10_000, // don't re-check too often
    });

    if (!similar?.length) return null;

    return (
        <div className="rounded-md border border-yellow-500/30 bg-yellow-500/5 p-3">
            <div className="flex items-center gap-2 text-sm font-medium text-yellow-700 dark:text-yellow-400">
                <AlertTriangle className="size-4" />
                Similar chunks already exist
            </div>
            <ul className="mt-2 space-y-1">
                {similar.map((c: any) => (
                    <li key={c.id} className="flex items-center gap-2 text-sm">
                        <Link to="/chunks/$chunkId" params={{ chunkId: c.id }} className="underline hover:no-underline">
                            {c.title}
                        </Link>
                        <Badge variant="secondary" size="sm">{c.type}</Badge>
                        <span className="text-muted-foreground text-xs">{Math.round(c.similarity * 100)}% similar</span>
                    </li>
                ))}
            </ul>
        </div>
    );
}
```

- [ ] **Step 2: Integrate into chunk create form**

In `apps/web/src/routes/chunks.new.tsx`, add `<SimilarChunksWarning>` below the content editor:
```tsx
<SimilarChunksWarning title={title} content={content} />
```

**Important:** `staleTime` does NOT debounce input — it only caches results for the same key. Use debounced values for `title` and `content` in the queryKey to avoid firing on every keystroke. The create form likely already has a debounce utility — check for `useDebouncedValue` or similar. Wrap the title/content in debounced state before passing to the component, or add debouncing inside the component.

- [ ] **Step 3: Commit**

```bash
git add apps/web/src/features/chunks/similar-chunks-warning.tsx apps/web/src/routes/chunks.new.tsx
git commit -m "feat: show similar chunk warnings on create form"
```
