# Requirements & Plans Integration Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Surface requirements and plans in CLAUDE.md export, context-for-file, health scoring, staleness detection, and dashboard widgets.

**Architecture:** Five independent read-side integrations into existing systems. No new schema tables — all relationships already exist. Each task touches 1-2 files and can be tested independently.

**Tech Stack:** TypeScript, Effect, Drizzle ORM, React, TanStack Query, Elysia

---

## File Structure

| File | Action | Responsibility |
|------|--------|---------------|
| `packages/api/src/context-export/claude-md.ts` | Modify | Add requirements, plans, sessions sections |
| `packages/api/src/context-for-file/service.ts` | Modify | Surface requirements linked to matched chunks |
| `packages/api/src/chunks/health-score.ts` | Modify | Rebalance to 5 dimensions, add coverage |
| `packages/db/src/repository/staleness.ts` | Modify | Add `requirement_failing` and `requirement_uncovered` detection |
| `packages/api/src/staleness/service.ts` | Modify | Re-export new staleness functions |
| `packages/api/src/requirements/service.ts` | Modify | Trigger `requirement_failing` flags on status change |
| `apps/web/src/routes/dashboard.tsx` | Modify | Add plans widget, sessions widget, enhance req stats |

---

### Task 1: CLAUDE.md Export Enhancement

**Files:**
- Modify: `packages/api/src/context-export/claude-md.ts`

**Context:** This file generates markdown context for AI agents. Currently only exports chunks grouped by type. We need to add sections for requirements, active plans, and recent sessions after the chunks section.

- [ ] **Step 1: Read the current `claude-md.ts` implementation**

Read `packages/api/src/context-export/claude-md.ts` to confirm the current structure.

- [ ] **Step 2: Add repository imports and implement the enhanced generation**

Replace the full file contents with:

```typescript
import { listChunksByTag, listRequirements, getRequirementStats, getChunksForRequirement } from "@fubbik/db/repository";
import { listPlans, getStepsForPlan } from "@fubbik/db/repository/plan";
import { listSessions, getSessionDetail } from "@fubbik/db/repository/implementation-session";
import { Effect } from "effect";

interface GenerateClaudeMdParams {
    userId: string;
    codebaseId?: string;
    tag?: string;
}

interface ChunkRow {
    id: string;
    title: string;
    content: string;
    type: string;
    rationale: string | null;
    summary: string | null;
}

const TYPE_SECTIONS: Record<string, string> = {
    note: "Conventions",
    document: "Architecture",
    reference: "References"
};

function sectionLabel(type: string): string {
    return TYPE_SECTIONS[type] ?? "Other";
}

function formatChunkEntry(c: ChunkRow): string {
    const parts = [`### ${c.title}`];
    if (c.content) parts.push(c.content);
    if (c.rationale) parts.push(`**Rationale:** ${c.rationale}`);
    return parts.join("\n\n");
}

