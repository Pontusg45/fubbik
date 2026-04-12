# Chunk Proposal Review Workflow Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a proposal system where AI agents suggest changes to existing chunks that are staged for human review before being applied.

**Architecture:** One new `chunk_proposal` table with JSONB `changes`. Backend service creates proposals, applies them via the existing `updateChunk` flow on approve, and marks rejected without touching the chunk. Review queue page at `/review` + inline section on chunk detail. New MCP tool `propose_chunk_update` separate from existing `update_chunk`.

**Tech Stack:** Drizzle ORM (PostgreSQL), Elysia + Effect (backend), Model Context Protocol SDK, TanStack Start + TanStack Query (web), shadcn-ui on base-ui, Tailwind CSS.

**Spec:** `docs/superpowers/specs/2026-04-12-chunk-proposal-review-design.md`

---

## File Structure

### Created

| Path | Responsibility |
|---|---|
| `packages/db/src/schema/chunk-proposal.ts` | Schema for `chunk_proposal` table |
| `packages/db/src/repository/chunk-proposal.ts` | Effect-based CRUD |
| `packages/api/src/proposals/service.ts` | Business logic: create, approve, reject, list, count |
| `packages/api/src/proposals/routes.ts` | Elysia route definitions |
| `apps/web/src/routes/review.tsx` | Review queue page |
| `apps/web/src/features/proposals/proposal-card.tsx` | Expandable proposal card with diff + actions |
| `apps/web/src/features/proposals/proposal-diff.tsx` | Field-by-field diff renderer |
| `apps/web/src/features/proposals/chunk-proposals-section.tsx` | Inline section for chunk detail |
| `apps/web/src/features/proposals/use-pending-proposal-count.tsx` | Hook for nav badge count |

### Modified

| Path | Change |
|---|---|
| `packages/db/src/schema/index.ts` | Add chunk-proposal export |
| `packages/db/src/repository/index.ts` | Add chunk-proposal export |
| `packages/api/src/index.ts` | Mount proposal routes |
| `packages/mcp/src/tools.ts` | Add `propose_chunk_update` tool |
| `apps/web/src/routes/__root.tsx` | Add nav badge for pending proposals |
| `apps/web/src/routes/chunks.$chunkId.tsx` | Mount ChunkProposalsSection |

---

### Task 1: Schema + Migration

**Files:**
- Create: `packages/db/src/schema/chunk-proposal.ts`
- Modify: `packages/db/src/schema/index.ts`
- Modify: `packages/db/src/repository/index.ts`

- [ ] **Step 1: Read reference files to confirm patterns**

Read `packages/db/src/schema/chunk.ts` (lines 1-30) for import paths. Read `packages/db/src/schema/plan.ts` (lines 1-20) for `$defaultFn` pattern on text PKs. Read `packages/db/src/schema/auth.ts` for the `user` table import path.

- [ ] **Step 2: Create `packages/db/src/schema/chunk-proposal.ts`**

```typescript
import { relations } from "drizzle-orm";
import { index, jsonb, pgTable, text, timestamp } from "drizzle-orm/pg-core";

import { chunk } from "./chunk";
import { user } from "./auth";

export const chunkProposal = pgTable(
    "chunk_proposal",
    {
        id: text("id").primaryKey().$defaultFn(() => crypto.randomUUID()),
        chunkId: text("chunk_id")
            .notNull()
            .references(() => chunk.id, { onDelete: "cascade" }),
        changes: jsonb("changes").notNull(),
        reason: text("reason"),
        status: text("status").notNull().default("pending"),
        // pending | approved | rejected | superseded
        proposedBy: text("proposed_by").notNull(),
        reviewedBy: text("reviewed_by").references(() => user.id, { onDelete: "set null" }),
        reviewedAt: timestamp("reviewed_at"),
        reviewNote: text("review_note"),
        createdAt: timestamp("created_at").notNull().defaultNow(),
    },
    table => [
        index("chunk_proposal_chunkId_idx").on(table.chunkId),
        index("chunk_proposal_status_idx").on(table.status),
        index("chunk_proposal_chunkId_status_idx").on(table.chunkId, table.status),
    ],
);

export const chunkProposalRelations = relations(chunkProposal, ({ one }) => ({
    chunk: one(chunk, { fields: [chunkProposal.chunkId], references: [chunk.id] }),
    reviewer: one(user, { fields: [chunkProposal.reviewedBy], references: [user.id] }),
}));

export type ChunkProposal = typeof chunkProposal.$inferSelect;
export type NewChunkProposal = typeof chunkProposal.$inferInsert;

export interface ProposedChanges {
    title?: string;
    content?: string;
    type?: string;
    tags?: string[];
    rationale?: string;
    alternatives?: string[];
    consequences?: string;
    scope?: Record<string, string>;
}
```

- [ ] **Step 3: Update `packages/db/src/schema/index.ts`**

Add at the appropriate position (alphabetical or after chunk exports):

```typescript
export * from "./chunk-proposal";
```

- [ ] **Step 4: Generate and apply the migration**

```bash
pnpm db:generate
```

Inspect the generated migration file. It should create the `chunk_proposal` table with all columns and indexes. If Drizzle prompts interactively, accept defaults.

Apply:

```bash
pnpm db:push
```

Verify:

```bash
psql "${DATABASE_URL:-postgres://localhost/fubbik}" -c "\d chunk_proposal"
```

Expected: table with columns `id`, `chunk_id`, `changes`, `reason`, `status`, `proposed_by`, `reviewed_by`, `reviewed_at`, `review_note`, `created_at`.

