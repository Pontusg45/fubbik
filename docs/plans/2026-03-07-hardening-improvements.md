# Hardening & Quality Improvements Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Harden the codebase with input validation, self-link prevention, env config for AI/rate-limits, test coverage for services, and
extract large components from the chunk detail page.

**Architecture:** Each task is independent. Backend tasks add Elysia body validation constraints and service-layer guards. The test task
adds unit tests for chunk and connection services using the existing Eden treaty test pattern. The frontend task extracts inline components
to `features/` directories. The env task centralizes hardcoded config into `packages/env`.

**Tech Stack:** TypeScript, Effect, Elysia (t.String with maxLength), Vitest, Arktype, @t3-oss/env-core

---

### Task 1: Backend Input Validation

Add length constraints to all Elysia body schemas so oversized payloads are rejected before hitting the DB. Also prevent self-linking in
connections.

**Files:**

- Modify: `packages/api/src/chunks/routes.ts`
- Modify: `packages/api/src/connections/routes.ts`
- Modify: `packages/api/src/connections/service.ts`
- Modify: `packages/api/src/errors.ts`

**Step 1: Add ValidationError to errors.ts**

In `packages/api/src/errors.ts`, add after the existing errors:

```typescript
export class ValidationError extends Data.TaggedError("ValidationError")<{
    message: string;
}> {}
```

**Step 2: Handle ValidationError in the API error handler**

In `packages/api/src/index.ts`, add a case in the `switch (effectError._tag)` block, before `"NotFoundError"`:

```typescript
case "ValidationError":
    set.status = 400;
    return { message: effectError.message as string };
```

**Step 3: Add length constraints to chunk routes**

In `packages/api/src/chunks/routes.ts`, update ALL body schemas for POST `/chunks`, POST `/chunks/import`, and PATCH `/chunks/:id`.

For POST `/chunks` (line 72-79), replace:

```typescript
body: t.Object({
    title: t.String(),
    content: t.Optional(t.String()),
    type: t.Optional(t.String()),
    tags: t.Optional(t.Array(t.String()))
});
```

with:

```typescript
body: t.Object({
    title: t.String({ maxLength: 200 }),
    content: t.Optional(t.String({ maxLength: 50000 })),
    type: t.Optional(t.String({ maxLength: 20 })),
    tags: t.Optional(t.Array(t.String({ maxLength: 50 }), { maxItems: 20 }))
});
```

For PATCH `/chunks/:id` (line 88-93), apply the same constraints.

For POST `/chunks/import` (line 37-46), add `{ maxItems: 500 }` on the outer Array and the same field constraints inside:

```typescript
body: t.Object({
    chunks: t.Array(
        t.Object({
            title: t.String({ maxLength: 200 }),
            content: t.Optional(t.String({ maxLength: 50000 })),
            type: t.Optional(t.String({ maxLength: 20 })),
            tags: t.Optional(t.Array(t.String({ maxLength: 50 }), { maxItems: 20 }))
        }),
        { maxItems: 500 }
    )
});
```

**Step 4: Add constraints to connection routes**

In `packages/api/src/connections/routes.ts`, replace the POST body (line 22-26):

```typescript
body: t.Object({
    sourceId: t.String({ maxLength: 100 }),
    targetId: t.String({ maxLength: 100 }),
    relation: t.String({ maxLength: 50 })
});
```

**Step 5: Add self-link guard in connection service**

In `packages/api/src/connections/service.ts`, add at the top of `createConnection`, before the `getChunkById` call:

```typescript
export function createConnection(userId: string, body: { sourceId: string; targetId: string; relation: string }) {
    if (body.sourceId === body.targetId) {
        return Effect.fail(new ValidationError({ message: "Cannot connect a chunk to itself" }));
    }
    return getChunkById(body.sourceId, userId).pipe(
```

Add the import:

```typescript
import { NotFoundError, ValidationError } from "../errors";
```

**Step 6: Run type check**

Run: `cd /Users/pontus/GitHub/fubbik && bun tsc --noEmit` Expected: 0 errors

**Step 7: Commit**

```bash
git add packages/api/src/errors.ts packages/api/src/index.ts \
  packages/api/src/chunks/routes.ts packages/api/src/connections/routes.ts \
  packages/api/src/connections/service.ts
git commit -m "feat: add backend input validation and self-link prevention"
```

---

### Task 2: Environment Config for AI and Rate Limits

Move hardcoded OpenAI model and rate limit values to env vars.

**Files:**