export function generateClaudeMd(params: GenerateClaudeMdParams) {
    const tagName = params.tag ?? "claude-context";

    return Effect.gen(function* () {
        // Fetch chunks (existing)
        const chunks = yield* listChunksByTag({
            userId: params.userId,
            tagName,
            codebaseId: params.codebaseId
        });

        const parts: string[] = ["# Project Context\n"];

        // ── Chunks section (existing logic) ──
        if (chunks.length === 0) {
            parts.push(`No chunks found with tag "${tagName}".\n`);
        } else {
            const sections = new Map<string, ChunkRow[]>();
            for (const c of chunks) {
                const label = sectionLabel(c.type);
                const group = sections.get(label) ?? [];
                group.push(c);
                sections.set(label, group);
            }

            const sectionOrder = ["Conventions", "Architecture", "References", "Other"];
            for (const sectionName of sectionOrder) {
                const group = sections.get(sectionName);
                if (!group || group.length === 0) continue;
                parts.push(`## ${sectionName}\n`);
                for (const c of group) {
                    parts.push(formatChunkEntry(c));
                }
            }
        }

        // ── Requirements section ──
        const { requirements } = yield* listRequirements(params.userId, {
            codebaseId: params.codebaseId,
            limit: "50"
        });

        if (requirements.length > 0) {
            parts.push("## Requirements\n");

            // Sort: failing first, then untested, then passing
            const statusOrder: Record<string, number> = { failing: 0, untested: 1, passing: 2 };
            const sorted = [...requirements].sort(
                (a, b) => (statusOrder[a.status] ?? 3) - (statusOrder[b.status] ?? 3)
            );

            for (const req of sorted) {
                const marker = req.status === "failing" || req.status === "untested"
                    ? " <!-- ACTION NEEDED -->"
                    : "";
                const priority = req.priority ? ` [${req.priority}]` : "";
                parts.push(`### ${req.title}${priority} — ${req.status}${marker}`);

                if (req.steps && Array.isArray(req.steps)) {
                    const stepsText = (req.steps as Array<{ keyword: string; text: string }>)
                        .map(s => `- **${s.keyword}** ${s.text}`)
                        .join("\n");
                    parts.push(stepsText);
                }
            }
        }

        // ── Active plans section ──
        const plans = yield* listPlans(params.userId, params.codebaseId, "active");

        if (plans.length > 0) {
            parts.push("## Active Plans\n");

            for (const plan of plans) {
                const steps = yield* getStepsForPlan(plan.id);
                const done = steps.filter(s => s.status === "done" || s.status === "skipped").length;
                const total = steps.length;
                const pct = total > 0 ? Math.round((done / total) * 100) : 0;

                parts.push(`### ${plan.title} (${done}/${total} steps — ${pct}%)`);

                const pending = steps.filter(s => s.status === "pending" || s.status === "in_progress");
                if (pending.length > 0) {
                    const pendingText = pending
                        .map(s => `- [ ] ${s.description}`)
                        .join("\n");
                    parts.push(pendingText);
                }
            }
        }

        // ── Recent sessions section ──
        const { sessions } = yield* listSessions({
            userId: params.userId,
            status: "completed",
            limit: 5
        });

        if (sessions.length > 0) {
            parts.push("## Recent Implementation Sessions\n");

            for (const session of sessions) {
                const detail = yield* getSessionDetail(session.id);
                const reqCount = detail.requirementRefs?.length ?? 0;
                const unresolvedCount = (detail.assumptions ?? []).filter(
                    (a: any) => !a.resolved
                ).length;

                const reviewStatus = session.reviewStatus ?? "pending";
                let line = `### ${session.title} — ${reviewStatus}`;
                if (reqCount > 0) line += ` (${reqCount} requirements addressed)`;
                parts.push(line);

                if (unresolvedCount > 0) {
                    parts.push(`**${unresolvedCount} unresolved assumption(s)**`);
                }
            }
        }

        return { content: parts.join("\n\n"), chunks: chunks.length };
    });
}
```

- [ ] **Step 3: Verify type-check passes**

Run: `pnpm --filter @fubbik/api run check-types 2>&1 | head -20`

Expected: No errors in `claude-md.ts`. If there are import errors, check that the repository functions are exported from the right paths and adjust imports accordingly.

- [ ] **Step 4: Commit**

```bash
git add packages/api/src/context-export/claude-md.ts
git commit -m "feat(context-export): add requirements, plans, sessions to CLAUDE.md generation"
```

---

### Task 2: Context-for-File Enhancement

**Files:**
- Modify: `packages/api/src/context-for-file/service.ts`

**Context:** This service returns chunks relevant to a file path. We need to also return requirements linked to those matched chunks. The `requirement_chunk` join table links requirements to chunks — we query it after chunk matching.

- [ ] **Step 1: Read `packages/api/src/context-for-file/service.ts` to confirm current structure**

- [ ] **Step 2: Add the requirements lookup**

Add a new export type and extend the function. Replace the file contents with:

```typescript
import { getAppliesToForChunk, getChunkById, listChunks, listCodebases, lookupChunksByFilePath } from "@fubbik/db/repository";
import { Effect } from "effect";
import { db } from "@fubbik/db";
import { requirement, requirementChunk } from "@fubbik/db/schema/requirement";
import { eq, inArray } from "drizzle-orm";