- [ ] **Step 5: Type check the db package**

Run: `pnpm --filter @fubbik/db run check-types 2>&1 | tail -20`

Expected: zero new errors.

- [ ] **Step 6: Commit**

```bash
git add packages/db/src/schema/chunk-proposal.ts packages/db/src/schema/index.ts packages/db/src/migrations/
git commit -m "feat(db): add chunk_proposal schema for AI review workflow

Co-Authored-By: Claude Opus 4.6 (1M context) <noreply@anthropic.com>"
```

---

### Task 2: Repository

**Files:**
- Create: `packages/db/src/repository/chunk-proposal.ts`
- Modify: `packages/db/src/repository/index.ts`

- [ ] **Step 1: Read the reference repository**

Read `packages/db/src/repository/chunk.ts` (lines 1-50) for the Effect pattern. Confirm: `DatabaseError` from `"../errors"`, `db` from `"../index"`, `Effect.tryPromise({ try, catch: e => new DatabaseError({ cause: e }) })`.

- [ ] **Step 2: Create `packages/db/src/repository/chunk-proposal.ts`**

```typescript
import { and, asc, count, desc, eq } from "drizzle-orm";
import { Effect } from "effect";

import { DatabaseError } from "../errors";
import { db } from "../index";
import {
    chunkProposal,
    type ChunkProposal,
    type NewChunkProposal,
} from "../schema/chunk-proposal";
import { chunk } from "../schema/chunk";

export function createProposal(input: NewChunkProposal): Effect.Effect<ChunkProposal, DatabaseError> {
    return Effect.tryPromise({
        try: async () => {
            const [row] = await db.insert(chunkProposal).values(input).returning();
            if (!row) throw new Error("Insert returned no row");
            return row;
        },
        catch: e => new DatabaseError({ cause: e }),
    });
}

export function getProposalById(id: string): Effect.Effect<ChunkProposal | null, DatabaseError> {
    return Effect.tryPromise({
        try: async () => {
            const [row] = await db.select().from(chunkProposal).where(eq(chunkProposal.id, id)).limit(1);
            return row ?? null;
        },
        catch: e => new DatabaseError({ cause: e }),
    });
}

export interface ListProposalsFilter {
    chunkId?: string;
    status?: string;
    limit?: number;
    offset?: number;
}

export function listProposals(filter: ListProposalsFilter): Effect.Effect<
    Array<ChunkProposal & { chunkTitle: string; chunkType: string }>,
    DatabaseError
> {
    return Effect.tryPromise({
        try: async () => {
            const conditions = [];
            if (filter.chunkId) conditions.push(eq(chunkProposal.chunkId, filter.chunkId));
            if (filter.status) conditions.push(eq(chunkProposal.status, filter.status));

            const rows = await db
                .select({
                    id: chunkProposal.id,
                    chunkId: chunkProposal.chunkId,
                    changes: chunkProposal.changes,
                    reason: chunkProposal.reason,
                    status: chunkProposal.status,
                    proposedBy: chunkProposal.proposedBy,
                    reviewedBy: chunkProposal.reviewedBy,
                    reviewedAt: chunkProposal.reviewedAt,
                    reviewNote: chunkProposal.reviewNote,
                    createdAt: chunkProposal.createdAt,
                    chunkTitle: chunk.title,
                    chunkType: chunk.type,
                })
                .from(chunkProposal)
                .innerJoin(chunk, eq(chunk.id, chunkProposal.chunkId))
                .where(conditions.length > 0 ? and(...conditions) : undefined)
                .orderBy(desc(chunkProposal.createdAt))
                .limit(filter.limit ?? 50)
                .offset(filter.offset ?? 0);

            return rows;
        },
        catch: e => new DatabaseError({ cause: e }),
    });
}

export function listProposalsForChunk(
    chunkId: string,
    status?: string,
): Effect.Effect<ChunkProposal[], DatabaseError> {
    return Effect.tryPromise({
        try: async () => {
            const conditions = [eq(chunkProposal.chunkId, chunkId)];
            if (status) conditions.push(eq(chunkProposal.status, status));

            return db
                .select()
                .from(chunkProposal)
                .where(and(...conditions))
                .orderBy(asc(chunkProposal.createdAt));
        },
        catch: e => new DatabaseError({ cause: e }),
    });
}

export function updateProposalStatus(
    id: string,
    status: string,
    reviewedBy: string,
    reviewNote?: string,
): Effect.Effect<ChunkProposal, DatabaseError> {
    return Effect.tryPromise({
        try: async () => {
            const [row] = await db
                .update(chunkProposal)
                .set({
                    status,
                    reviewedBy,
                    reviewedAt: new Date(),
                    reviewNote: reviewNote ?? null,
                })
                .where(eq(chunkProposal.id, id))
                .returning();
            if (!row) throw new Error("Proposal not found");
            return row;
        },
        catch: e => new DatabaseError({ cause: e }),
    });
}

export function getPendingCount(): Effect.Effect<number, DatabaseError> {
    return Effect.tryPromise({
        try: async () => {
            const [row] = await db
                .select({ count: count() })
                .from(chunkProposal)
                .where(eq(chunkProposal.status, "pending"));
            return row?.count ?? 0;
        },
        catch: e => new DatabaseError({ cause: e }),
    });
}
```

- [ ] **Step 3: Update `packages/db/src/repository/index.ts`**

Add:

```typescript
export * from "./chunk-proposal";
```

- [ ] **Step 4: Type check**

Run: `pnpm --filter @fubbik/db run check-types 2>&1 | tail -20`