- Modify: `packages/env/src/server.ts`
- Modify: `packages/api/src/ai/service.ts`
- Modify: `apps/server/src/index.ts`

**Step 1: Add env vars to server env**

In `packages/env/src/server.ts`, add to the `server` object:

```typescript
OPENAI_API_KEY: type("string >= 1").optional(),
OPENAI_MODEL: type("string >= 1").optional(),
RATE_LIMIT_MAX: type("string >= 1").optional(),
RATE_LIMIT_DURATION_MS: type("string >= 1").optional(),
```

And add to `runtimeEnv`:

```typescript
OPENAI_API_KEY: process.env.OPENAI_API_KEY,
OPENAI_MODEL: process.env.OPENAI_MODEL,
RATE_LIMIT_MAX: process.env.RATE_LIMIT_MAX,
RATE_LIMIT_DURATION_MS: process.env.RATE_LIMIT_DURATION_MS,
```

**Step 2: Use env var for AI model**

In `packages/api/src/ai/service.ts`, replace the import section and add a model constant:

```typescript
import { env } from "@fubbik/env/server";
```

Add below imports:

```typescript
const AI_MODEL = env.OPENAI_MODEL ?? "gpt-4o-mini";
```

Replace all 4 occurrences of `openai("gpt-4o-mini")` with `openai(AI_MODEL)`.

**Step 3: Use env vars for rate limit**

In `apps/server/src/index.ts`, replace the hardcoded rate limit (line 21-24):

```typescript
.use(
    rateLimit({
        max: Number(env.RATE_LIMIT_MAX ?? "100"),
        duration: Number(env.RATE_LIMIT_DURATION_MS ?? "60000")
    })
)
```

**Step 4: Run type check**

Run: `cd /Users/pontus/GitHub/fubbik && bun tsc --noEmit` Expected: 0 errors

**Step 5: Commit**

```bash
git add packages/env/src/server.ts packages/api/src/ai/service.ts apps/server/src/index.ts
git commit -m "feat: move AI model and rate limit config to env vars"
```

---

### Task 3: Service Layer Tests

Add unit tests for chunk service and connection service. Use the existing Eden treaty test pattern from `packages/api/src/index.test.ts`.

**Files:**

- Create: `packages/api/src/chunks/service.test.ts`
- Create: `packages/api/src/connections/service.test.ts`

**Step 1: Create chunk service tests**

Create `packages/api/src/chunks/service.test.ts`:

```typescript
import { treaty } from "@elysiajs/eden";
import { Elysia } from "elysia";
import { describe, expect, it } from "vitest";

import { api } from "../index";

const app = new Elysia().use(api);
const client = treaty(app);

describe("Chunk routes — auth required", () => {
    it("GET /api/chunks/export returns 401 without auth", async () => {
        const { status } = await client.api.chunks.export.get();
        expect(status).toBe(401);
    });

    it("POST /api/chunks/import returns 401 without auth", async () => {
        const { status } = await client.api.chunks.import.post({
            chunks: [{ title: "test" }]
        });
        expect(status).toBe(401);
    });

    it("GET /api/chunks/:id/history returns 401 without auth", async () => {
        const { status } = await client.api.chunks({ id: "test" }).history.get();
        expect(status).toBe(401);
    });
});

describe("Chunk routes — validation", () => {
    // These test that Elysia rejects invalid bodies before hitting the service layer.
    // Note: Without auth, we get 401 first, so these tests verify the auth guard runs.
    // To test validation, we would need a dev-mode session. For now, verify 401.

    it("POST /api/chunks with empty title still requires auth", async () => {
        const { status } = await client.api.chunks.post({ title: "" });
        expect(status).toBe(401);
    });
});
```

**Step 2: Create connection service tests**

Create `packages/api/src/connections/service.test.ts`:

```typescript
import { treaty } from "@elysiajs/eden";
import { Elysia } from "elysia";
import { describe, expect, it } from "vitest";

import { api } from "../index";

const app = new Elysia().use(api);
const client = treaty(app);

describe("Connection routes — auth required", () => {
    it("POST /api/connections returns 401 without auth", async () => {
        const { status } = await client.api.connections.post({
            sourceId: "a",
            targetId: "b",
            relation: "related"
        });
        expect(status).toBe(401);
    });

    it("DELETE /api/connections/:id returns 401 without auth", async () => {
        const { status } = await client.api.connections({ id: "test" }).delete();
        expect(status).toBe(401);
    });
});

describe("AI routes — auth required", () => {
    it("POST /api/ai/summarize returns 401 without auth", async () => {
        const { status } = await client.api.ai.summarize.post({ chunkId: "test" });
        expect(status).toBe(401);
    });

    it("POST /api/ai/suggest-connections returns 401 without auth", async () => {
        const { status } = await client.api.ai["suggest-connections"].post({ chunkId: "test" });
        expect(status).toBe(401);
    });

    it("POST /api/ai/generate returns 401 without auth", async () => {
        const { status } = await client.api.ai.generate.post({ prompt: "test" });
        expect(status).toBe(401);
    });
});

describe("Graph & Tags routes — auth required", () => {
    it("GET /api/graph returns 401 without auth", async () => {
        const { status } = await client.api.graph.get();
        expect(status).toBe(401);
    });

    it("GET /api/tags returns 401 without auth", async () => {
        const { status } = await client.api.tags.get();
        expect(status).toBe(401);
    });
});
```

**Step 3: Run tests**

Run: `cd /Users/pontus/GitHub/fubbik && bun run --filter @fubbik/api test`

If the filter doesn't work, run: `cd packages/api && bun vitest run`

Expected: All tests pass (existing + new auth guard tests).

**Step 4: Commit**

```bash
git add packages/api/src/chunks/service.test.ts packages/api/src/connections/service.test.ts
git commit -m "test: add auth guard tests for all API routes"
```

---

### Task 4: Extract Chunk Detail Components

The chunk detail page (`chunks.$chunkId.tsx`) is 527 lines with 5 inline components. Extract them to `features/` directories.

**Files:**

- Create: `apps/web/src/features/chunks/ai-section.tsx`
- Create: `apps/web/src/features/chunks/link-chunk-dialog.tsx`
- Create: `apps/web/src/features/chunks/delete-connection-button.tsx`
- Create: `apps/web/src/features/chunks/version-history.tsx`
- Modify: `apps/web/src/routes/chunks.$chunkId.tsx`

**Step 1: Extract DeleteConnectionButton**

Create `apps/web/src/features/chunks/delete-connection-button.tsx`:

```typescript
import { useMutation, useQueryClient } from "@tanstack/react-query";
import { X } from "lucide-react";
import { toast } from "sonner";

import { api } from "@/utils/api";

export function DeleteConnectionButton({ connectionId, chunkId }: { connectionId: string; chunkId: string }) {
    const queryClient = useQueryClient();

    const deleteMutation = useMutation({
        mutationFn: async () => {
            const { error } = await api.api.connections({ id: connectionId }).delete();
            if (error) throw new Error("Failed to delete connection");
        },
        onSuccess: () => {
            queryClient.invalidateQueries({ queryKey: ["chunk", chunkId] });
            toast.success("Connection removed");
        },
        onError: () => {
            toast.error("Failed to remove connection");
        }
    });

    return (
        <button
            type="button"
            onClick={e => {
                e.preventDefault();
                e.stopPropagation();
                deleteMutation.mutate();
            }}
            disabled={deleteMutation.isPending}
            className="text-muted-foreground hover:text-destructive rounded p-0.5 transition-colors"
            aria-label="Remove connection"
        >
            <X className="size-3.5" />
        </button>
    );
}
```

**Step 2: Extract AiSection**

Create `apps/web/src/features/chunks/ai-section.tsx`. Copy the `AiSection` function (lines 256-362 of `chunks.$chunkId.tsx`) into a new file
with its own imports:

```typescript
import { useMutation } from "@tanstack/react-query";
import { Link } from "@tanstack/react-router";
import { Bot, Loader2, Network, Sparkles } from "lucide-react";
import { useState } from "react";
import { toast } from "sonner";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardDescription, CardHeader, CardPanel, CardTitle } from "@/components/ui/card";
import { api } from "@/utils/api";
```

Keep the component body identical. Export as a named export.

**Step 3: Extract LinkChunkDialog**

Create `apps/web/src/features/chunks/link-chunk-dialog.tsx`. Copy the `LinkChunkDialog` function (lines 364-459) with its imports:

```typescript
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { Link2, Search } from "lucide-react";
import { useState } from "react";
import { toast } from "sonner";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Dialog, DialogHeader, DialogPopup, DialogTitle, DialogTrigger } from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { api } from "@/utils/api";
```

**Step 4: Extract VersionHistory**

Create `apps/web/src/features/chunks/version-history.tsx`. Copy the `VersionHistory` component with its imports:

```typescript
import { useQuery } from "@tanstack/react-query";
import { History } from "lucide-react";
import { useState } from "react";

import { Badge } from "@/components/ui/badge";
import { Card, CardHeader, CardPanel, CardTitle } from "@/components/ui/card";
import { api } from "@/utils/api";
```

**Step 5: Update chunks.$chunkId.tsx**

Replace the inline component definitions with imports. The main file should drop from ~527 lines to ~220 lines. The import section becomes:

```typescript
import { AiSection } from "@/features/chunks/ai-section";
import { DeleteConnectionButton } from "@/features/chunks/delete-connection-button";
import { LinkChunkDialog } from "@/features/chunks/link-chunk-dialog";
import { VersionHistory } from "@/features/chunks/version-history";
```

Remove the following inline functions from the file:

- `DeleteConnectionButton` (lines 222-254)
- `AiSection` (lines 256-362)
- `LinkChunkDialog` (lines 364-459)
- `VersionHistory` (any inline version history component)

Remove now-unused imports from the main file: `Bot`, `Loader2`, `Sparkles`, `Search`, `Link2`, `Dialog*`, `Input`, and `useState` (if no
longer used in main component).

**Step 6: Run type check**

Run: `cd /Users/pontus/GitHub/fubbik && bun tsc --noEmit` Expected: 0 errors

**Step 7: Commit**

```bash
git add apps/web/src/features/chunks/ apps/web/src/routes/chunks.\$chunkId.tsx
git commit -m "refactor: extract chunk detail components to features/"
```

---

### Task 5: Eden Type Helper

Replace the repeated `as Exclude<typeof data, { message: string }>` casts across all frontend query functions with a shared helper.

**Files:**

- Create: `apps/web/src/utils/eden.ts`
- Modify: `apps/web/src/routes/dashboard.tsx`
- Modify: `apps/web/src/routes/chunks.index.tsx`
- Modify: `apps/web/src/routes/chunks.new.tsx`
- Modify: `apps/web/src/routes/chunks.$chunkId.tsx`
- Modify: `apps/web/src/routes/graph.tsx`
- Modify: `apps/web/src/routes/tags.tsx`

**Step 1: Create the helper**

Create `apps/web/src/utils/eden.ts`:

```typescript
/**
 * Unwrap an Eden treaty response, throwing on error.
 * Removes the need for `as Exclude<typeof data, { message: string }>` everywhere.
 */
export function unwrapEden<T>(response: { data: T | { message: string } | null; error: unknown }): T {
    if (response.error) throw new Error("Request failed");
    return response.data as T;
}
```

**Step 2: Replace casts in each route file**

In each file, replace patterns like:

```typescript
const { data, error } = await api.api.chunks.get({ query: { limit: "5" } });
if (error) return null;
return data as Exclude<typeof data, { message: string }>;
```

with:

```typescript
return unwrapEden(await api.api.chunks.get({ query: { limit: "5" } }));
```

Add `import { unwrapEden } from "@/utils/eden"` to each modified file. Remove `Exclude` type-only imports where no longer needed.

Apply this to:

- `dashboard.tsx` — 3 query functions (health, stats, chunks)
- `chunks.index.tsx` — 1 query function
- `chunks.new.tsx` — any query/mutation
- `chunks.$chunkId.tsx` — chunk detail query
- `graph.tsx` — graph query
- `tags.tsx` — tags query

**Step 3: Run type check**

Run: `cd /Users/pontus/GitHub/fubbik && bun tsc --noEmit` Expected: 0 errors

**Step 4: Commit**

```bash
git add apps/web/src/utils/eden.ts apps/web/src/routes/
git commit -m "refactor: add unwrapEden helper to remove Exclude type casts"
```

---

### Task 6: Add GIN Index on Tags

Add a GIN index on the `chunk.tags` JSONB column to speed up `jsonb_array_elements_text` queries in the tags page.

**Files:**

- Modify: `packages/db/src/migrations/0002_add_trgm.sql`

**Step 1: Add index to the existing migration**

Append to `packages/db/src/migrations/0002_add_trgm.sql`:

```sql
-- GIN index for JSONB tags queries
CREATE INDEX IF NOT EXISTS chunk_tags_gin_idx ON chunk USING gin (tags);
```

**Step 2: Commit**

```bash
git add packages/db/src/migrations/0002_add_trgm.sql
git commit -m "perf: add GIN index on chunk.tags for faster tag queries"
```