import { globMatch } from "./glob-match";

export interface ContextChunk {
    id: string;
    title: string;
    type: string;
    content: string;
    summary: string | null;
    matchReason: "file-ref" | "applies-to" | "dependency";
}

export interface ContextRequirement {
    id: string;
    title: string;
    status: string;
    priority: string | null;
    steps: Array<{ keyword: string; text: string }>;
    matchedChunkIds: string[];
}

export interface FileContext {
    chunks: ContextChunk[];
    requirements: ContextRequirement[];
}

/**
 * Check if a dependency name matches a codebase name.
 * Supports partial matching: "@acme/auth" matches codebase named "auth".
 */
function depMatchesCodebase(dep: string, codebaseName: string): boolean {
    const depLower = dep.toLowerCase();
    const cbLower = codebaseName.toLowerCase();
    if (depLower === cbLower) return true;
    const lastSegment = depLower.split("/").pop()!;
    return lastSegment === cbLower;
}

export function getContextForFile(
    userId: string,
    filePath: string,
    codebaseId?: string,
    deps?: string[]
): Effect.Effect<FileContext> {
    return Effect.gen(function* () {
        const results = new Map<string, ContextChunk>();

        // 1. Direct file-ref matches
        const fileRefMatches = yield* lookupChunksByFilePath(filePath, userId);
        for (const match of fileRefMatches) {
            if (results.has(match.chunkId)) continue;
            const full = yield* getChunkById(match.chunkId, userId);
            if (!full) continue;
            results.set(match.chunkId, {
                id: full.id,
                title: full.title,
                type: full.type,
                content: full.content,
                summary: full.summary,
                matchReason: "file-ref"
            });
        }

        // 2. Applies-to glob pattern matches
        const { chunks } = yield* listChunks({
            userId,
            codebaseId,
            limit: 1000,
            offset: 0
        });

        for (const c of chunks) {
            if (results.has(c.id)) continue;
            const patterns = yield* getAppliesToForChunk(c.id);
            if (patterns.length === 0) continue;

            const matches = patterns.some(p => globMatch(p.pattern, filePath));
            if (matches) {
                results.set(c.id, {
                    id: c.id,
                    title: c.title,
                    type: c.type,
                    content: c.content,
                    summary: c.summary,
                    matchReason: "applies-to"
                });
            }
        }

        // 3. Dependency-based matches
        if (deps && deps.length > 0) {
            const allCodebases = yield* listCodebases(userId);
            const matchedCodebaseIds: string[] = [];
            for (const cb of allCodebases) {
                if (deps.some(dep => depMatchesCodebase(dep, cb.name))) {
                    matchedCodebaseIds.push(cb.id);
                }
            }

            for (const cbId of matchedCodebaseIds) {
                const { chunks: depChunks } = yield* listChunks({
                    userId,
                    codebaseId: cbId,
                    limit: 5,
                    offset: 0,
                    sort: "updated"
                });
                for (const c of depChunks) {
                    if (results.has(c.id)) continue;
                    results.set(c.id, {
                        id: c.id,
                        title: c.title,
                        type: c.type,
                        content: c.content,
                        summary: c.summary,
                        matchReason: "dependency"
                    });
                }
            }
        }

        const matchedChunks = Array.from(results.values());

        // 4. Requirements linked to matched chunks
        const matchedChunkIds = matchedChunks.map(c => c.id);
        let requirements: ContextRequirement[] = [];

        if (matchedChunkIds.length > 0) {
            const reqLinks = yield* Effect.tryPromise({
                try: () =>
                    db
                        .select({
                            requirementId: requirementChunk.requirementId,
                            chunkId: requirementChunk.chunkId,
                            id: requirement.id,
                            title: requirement.title,
                            status: requirement.status,
                            priority: requirement.priority,
                            steps: requirement.steps
                        })
                        .from(requirementChunk)
                        .innerJoin(requirement, eq(requirementChunk.requirementId, requirement.id))
                        .where(inArray(requirementChunk.chunkId, matchedChunkIds)),
                catch: () => ({ _tag: "DatabaseError" as const })
            });

            // Group by requirement, collect matched chunk IDs
            const reqMap = new Map<string, ContextRequirement>();
            for (const row of reqLinks) {
                const existing = reqMap.get(row.id);
                if (existing) {
                    existing.matchedChunkIds.push(row.chunkId);
                } else {
                    reqMap.set(row.id, {
                        id: row.id,
                        title: row.title,
                        status: row.status,
                        priority: row.priority ?? null,
                        steps: (row.steps as Array<{ keyword: string; text: string }>) ?? [],
                        matchedChunkIds: [row.chunkId]
                    });
                }
            }
            requirements = Array.from(reqMap.values());
        }

        return { chunks: matchedChunks, requirements };
    });
}
```

- [ ] **Step 3: Update the route handler if it destructures the return value**

Check `packages/api/src/context-for-file/routes.ts` — if it returns the result directly, the new `{ chunks, requirements }` shape will flow through automatically. If it wraps the result in a `{ chunks: ... }` object, adjust accordingly.

- [ ] **Step 4: Verify type-check passes**

Run: `pnpm --filter @fubbik/api run check-types 2>&1 | head -20`

Expected: No errors in `context-for-file/service.ts`. If `db` or schema imports fail, adjust import paths to match the project's module resolution.

- [ ] **Step 5: Commit**

```bash
git add packages/api/src/context-for-file/service.ts
git commit -m "feat(context): surface requirements linked to file-relevant chunks"
```

---

### Task 3: Health Score Rebalance

**Files:**
- Modify: `packages/api/src/chunks/health-score.ts`

**Context:** Currently 4 dimensions at 0-25 each. We rebalance to 5 dimensions at 0-20 each, adding a `coverage` dimension for requirement backing. The input interface needs a new field for requirement data.

- [ ] **Step 1: Read the current health-score.ts**

Confirm the existing `ChunkHealthInput` and `HealthScore` interfaces.

- [ ] **Step 2: Replace the file contents**

```typescript
export interface ChunkHealthInput {
    content: string;
    updatedAt: Date;
    summary: string | null;
    rationale: string | null;
    alternatives: string[] | null;
    consequences: string | null;
    connectionCount: number;
    hasEmbedding: boolean;
    requirementCount: number;
    allRequirementsPassing: boolean;
    referencedInSession: boolean;
}