Expected: zero new errors.

- [ ] **Step 5: Commit**

```bash
git add packages/db/src/repository/chunk-proposal.ts packages/db/src/repository/index.ts
git commit -m "feat(db): add chunk proposal repository

Co-Authored-By: Claude Opus 4.6 (1M context) <noreply@anthropic.com>"
```

---

### Task 3: Service + Routes

**Files:**
- Create: `packages/api/src/proposals/service.ts`
- Create: `packages/api/src/proposals/routes.ts`
- Modify: `packages/api/src/index.ts`

- [ ] **Step 1: Read reference patterns**

Read `packages/api/src/chunks/service.ts` (lines 230-280) to see how `updateChunk` works — it takes `(chunkId, userId, body)` where body includes any chunk fields plus `tags`. This is the function we call on approve.

Read `packages/api/src/chunks/routes.ts` (lines 1-40) for Elysia + requireSession pattern.

Read `packages/api/src/index.ts` to see where to add `.use(proposalRoutes)`.

- [ ] **Step 2: Create `packages/api/src/proposals/service.ts`**

```typescript
import { Effect } from "effect";

import * as proposalRepo from "@fubbik/db/repository/chunk-proposal";
import type { ProposedChanges } from "@fubbik/db/schema/chunk-proposal";

import { updateChunk } from "../chunks/service";
import { NotFoundError, ValidationError } from "../errors";

const VALID_STATUSES = ["pending", "approved", "rejected", "superseded"] as const;

export interface CreateProposalInput {
    changes: ProposedChanges;
    reason?: string;
}

export function createProposal(chunkId: string, proposedBy: string, input: CreateProposalInput) {
    return Effect.gen(function* () {
        if (!input.changes || Object.keys(input.changes).length === 0) {
            return yield* Effect.fail(new ValidationError({ message: "Changes must not be empty" }));
        }
        return yield* proposalRepo.createProposal({
            chunkId,
            changes: input.changes,
            reason: input.reason ?? null,
            proposedBy,
            status: "pending",
        });
    });
}

export function getProposal(proposalId: string) {
    return proposalRepo.getProposalById(proposalId).pipe(
        Effect.flatMap(p =>
            p ? Effect.succeed(p) : Effect.fail(new NotFoundError({ resource: "Proposal" })),
        ),
    );
}

export function listProposals(filter: {
    chunkId?: string;
    status?: string;
    limit?: number;
    offset?: number;
}) {
    if (filter.status && !VALID_STATUSES.includes(filter.status as any)) {
        return Effect.fail(new ValidationError({ message: `Invalid status: ${filter.status}` }));
    }
    return proposalRepo.listProposals({
        chunkId: filter.chunkId,
        status: filter.status ?? "pending",
        limit: filter.limit,
        offset: filter.offset,
    });
}

export function listProposalsForChunk(chunkId: string, status?: string) {
    return proposalRepo.listProposalsForChunk(chunkId, status);
}

export function approveProposal(proposalId: string, reviewerId: string, note?: string) {
    return Effect.gen(function* () {
        const proposal = yield* getProposal(proposalId);
        if (proposal.status !== "pending") {
            return yield* Effect.fail(
                new ValidationError({ message: `Proposal is already ${proposal.status}` }),
            );
        }
        const changes = proposal.changes as ProposedChanges;

        // Apply changes via the existing updateChunk flow
        // This creates a version, updates the chunk, and triggers re-enrichment
        yield* updateChunk(proposal.chunkId, reviewerId, {
            ...(changes.title !== undefined && { title: changes.title }),
            ...(changes.content !== undefined && { content: changes.content }),
            ...(changes.type !== undefined && { type: changes.type }),
            ...(changes.tags !== undefined && { tags: changes.tags }),
            ...(changes.rationale !== undefined && { rationale: changes.rationale }),
            ...(changes.alternatives !== undefined && { alternatives: changes.alternatives }),
            ...(changes.consequences !== undefined && { consequences: changes.consequences }),
            ...(changes.scope !== undefined && { scope: changes.scope }),
        });

        return yield* proposalRepo.updateProposalStatus(proposalId, "approved", reviewerId, note);
    });
}

export function rejectProposal(proposalId: string, reviewerId: string, note?: string) {
    return Effect.gen(function* () {
        const proposal = yield* getProposal(proposalId);
        if (proposal.status !== "pending") {
            return yield* Effect.fail(
                new ValidationError({ message: `Proposal is already ${proposal.status}` }),
            );
        }
        return yield* proposalRepo.updateProposalStatus(proposalId, "rejected", reviewerId, note);
    });
}

export interface BulkAction {
    proposalId: string;
    action: "approve" | "reject";
    note?: string;
}

export function bulkAction(actions: BulkAction[], reviewerId: string) {
    return Effect.gen(function* () {
        const results: Array<{ proposalId: string; status: string }> = [];
        for (const a of actions) {
            if (a.action === "approve") {
                yield* approveProposal(a.proposalId, reviewerId, a.note);
            } else {
                yield* rejectProposal(a.proposalId, reviewerId, a.note);
            }
            results.push({ proposalId: a.proposalId, status: a.action === "approve" ? "approved" : "rejected" });
        }
        return results;
    });
}

export function getPendingCount() {
    return proposalRepo.getPendingCount();
}
```

- [ ] **Step 3: Create `packages/api/src/proposals/routes.ts`**