export interface HealthScore {
    total: number; // 0-100
    breakdown: {
        freshness: number; // 0-20
        completeness: number; // 0-20
        richness: number; // 0-20
        connectivity: number; // 0-20
        coverage: number; // 0-20
    };
    issues: string[];
}

export function computeHealthScore(input: ChunkHealthInput): HealthScore {
    const issues: string[] = [];

    // Freshness (0-20): Full at <7 days, degrades to 0 at 90 days
    const daysSinceUpdate = (Date.now() - input.updatedAt.getTime()) / (1000 * 60 * 60 * 24);
    let freshness: number;
    if (daysSinceUpdate < 7) {
        freshness = 20;
    } else if (daysSinceUpdate >= 90) {
        freshness = 0;
    } else {
        freshness = Math.round(20 * (1 - (daysSinceUpdate - 7) / (90 - 7)));
    }
    if (daysSinceUpdate >= 30) {
        issues.push("Chunk has not been updated in over 30 days");
    }

    // Completeness (0-20): Base 8 for content, +4 each for rationale/alternatives/consequences
    let completeness = input.content.length > 0 ? 8 : 0;
    if (input.rationale) completeness += 4;
    if (input.alternatives && input.alternatives.length > 0) completeness += 4;
    if (input.consequences) completeness += 4;

    // Richness (0-20): content length + summary + embedding
    let richness = 0;
    if (input.content.length >= 200) {
        richness += 8;
    } else if (input.content.length >= 100) {
        richness += 4;
    }
    if (input.content.length < 100) {
        issues.push("Content is thin (less than 100 characters)");
    }
    if (input.summary) {
        richness += 6;
    } else {
        issues.push("Missing AI summary");
    }
    if (input.hasEmbedding) {
        richness += 6;
    } else {
        issues.push("Missing embedding for semantic search");
    }

    // Connectivity (0-20): 20 for 3+, 12 for 1-2, 0 for orphans
    let connectivity: number;
    if (input.connectionCount >= 3) {
        connectivity = 20;
    } else if (input.connectionCount >= 1) {
        connectivity = 12;
    } else {
        connectivity = 0;
        issues.push("Orphan chunk with no connections");
    }

    // Coverage (0-20): requirement backing
    let coverage: number;
    if (input.requirementCount === 0) {
        coverage = 0;
        issues.push("No requirements linked");
    } else if (!input.allRequirementsPassing) {
        coverage = 10;
    } else if (!input.referencedInSession) {
        coverage = 15;
    } else {
        coverage = 20;
    }

    const total = freshness + completeness + richness + connectivity + coverage;

    return {
        total,
        breakdown: {
            freshness,
            completeness,
            richness,
            connectivity,
            coverage
        },
        issues
    };
}
```

- [ ] **Step 3: Update callers of `computeHealthScore`**

Search for all callers of `computeHealthScore` and update them to pass the new fields. The caller is likely in the chunk detail route/service. Find it with:

```bash
grep -rn "computeHealthScore" packages/api/src/ apps/
```

For each caller, add the three new fields to the `ChunkHealthInput` object:
- `requirementCount`: query `requirement_chunk` table for this chunk's ID, count rows
- `allRequirementsPassing`: query linked requirements, check all have `status = 'passing'`
- `referencedInSession`: query `session_chunk_ref` table for this chunk's ID, check if any exist

If the caller doesn't have easy access to these, add a repository helper or inline the query. Use `Effect.tryPromise` for the DB queries, matching the project pattern.

For callers where the data isn't available (e.g., bulk health computation), default to `{ requirementCount: 0, allRequirementsPassing: false, referencedInSession: false }` so existing scores degrade gracefully (coverage = 0).

- [ ] **Step 4: Verify type-check passes**

Run: `pnpm --filter @fubbik/api run check-types 2>&1 | head -30`

Expected: No type errors. If the web app also uses `HealthScore`, check it too:
Run: `pnpm --filter web run check-types 2>&1 | grep health`

- [ ] **Step 5: Commit**

```bash
git add packages/api/src/chunks/health-score.ts
git add -u packages/api/src/  # include any modified callers
git commit -m "feat(health): rebalance to 5 dimensions, add requirement coverage scoring"
```

---

### Task 4: Staleness Detection Enhancement

**Files:**
- Modify: `packages/db/src/repository/staleness.ts`
- Modify: `packages/api/src/staleness/service.ts`
- Modify: `packages/api/src/requirements/service.ts` (trigger on status change)

**Context:** We add two new staleness reasons: `requirement_failing` (when a requirement goes to "failing") and `requirement_uncovered` (chunks 30+ days old with no requirement links). The existing `chunk_staleness.reason` is a text column — no migration needed.

- [ ] **Step 1: Add `detectUncoveredChunks` to the staleness repository**

Add to the end of `packages/db/src/repository/staleness.ts`:

```typescript
export function detectUncoveredChunks(userId: string, codebaseId?: string, thresholdDays = 30) {
    return Effect.tryPromise({
        try: async () => {
            const threshold = sql`NOW() - INTERVAL '${sql.raw(String(thresholdDays))} days'`;

            // Chunks already flagged for "requirement_uncovered" (undismissed)
            const alreadyFlagged = db
                .select({ chunkId: chunkStaleness.chunkId })
                .from(chunkStaleness)
                .where(
                    and(
                        eq(chunkStaleness.reason, "requirement_uncovered"),
                        isNull(chunkStaleness.dismissedAt)
                    )
                );

            // Chunks that have at least one requirement link
            const hasRequirement = db
                .select({ chunkId: requirementChunk.chunkId })
                .from(requirementChunk);

            const conditions = [
                eq(chunk.userId, userId),
                sql`${chunk.createdAt} < ${threshold}`,
                isNull(chunk.archivedAt),
                sql`${chunk.id} NOT IN (${alreadyFlagged})`,
                sql`${chunk.id} NOT IN (${hasRequirement})`,
                ...codebaseConditions(codebaseId)
            ];

            const uncoveredChunks = await db
                .select({ id: chunk.id })
                .from(chunk)
                .where(and(...conditions));

            if (uncoveredChunks.length === 0) {
                return { flagged: 0 };
            }

            const flags = uncoveredChunks.map(c => ({
                id: crypto.randomUUID(),
                chunkId: c.id,
                reason: "requirement_uncovered" as const,
                detail: "No requirements linked — consider adding requirement coverage"
            }));

            await db.insert(chunkStaleness).values(flags).onConflictDoNothing();

            return { flagged: flags.length };
        },
        catch: cause => new DatabaseError({ cause })
    });
}