```typescript
import { Elysia, t } from "elysia";
import { Effect } from "effect";

import { requireSession } from "../require-session";
import * as proposalService from "./service";

export const proposalRoutes = new Elysia()
    // Create a proposal for a chunk
    .post(
        "/api/chunks/:id/proposals",
        async ctx => {
            return await Effect.runPromise(
                requireSession(ctx).pipe(
                    Effect.flatMap(session =>
                        proposalService.createProposal(ctx.params.id, session.user.id, ctx.body),
                    ),
                ),
            );
        },
        {
            body: t.Object({
                changes: t.Any(),
                reason: t.Optional(t.String()),
            }),
        },
    )
    // List proposals for a chunk
    .get("/api/chunks/:id/proposals", async ctx => {
        return await Effect.runPromise(
            requireSession(ctx).pipe(
                Effect.flatMap(() =>
                    proposalService.listProposalsForChunk(ctx.params.id, ctx.query.status),
                ),
            ),
        );
    })
    // Global review queue
    .get(
        "/api/proposals",
        async ctx => {
            return await Effect.runPromise(
                requireSession(ctx).pipe(
                    Effect.flatMap(() =>
                        proposalService.listProposals({
                            chunkId: ctx.query.chunkId,
                            status: ctx.query.status,
                            limit: ctx.query.limit ? Number(ctx.query.limit) : undefined,
                            offset: ctx.query.offset ? Number(ctx.query.offset) : undefined,
                        }),
                    ),
                ),
            );
        },
        {
            query: t.Object({
                status: t.Optional(t.String()),
                chunkId: t.Optional(t.String()),
                limit: t.Optional(t.String()),
                offset: t.Optional(t.String()),
            }),
        },
    )
    // Single proposal detail
    .get("/api/proposals/:proposalId", async ctx => {
        return await Effect.runPromise(
            requireSession(ctx).pipe(
                Effect.flatMap(() => proposalService.getProposal(ctx.params.proposalId)),
            ),
        );
    })
    // Approve
    .post(
        "/api/proposals/:proposalId/approve",
        async ctx => {
            return await Effect.runPromise(
                requireSession(ctx).pipe(
                    Effect.flatMap(session =>
                        proposalService.approveProposal(
                            ctx.params.proposalId,
                            session.user.id,
                            ctx.body?.note,
                        ),
                    ),
                ),
            );
        },
        {
            body: t.Object({
                note: t.Optional(t.String()),
            }),
        },
    )
    // Reject
    .post(
        "/api/proposals/:proposalId/reject",
        async ctx => {
            return await Effect.runPromise(
                requireSession(ctx).pipe(
                    Effect.flatMap(session =>
                        proposalService.rejectProposal(
                            ctx.params.proposalId,
                            session.user.id,
                            ctx.body?.note,
                        ),
                    ),
                ),
            );
        },
        {
            body: t.Object({
                note: t.Optional(t.String()),
            }),
        },
    )
    // Bulk approve/reject
    .post(
        "/api/proposals/bulk",
        async ctx => {
            return await Effect.runPromise(
                requireSession(ctx).pipe(
                    Effect.flatMap(session =>
                        proposalService.bulkAction(ctx.body.actions, session.user.id),
                    ),
                ),
            );
        },
        {
            body: t.Object({
                actions: t.Array(
                    t.Object({
                        proposalId: t.String(),
                        action: t.Union([t.Literal("approve"), t.Literal("reject")]),
                        note: t.Optional(t.String()),
                    }),
                ),
            }),
        },
    )
    // Pending count
    .get("/api/proposals/count", async ctx => {
        return await Effect.runPromise(
            requireSession(ctx).pipe(Effect.flatMap(() => proposalService.getPendingCount())),
        );
    });
```

- [ ] **Step 4: Mount routes in `packages/api/src/index.ts`**

Add to the import block:

```typescript
import { proposalRoutes } from "./proposals/routes";
```

Add to the Elysia chain (after other `.use()` calls):

```typescript
    .use(proposalRoutes)
```

- [ ] **Step 5: Type check**

Run: `pnpm --filter @fubbik/api run check-types 2>&1 | grep -E "proposals/" | head -20`

Expected: zero errors. If `updateChunk` from chunks/service has a different signature, read it and adapt the `approveProposal` function.

- [ ] **Step 6: Commit**

```bash
git add packages/api/src/proposals/ packages/api/src/index.ts
git commit -m "feat(api): add proposal service and routes for chunk review workflow

Co-Authored-By: Claude Opus 4.6 (1M context) <noreply@anthropic.com>"
```

---

### Task 4: MCP Tool

**Files:**
- Modify: `packages/mcp/src/tools.ts`

- [ ] **Step 1: Read the current tools.ts to find where to add the new tool**

Read `packages/mcp/src/tools.ts` — find the `update_chunk` tool registration. The new tool goes next to it.

- [ ] **Step 2: Add `propose_chunk_update` tool**

Find the end of the `update_chunk` tool registration and add after it:

```typescript
    server.tool(
        "propose_chunk_update",
        "Propose changes to an existing chunk for human review. Changes are staged as a pending proposal — the chunk is NOT modified until a human approves.",
        {
            chunkId: z.string().describe("The chunk ID to propose changes for"),
            changes: z.object({
                title: z.string().optional(),
                content: z.string().optional(),
                type: z.string().optional(),
                tags: z.array(z.string()).optional(),
                rationale: z.string().optional(),
                alternatives: z.array(z.string()).optional(),
                consequences: z.string().optional(),
                scope: z.record(z.string()).optional(),
            }).describe("Only include fields you want to change"),
            reason: z.string().optional().describe("Why you're proposing this change"),
        },
        async ({ chunkId, changes, reason }) => {
            const proposal = await apiFetch(`/chunks/${chunkId}/proposals`, {
                method: "POST",
                body: JSON.stringify({ changes, reason }),
            });
            return {
                content: [{
                    type: "text" as const,
                    text: `Proposal created (pending review):\n${JSON.stringify(proposal, null, 2)}`,
                }],
            };
        },
    );
```

- [ ] **Step 3: Type check**

Run: `pnpm --filter @fubbik/mcp run check-types 2>&1 | tail -20`

Expected: zero new errors. If the tool registration pattern differs (e.g., 3 args instead of 4, or no description string), adapt to match the existing `update_chunk` registration.

- [ ] **Step 4: Commit**

```bash
git add packages/mcp/src/tools.ts
git commit -m "feat(mcp): add propose_chunk_update tool

Co-Authored-By: Claude Opus 4.6 (1M context) <noreply@anthropic.com>"
```

---

### Task 5: Review Queue Page + Proposal Components

**Files:**
- Create: `apps/web/src/features/proposals/proposal-diff.tsx`
- Create: `apps/web/src/features/proposals/proposal-card.tsx`
- Create: `apps/web/src/routes/review.tsx`

- [ ] **Step 1: Read reference patterns**

Read `apps/web/src/routes/plans.index.tsx` for the list page pattern (createFileRoute, useQuery, PageContainer).
Read `apps/web/src/components/ui/button.tsx` to confirm Button accepts `variant`, `size`, `render` props.

- [ ] **Step 2: Create `apps/web/src/features/proposals/proposal-diff.tsx`**

```typescript
interface ProposalDiffProps {
    currentChunk: { title: string; content: string; type: string; rationale?: string | null; [key: string]: unknown };
    changes: Record<string, unknown>;
}

const FIELD_LABELS: Record<string, string> = {
    title: "Title",
    content: "Content",
    type: "Type",
    tags: "Tags",
    rationale: "Rationale",
    alternatives: "Alternatives",
    consequences: "Consequences",
    scope: "Scope",
};

export function ProposalDiff({ currentChunk, changes }: ProposalDiffProps) {
    const changedFields = Object.keys(changes).filter(k => k in FIELD_LABELS);

    if (changedFields.length === 0) {
        return <div className="text-xs text-muted-foreground">No changes</div>;
    }

    return (
        <div className="space-y-3">
            {changedFields.map(field => (
                <div key={field} className="space-y-1">
                    <div className="text-[10px] font-semibold uppercase tracking-wider text-muted-foreground">
                        {FIELD_LABELS[field] ?? field}
                    </div>
                    <FieldDiff
                        field={field}
                        current={currentChunk[field]}
                        proposed={changes[field]}
                    />
                </div>
            ))}
        </div>
    );
}

function FieldDiff({ field, current, proposed }: { field: string; current: unknown; proposed: unknown }) {
    if (field === "tags") {
        const currentTags = (current as string[] | undefined) ?? [];
        const proposedTags = (proposed as string[]) ?? [];
        const added = proposedTags.filter(t => !currentTags.includes(t));
        const removed = currentTags.filter(t => !proposedTags.includes(t));
        const kept = currentTags.filter(t => proposedTags.includes(t));
        return (
            <div className="flex flex-wrap gap-1 text-xs">
                {kept.map(t => (
                    <span key={t} className="rounded bg-muted px-1.5 py-0.5">{t}</span>
                ))}
                {added.map(t => (
                    <span key={`+${t}`} className="rounded bg-emerald-500/15 border border-emerald-500/30 text-emerald-500 px-1.5 py-0.5">+ {t}</span>
                ))}
                {removed.map(t => (
                    <span key={`-${t}`} className="rounded bg-red-500/15 border border-red-500/30 text-red-500 px-1.5 py-0.5 line-through">− {t}</span>
                ))}
            </div>
        );
    }

    if (field === "alternatives") {
        const currentAlts = (current as string[] | undefined) ?? [];
        const proposedAlts = (proposed as string[]) ?? [];
        return (
            <div className="text-xs">
                <div className="text-red-500/80 line-through">{currentAlts.join(", ") || "(none)"}</div>
                <div className="text-emerald-500/80">{proposedAlts.join(", ") || "(none)"}</div>
            </div>
        );
    }

    if (field === "scope") {
        return (
            <div className="text-xs font-mono">
                <div className="text-red-500/80 line-through">{JSON.stringify(current ?? {})}</div>
                <div className="text-emerald-500/80">{JSON.stringify(proposed ?? {})}</div>
            </div>
        );
    }

    // Default: text diff for title, content, type, rationale, consequences
    const currentStr = String(current ?? "");
    const proposedStr = String(proposed ?? "");

    if (field === "content" && currentStr.length > 300) {
        // Truncated content diff
        return (
            <div className="space-y-1 text-xs">
                <details>
                    <summary className="cursor-pointer text-muted-foreground hover:text-foreground">
                        Content changed ({currentStr.length} → {proposedStr.length} chars)
                    </summary>
                    <div className="mt-1 space-y-1 rounded border p-2">
                        <div className="whitespace-pre-wrap text-red-500/80 line-through">{currentStr}</div>
                        <div className="whitespace-pre-wrap text-emerald-500/80">{proposedStr}</div>
                    </div>
                </details>
            </div>
        );
    }

    return (
        <div className="space-y-0.5 text-xs">
            <div className="whitespace-pre-wrap text-red-500/80 line-through">{currentStr || "(empty)"}</div>
            <div className="whitespace-pre-wrap text-emerald-500/80">{proposedStr || "(empty)"}</div>
        </div>
    );
}
```