export function flagRequirementFailing(requirementId: string, requirementTitle: string, chunkIds: string[]) {
    return Effect.tryPromise({
        try: async () => {
            if (chunkIds.length === 0) return { flagged: 0 };

            // Don't re-flag chunks already flagged for this requirement
            const alreadyFlagged = await db
                .select({ chunkId: chunkStaleness.chunkId })
                .from(chunkStaleness)
                .where(
                    and(
                        eq(chunkStaleness.reason, "requirement_failing"),
                        isNull(chunkStaleness.dismissedAt),
                        inArray(chunkStaleness.chunkId, chunkIds),
                        eq(chunkStaleness.detail, `Requirement "${requirementTitle}" (${requirementId}) is failing`)
                    )
                );

            const alreadySet = new Set(alreadyFlagged.map(r => r.chunkId));
            const newChunkIds = chunkIds.filter(id => !alreadySet.has(id));

            if (newChunkIds.length === 0) return { flagged: 0 };

            const flags = newChunkIds.map(chunkId => ({
                id: crypto.randomUUID(),
                chunkId,
                reason: "requirement_failing" as const,
                detail: `Requirement "${requirementTitle}" (${requirementId}) is failing`
            }));

            await db.insert(chunkStaleness).values(flags).onConflictDoNothing();

            return { flagged: flags.length };
        },
        catch: cause => new DatabaseError({ cause })
    });
}
```

You will also need to add `requirementChunk` and `inArray` to the imports at the top of the file:

```typescript
import { and, desc, eq, inArray, isNull, sql } from "drizzle-orm";
import { requirementChunk } from "../schema/requirement";
```

- [ ] **Step 2: Re-export from the staleness service**

In `packages/api/src/staleness/service.ts`, add the new exports:

```typescript
export {
    getStaleFlags,
    getStaleCount,
    getStaleFlagsForChunk,
    dismissStaleFlag,
    suppressDuplicatePair,
    detectUncoveredChunks,
    flagRequirementFailing
};
```

Make sure the import source matches (it should import from `@fubbik/db/repository` or the staleness repository directly).

- [ ] **Step 3: Trigger `requirement_failing` flags on requirement status change**

In `packages/api/src/requirements/service.ts`, find the `updateRequirement` function. After the requirement is updated, if the new status is `"failing"`, call `flagRequirementFailing`. Add this logic:

Find the section where the requirement is updated and add after it:

```typescript
// After the requirement update succeeds:
if (body.status === "failing") {
    const chunkLinks = yield* getChunksForRequirement(id);
    const chunkIds = chunkLinks.map(l => l.chunkId);
    if (chunkIds.length > 0) {
        yield* flagRequirementFailing(id, updated.title, chunkIds);
    }
}
```

Import `flagRequirementFailing` from the staleness repository at the top of the file.

- [ ] **Step 4: Integrate `detectUncoveredChunks` into the age scan route**

Find the route that handles `POST /api/chunks/stale/scan-age` (likely in `packages/api/src/staleness/routes.ts` or `packages/api/src/chunks/routes.ts`). After calling `detectAgeStaleChunks`, also call `detectUncoveredChunks` and return combined results:

```typescript
const ageResult = yield* detectAgeStaleChunks(session.user.id, body.codebaseId, body.thresholdDays);
const uncoveredResult = yield* detectUncoveredChunks(session.user.id, body.codebaseId);
return { flagged: ageResult.flagged + uncoveredResult.flagged };
```

- [ ] **Step 5: Verify type-check passes**

Run: `pnpm --filter @fubbik/db run check-types 2>&1 | head -20`
Run: `pnpm --filter @fubbik/api run check-types 2>&1 | head -20`

Expected: No type errors in staleness or requirements files.

- [ ] **Step 6: Commit**

```bash
git add packages/db/src/repository/staleness.ts
git add packages/api/src/staleness/service.ts
git add packages/api/src/requirements/service.ts
git add -u  # include any route handler changes
git commit -m "feat(staleness): add requirement_failing and requirement_uncovered detection"
```

---

### Task 5: Dashboard Enhancement

**Files:**
- Modify: `apps/web/src/routes/dashboard.tsx`

**Context:** The dashboard already has a requirements stats card and uses the `DashboardSection` component pattern. We need to: (1) make requirement stat counts clickable with filtering, (2) add an "Active Plans" widget, (3) add a "Recent Sessions" widget. Existing queries for plans and sessions are available via the Eden API client.

- [ ] **Step 1: Read the current dashboard.tsx**

Confirm the current layout structure and query patterns.

- [ ] **Step 2: Add new query hooks**

In the `DashboardPage` component, after the existing `activityQuery`, add:

```typescript
const plansQuery = useQuery({
    queryKey: ["dashboard-plans", codebaseId],
    queryFn: async () => unwrapEden(await api.api.plans.get({ query: { ...codebaseQuery, status: "active" } as any }))
});