- [ ] **Step 3: Create `apps/web/src/features/proposals/proposal-card.tsx`**

```typescript
import { useMutation, useQuery } from "@tanstack/react-query";
import { Link } from "@tanstack/react-router";
import { Check, ChevronDown, ChevronRight, X } from "lucide-react";
import { useState } from "react";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { api } from "@/utils/api";
import { unwrapEden } from "@/utils/eden";

import { ProposalDiff } from "./proposal-diff";

export interface Proposal {
    id: string;
    chunkId: string;
    changes: Record<string, unknown>;
    reason: string | null;
    status: string;
    proposedBy: string;
    reviewedBy: string | null;
    reviewedAt: string | null;
    reviewNote: string | null;
    createdAt: string;
    chunkTitle?: string;
    chunkType?: string;
}

export interface ProposalCardProps {
    proposal: Proposal;
    showChunkInfo?: boolean;
    onUpdate: () => void;
}

export function ProposalCard({ proposal, showChunkInfo = true, onUpdate }: ProposalCardProps) {
    const [expanded, setExpanded] = useState(false);

    // Fetch chunk detail for diff rendering when expanded
    const chunkQuery = useQuery({
        queryKey: ["chunk-for-diff", proposal.chunkId],
        queryFn: async () => unwrapEden(await (api.api as any).chunks[proposal.chunkId].get()),
        enabled: expanded,
    });

    const approveMutation = useMutation({
        mutationFn: async () =>
            unwrapEden(await (api.api as any).proposals[proposal.id].approve.post({})),
        onSuccess: () => onUpdate(),
    });

    const rejectMutation = useMutation({
        mutationFn: async () =>
            unwrapEden(await (api.api as any).proposals[proposal.id].reject.post({})),
        onSuccess: () => onUpdate(),
    });

    const changedFields = Object.keys(proposal.changes);
    const isPending = proposal.status === "pending";
    const age = getRelativeTime(proposal.createdAt);

    return (
        <div className="rounded-md border bg-card">
            <div className="flex items-start gap-3 p-3">
                <button type="button" onClick={() => setExpanded(e => !e)} className="mt-0.5 text-muted-foreground">
                    {expanded ? <ChevronDown className="size-4" /> : <ChevronRight className="size-4" />}
                </button>
                <div className="flex-1">
                    {showChunkInfo && (
                        <div className="mb-1 flex items-center gap-2">
                            <Link
                                to="/chunks/$chunkId"
                                params={{ chunkId: proposal.chunkId }}
                                className="font-medium hover:underline"
                            >
                                {proposal.chunkTitle ?? proposal.chunkId.slice(0, 8)}
                            </Link>
                            {proposal.chunkType && (
                                <Badge variant="secondary" size="sm">{proposal.chunkType}</Badge>
                            )}
                        </div>
                    )}
                    <div className="flex items-center gap-2 text-xs text-muted-foreground">
                        <span>{age}</span>
                        {proposal.reason && (
                            <>
                                <span>•</span>
                                <span className="truncate max-w-[300px]">{proposal.reason}</span>
                            </>
                        )}
                    </div>
                    <div className="mt-1 flex flex-wrap gap-1">
                        {changedFields.map(f => (
                            <span key={f} className="rounded bg-muted px-1.5 py-0.5 text-[10px] font-mono">
                                {f}
                            </span>
                        ))}
                    </div>
                </div>
                {isPending && (
                    <div className="flex gap-1">
                        <Button
                            size="sm"
                            variant="ghost"
                            onClick={() => approveMutation.mutate()}
                            disabled={approveMutation.isPending}
                            title="Approve"
                        >
                            <Check className="size-4 text-emerald-500" />
                        </Button>
                        <Button
                            size="sm"
                            variant="ghost"
                            onClick={() => rejectMutation.mutate()}
                            disabled={rejectMutation.isPending}
                            title="Reject"
                        >
                            <X className="size-4 text-red-500" />
                        </Button>
                    </div>
                )}
                {!isPending && (
                    <Badge variant={proposal.status === "approved" ? "default" : "secondary"} size="sm">
                        {proposal.status}
                    </Badge>
                )}
            </div>
            {expanded && (
                <div className="border-t px-3 py-3">
                    {chunkQuery.isLoading ? (
                        <div className="text-xs text-muted-foreground">Loading chunk...</div>
                    ) : chunkQuery.data ? (
                        <ProposalDiff
                            currentChunk={(chunkQuery.data as any).chunk ?? chunkQuery.data}
                            changes={proposal.changes}
                        />
                    ) : (
                        <div className="text-xs text-muted-foreground">Could not load chunk data</div>
                    )}
                </div>
            )}
        </div>
    );
}

function getRelativeTime(dateStr: string): string {
    const diff = Date.now() - new Date(dateStr).getTime();
    const mins = Math.floor(diff / 60000);
    if (mins < 60) return `${mins}m ago`;
    const hours = Math.floor(mins / 60);
    if (hours < 24) return `${hours}h ago`;
    const days = Math.floor(hours / 24);
    return `${days}d ago`;
}
```

- [ ] **Step 4: Create `apps/web/src/routes/review.tsx`**

```typescript
import { useMutation, useQuery } from "@tanstack/react-query";
import { createFileRoute } from "@tanstack/react-router";
import { useState } from "react";

import { Button } from "@/components/ui/button";
import { PageContainer, PageHeader, PageLoading } from "@/components/ui/page";
import { ProposalCard, type Proposal } from "@/features/proposals/proposal-card";
import { api } from "@/utils/api";
import { unwrapEden } from "@/utils/eden";

export const Route = createFileRoute("/review")({ component: ReviewPage });

function ReviewPage() {
    const [statusFilter, setStatusFilter] = useState<string>("pending");

    const proposalsQuery = useQuery({
        queryKey: ["proposals", statusFilter],
        queryFn: async () => {
            const params = new URLSearchParams();
            if (statusFilter !== "all") params.set("status", statusFilter);
            return unwrapEden(await (api.api as any).proposals.get({ query: Object.fromEntries(params) }));
        },
    });

    const countQuery = useQuery({
        queryKey: ["proposals-count"],
        queryFn: async () => unwrapEden(await (api.api as any).proposals.count.get()),
    });

    const bulkApproveMutation = useMutation({
        mutationFn: async () => {
            const pending = ((proposalsQuery.data ?? []) as Proposal[]).filter(p => p.status === "pending");
            const actions = pending.map(p => ({ proposalId: p.id, action: "approve" as const }));
            return unwrapEden(await (api.api as any).proposals.bulk.post({ actions }));
        },
        onSuccess: () => {
            void proposalsQuery.refetch();
            void countQuery.refetch();
        },
    });

    const bulkRejectMutation = useMutation({
        mutationFn: async () => {
            const pending = ((proposalsQuery.data ?? []) as Proposal[]).filter(p => p.status === "pending");
            const actions = pending.map(p => ({ proposalId: p.id, action: "reject" as const }));
            return unwrapEden(await (api.api as any).proposals.bulk.post({ actions }));
        },
        onSuccess: () => {
            void proposalsQuery.refetch();
            void countQuery.refetch();
        },
    });

    const proposals = (proposalsQuery.data ?? []) as Proposal[];
    const pendingCount = (countQuery.data as any)?.pending ?? 0;
    const hasPending = proposals.some(p => p.status === "pending");

    const refetch = () => {
        void proposalsQuery.refetch();
        void countQuery.refetch();
    };

    const STATUS_OPTIONS = ["pending", "approved", "rejected", "all"] as const;

    return (
        <PageContainer>
            <PageHeader
                title="Review Queue"
                description={`${pendingCount} proposal${pendingCount === 1 ? "" : "s"} waiting for review`}
            />
            <div className="mb-4 flex items-center justify-between">
                <div className="flex gap-1">
                    {STATUS_OPTIONS.map(s => (
                        <Button
                            key={s}
                            size="sm"
                            variant={statusFilter === s ? "default" : "ghost"}
                            onClick={() => setStatusFilter(s)}
                        >
                            {s.charAt(0).toUpperCase() + s.slice(1)}
                        </Button>
                    ))}
                </div>
                {hasPending && (
                    <div className="flex gap-2">
                        <Button
                            size="sm"
                            variant="ghost"
                            onClick={() => {
                                if (confirm("Approve all visible pending proposals?")) bulkApproveMutation.mutate();
                            }}
                        >
                            Approve all
                        </Button>
                        <Button
                            size="sm"
                            variant="ghost"
                            onClick={() => {
                                if (confirm("Reject all visible pending proposals?")) bulkRejectMutation.mutate();
                            }}
                        >
                            Reject all
                        </Button>
                    </div>
                )}
            </div>
            {proposalsQuery.isLoading ? (
                <PageLoading />
            ) : proposals.length === 0 ? (
                <div className="rounded-md border border-dashed p-8 text-center text-sm text-muted-foreground">
                    No proposals waiting for review
                </div>
            ) : (
                <div className="space-y-2">
                    {proposals.map(p => (
                        <ProposalCard key={p.id} proposal={p} onUpdate={refetch} />
                    ))}
                </div>
            )}
        </PageContainer>
    );
}
```

- [ ] **Step 5: Regenerate route tree + type check**

Run: `pnpm --filter web run build 2>&1 | tail -15`

Expected: build succeeds, route tree includes `/review`.

- [ ] **Step 6: Commit**

```bash
git add apps/web/src/features/proposals/ apps/web/src/routes/review.tsx apps/web/src/routeTree.gen.ts
git commit -m "feat(web): review queue page with proposal cards and field-by-field diff

Co-Authored-By: Claude Opus 4.6 (1M context) <noreply@anthropic.com>"
```

---

### Task 6: Inline Proposals on Chunk Detail + Nav Badge

**Files:**
- Create: `apps/web/src/features/proposals/chunk-proposals-section.tsx`
- Create: `apps/web/src/features/proposals/use-pending-proposal-count.ts`
- Modify: `apps/web/src/routes/chunks.$chunkId.tsx`
- Modify: `apps/web/src/routes/__root.tsx`

- [ ] **Step 1: Create `apps/web/src/features/proposals/chunk-proposals-section.tsx`**

```typescript
import { useQuery } from "@tanstack/react-query";
import { AlertTriangle } from "lucide-react";

import { api } from "@/utils/api";
import { unwrapEden } from "@/utils/eden";

import { ProposalCard, type Proposal } from "./proposal-card";

export interface ChunkProposalsSectionProps {
    chunkId: string;
}

export function ChunkProposalsSection({ chunkId }: ChunkProposalsSectionProps) {
    const proposalsQuery = useQuery({
        queryKey: ["chunk-proposals", chunkId],
        queryFn: async () =>
            unwrapEden(await (api.api as any).chunks[chunkId].proposals.get({ query: { status: "pending" } })),
    });

    const proposals = (proposalsQuery.data ?? []) as Proposal[];
    if (proposals.length === 0) return null;

    return (
        <section className="space-y-2">
            <h3 className="flex items-center gap-1.5 text-xs font-semibold uppercase tracking-wider text-amber-500">
                <AlertTriangle className="size-3.5" />
                Pending proposals ({proposals.length})
            </h3>
            <div className="space-y-2">
                {proposals.map(p => (
                    <ProposalCard
                        key={p.id}
                        proposal={p}
                        showChunkInfo={false}
                        onUpdate={() => { void proposalsQuery.refetch(); }}
                    />
                ))}
            </div>
        </section>
    );
}
```