const sessionsQuery = useQuery({
    queryKey: ["dashboard-sessions"],
    queryFn: async () => unwrapEden(await api.api.sessions.get({ query: { status: "completed", limit: "3" } as any }))
});
```

- [ ] **Step 3: Enhance the requirements stat card**

Replace the existing requirements `StatCard` (around line 268) with an expanded version that shows passing/failing/untested as clickable counts:

Replace:
```tsx
<StatCard
    icon={FileText}
    label="Requirements"
    value={reqStats ? (reqStats as any).total : undefined}
    loading={requirementsQuery.isLoading}
    sub={reqStats ? `${(reqStats as any).passing ?? 0} passing` : undefined}
    to="/requirements"
/>
```

With:
```tsx
<Link to="/requirements">
    <div className="bg-card hover:bg-muted/50 cursor-pointer rounded-lg border p-4 transition-colors">
        <div className="flex items-center gap-2">
            <ClipboardList className="text-muted-foreground size-4" />
            <span className="text-muted-foreground text-xs font-medium uppercase tracking-wide">Requirements</span>
        </div>
        <div className="mt-2 text-2xl font-bold tabular-nums tracking-tight">
            {requirementsQuery.isLoading ? <Skeleton className="h-8 w-16" /> : (reqStats as any)?.total ?? 0}
        </div>
        {reqStats && (
            <div className="mt-1 flex gap-2 text-[11px]">
                <Link to="/requirements" search={{ status: "passing" } as any} className="text-emerald-500 hover:underline">
                    {(reqStats as any).passing ?? 0} passing
                </Link>
                <Link to="/requirements" search={{ status: "failing" } as any} className="text-red-500 hover:underline">
                    {(reqStats as any).failing ?? 0} failing
                </Link>
                <Link to="/requirements" search={{ status: "untested" } as any} className="text-muted-foreground hover:underline">
                    {(reqStats as any).untested ?? 0} untested
                </Link>
            </div>
        )}
    </div>