- [ ] **Step 2: Mount in `apps/web/src/routes/chunks.$chunkId.tsx`**

Read the chunk detail route file first. Find where the main content sections are rendered. Add the import:

```typescript
import { ChunkProposalsSection } from "@/features/proposals/chunk-proposals-section";
```

Add below the chunk content (but above the "More Context" drawer or connections section — wherever feels natural as a high-visibility position):

```tsx
<ChunkProposalsSection chunkId={chunkId} />
```

If the chunk detail page delegates to sub-components (e.g., `ChunkDetailContent`), add it there instead. Read the file to find the right insertion point.

- [ ] **Step 3: Create `apps/web/src/features/proposals/use-pending-proposal-count.ts`**

```typescript
import { useQuery } from "@tanstack/react-query";

import { api } from "@/utils/api";
import { unwrapEden } from "@/utils/eden";

export function usePendingProposalCount(): number {
    const { data } = useQuery({
        queryKey: ["proposals-count"],
        queryFn: async () => unwrapEden(await (api.api as any).proposals.count.get()),
        refetchInterval: 60_000,
        staleTime: 30_000,
    });
    return (data as any)?.pending ?? 0;
}
```

- [ ] **Step 4: Add nav badge in `apps/web/src/routes/__root.tsx`**

Read the file. Find the primary nav — there should be a `<Link to="/plans">Plans</Link>` or `<Link to="/chunks">Chunks</Link>`. Add a review badge next to the Manage dropdown or in the "Navigate" section of the Manage dropdown.

Add the import:

```typescript
import { usePendingProposalCount } from "@/features/proposals/use-pending-proposal-count";
```

In the `RootDocument` function, add:

```typescript
const proposalCount = usePendingProposalCount();
```

In the Manage dropdown's Navigate section, add a "Review" entry after "Requirements":

```tsx
<DropdownMenuItem render={<Link to="/review" />}>
    <AlertTriangle className="size-4" />
    Review
    {proposalCount > 0 && (
        <span className="ml-auto flex size-4 items-center justify-center rounded-full bg-amber-500 text-[10px] font-bold text-white">
            {proposalCount > 9 ? "9+" : proposalCount}
        </span>
    )}
</DropdownMenuItem>
```

Add `AlertTriangle` to the lucide import if not already present.

- [ ] **Step 5: Type check + build**

Run: `pnpm --filter web run build 2>&1 | tail -15`

Expected: build succeeds.

- [ ] **Step 6: Commit**

```bash
git add apps/web/src/features/proposals/ apps/web/src/routes/chunks.\$chunkId.tsx apps/web/src/routes/__root.tsx apps/web/src/routeTree.gen.ts
git commit -m "feat(web): inline chunk proposals section + nav badge

Co-Authored-By: Claude Opus 4.6 (1M context) <noreply@anthropic.com>"
```

---

### Task 7: Final Verification

**Files:** (verification only, plus CLAUDE.md if needed)

- [ ] **Step 1: Full type check**

```bash
pnpm --filter @fubbik/db run check-types 2>&1 | tail -15
pnpm --filter @fubbik/api run check-types 2>&1 | tail -15
pnpm --filter @fubbik/mcp run check-types 2>&1 | tail -15
pnpm --filter web run check-types 2>&1 | tail -15
```

Expected: zero new errors in `chunk-proposal`, `proposals/`, `review.tsx`, `proposal-*` files.

- [ ] **Step 2: Build**

```bash
pnpm build 2>&1 | tail -20
```

Expected: success.

- [ ] **Step 3: Server starts**

```bash
timeout 10 bun run --cwd apps/server src/index.ts 2>&1 | head -10
```

Expected: "Server is running on http://localhost:3000" — no missing export errors.

- [ ] **Step 4: Run tests**

```bash
pnpm test 2>&1 | tail -20
```

Expected: all new code compiles and doesn't break existing tests.

- [ ] **Step 5: Manual smoke test**

Start dev: `pnpm dev`

1. Navigate to `/review` — page loads, shows "No proposals waiting for review"
2. Create a proposal via curl:
   ```bash
   curl -X POST http://localhost:3000/api/chunks/<CHUNK_ID>/proposals \
     -H "Content-Type: application/json" \
     -d '{"changes":{"title":"Updated Title","tags":["api","auth"]},"reason":"Testing the proposal system"}'
   ```
   (Use a real chunk ID from the seed data)
3. Refresh `/review` — proposal appears with chunk title, changed fields pills, approve/reject buttons
4. Click the proposal to expand — field-by-field diff renders
5. Navigate to `/chunks/<CHUNK_ID>` — the "Pending proposals" amber section appears
6. Click "Approve" on the proposal
7. Verify: chunk title changed, proposal status = approved
8. Verify nav badge shows count (if proposals exist), disappears when queue is empty

- [ ] **Step 6: Commit any fixes**

```bash
git commit -am "fix: resolve smoke test issues for proposal review

Co-Authored-By: Claude Opus 4.6 (1M context) <noreply@anthropic.com>"
```