</Link>
```

Also add `ClipboardList` to the lucide-react imports at the top of the file (if not already there).

- [ ] **Step 4: Add the Active Plans widget**

In the right column (`<div className="space-y-6">` section), after the Requirements summary section and before the Activity section, add:

```tsx
{/* Active Plans */}
<DashboardSection
    icon={ClipboardList}
    title="Active Plans"
    action={
        <Link to="/plans" className="text-muted-foreground hover:text-foreground flex items-center gap-1 text-xs transition-colors">
            View all <ArrowRight className="size-3" />
        </Link>
    }
>
    {plansQuery.isLoading ? (
        <SkeletonList count={3} />
    ) : (() => {
        const plans = Array.isArray(plansQuery.data) ? plansQuery.data : (plansQuery.data as any)?.plans ?? [];
        return plans.length === 0 ? (
            <p className="text-muted-foreground py-2 text-center text-sm">No active plans</p>
        ) : (
            <div className="space-y-2">
                {plans.slice(0, 5).map((plan: any) => {
                    const done = plan.steps?.filter((s: any) => s.status === "done" || s.status === "skipped").length ?? 0;
                    const total = plan.steps?.length ?? plan.stepCount ?? 0;
                    const pct = total > 0 ? Math.round((done / total) * 100) : 0;
                    return (
                        <Link
                            key={plan.id}
                            to="/plans/$planId"
                            params={{ planId: plan.id }}
                            className="hover:bg-muted/50 block rounded-md px-2 py-2 transition-colors"
                        >
                            <div className="flex items-center justify-between">
                                <span className="truncate text-sm font-medium">{plan.title}</span>
                                <span className="text-muted-foreground ml-2 shrink-0 text-xs">{done}/{total}</span>
                            </div>
                            <div className="bg-muted mt-1.5 h-1 overflow-hidden rounded-full">
                                <div
                                    className="bg-primary h-full rounded-full transition-all"
                                    style={{ width: `${pct}%` }}
                                />
                            </div>
                        </Link>
                    );
                })}
            </div>
        );
    })()}
</DashboardSection>
```

- [ ] **Step 5: Add the Recent Sessions widget**

After the Active Plans widget, add:

```tsx
{/* Recent Sessions */}
<DashboardSection
    icon={Workflow}
    title="Recent Sessions"
    action={
        <Link to="/reviews" className="text-muted-foreground hover:text-foreground flex items-center gap-1 text-xs transition-colors">
            View all <ArrowRight className="size-3" />
        </Link>
    }
>
    {sessionsQuery.isLoading ? (
        <SkeletonList count={3} />
    ) : (() => {
        const sessions = Array.isArray(sessionsQuery.data) ? sessionsQuery.data : (sessionsQuery.data as any)?.sessions ?? [];
        return sessions.length === 0 ? (
            <p className="text-muted-foreground py-2 text-center text-sm">No recent sessions</p>
        ) : (
            <div className="space-y-1">
                {sessions.slice(0, 3).map((session: any) => (
                    <Link
                        key={session.id}
                        to="/reviews/$sessionId"
                        params={{ sessionId: session.id }}
                        className="hover:bg-muted/50 flex items-center gap-2 rounded-md px-2 py-2 transition-colors"
                    >
                        <div className="min-w-0 flex-1">
                            <p className="truncate text-sm font-medium">{session.title}</p>
                            <div className="mt-0.5 flex items-center gap-2">
                                <Badge
                                    variant="secondary"
                                    size="sm"
                                    className={`text-[9px] ${
                                        session.reviewStatus === "approved" ? "text-emerald-500" :
                                        session.reviewStatus === "rejected" ? "text-red-500" :
                                        "text-muted-foreground"
                                    }`}
                                >
                                    {session.reviewStatus ?? "pending"}
                                </Badge>
                                {session.completedAt && (
                                    <span className="text-muted-foreground text-[10px]">
                                        {timeAgo(session.completedAt)}
                                    </span>
                                )}
                            </div>
                        </div>
                    </Link>
                ))}
            </div>
        );
    })()}
</DashboardSection>
```

- [ ] **Step 6: Verify type-check passes**

Run: `pnpm --filter web run check-types 2>&1 | grep dashboard`

Expected: No type errors in dashboard.tsx.

- [ ] **Step 7: Commit**

```bash
git add apps/web/src/routes/dashboard.tsx
git commit -m "feat(dashboard): add active plans widget, recent sessions widget, enhanced requirements stats"
```
