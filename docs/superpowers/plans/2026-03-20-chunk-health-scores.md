# Chunk Health Scores Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add per-chunk health scores based on staleness, completeness, and coverage — surfaced on chunk detail and list pages.

**Architecture:** New service function computes a health score (0-100) from multiple signals: days since update, has rationale/connections/enrichment, content length, file reference validity. Computed on-demand (not stored). Exposed via a field on the chunk detail response and as a sort/filter option on the list.

**Tech Stack:** Elysia, Effect, React, Tailwind CSS

---

## File Structure

### New files:
- `packages/api/src/chunks/health-score.ts` — Health score computation logic
- `packages/api/src/chunks/health-score.test.ts` — Tests
- `apps/web/src/features/chunks/chunk-health-badge.tsx` — UI badge component

### Files to modify:
- `packages/api/src/chunks/service.ts` — Add health score to detail response
- `apps/web/src/routes/chunks.$chunkId.tsx` — Show health badge on detail page

---

## Task 1: Health Score Computation

**Files:**
- Create: `packages/api/src/chunks/health-score.ts`
- Create: `packages/api/src/chunks/health-score.test.ts`

- [ ] **Step 1: Write tests**

```ts
// packages/api/src/chunks/health-score.test.ts
import { describe, it, expect } from "vitest";
import { computeHealthScore } from "./health-score";

describe("computeHealthScore", () => {
    const baseChunk = {
        content: "A".repeat(200),
        updatedAt: new Date(),
        summary: "A summary",
        rationale: "Because reasons",
        alternatives: ["alt1"],
        consequences: "Some consequences",
        connectionCount: 3, // >= 3 for full connectivity score
        hasEmbedding: true,
    };

    it("returns 100 for a fully complete, fresh chunk", () => {
        const score = computeHealthScore(baseChunk);
        expect(score.total).toBe(100);
    });

    it("penalizes stale chunks (30+ days old)", () => {
        const stale = { ...baseChunk, updatedAt: new Date(Date.now() - 45 * 86400000) };
        const score = computeHealthScore(stale);
        expect(score.total).toBeLessThan(90);
        expect(score.breakdown.freshness).toBeLessThan(25);
    });

    it("penalizes thin content", () => {
        const thin = { ...baseChunk, content: "Short" };
        const score = computeHealthScore(thin);
        expect(score.total).toBeLessThan(90);
    });

    it("penalizes missing enrichment", () => {
        const noEnrich = { ...baseChunk, summary: null, hasEmbedding: false };
        const score = computeHealthScore(noEnrich);
        expect(score.total).toBeLessThan(90);
    });

    it("penalizes orphan chunks (no connections)", () => {
        const orphan = { ...baseChunk, connectionCount: 0 };
        const score = computeHealthScore(orphan);
        expect(score.total).toBeLessThan(90);
    });
});
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cd packages/api && pnpm vitest run src/chunks/health-score.test.ts`

- [ ] **Step 3: Implement computeHealthScore**

```ts
// packages/api/src/chunks/health-score.ts

interface ChunkHealthInput {
    content: string;
    updatedAt: Date;
    summary: string | null;
    rationale: string | null;
    alternatives: string[] | null;
    consequences: string | null;
    connectionCount: number;
    hasEmbedding: boolean;
}

interface HealthScore {
    total: number; // 0-100
    breakdown: {
        freshness: number;     // 0-25: how recently updated
        completeness: number;  // 0-25: has rationale/alternatives/consequences
        richness: number;      // 0-25: content length + enrichment
        connectivity: number;  // 0-25: connections to other chunks
    };
    issues: string[]; // human-readable problems
}

export function computeHealthScore(input: ChunkHealthInput): HealthScore {
    const issues: string[] = [];

    // Freshness (0-25): full score if <7 days, degrades to 0 at 90 days
    const daysSinceUpdate = (Date.now() - input.updatedAt.getTime()) / 86400000;
    let freshness = 25;
    if (daysSinceUpdate > 7) {
        freshness = Math.max(0, Math.round(25 * (1 - (daysSinceUpdate - 7) / 83)));
    }
    if (daysSinceUpdate > 30) issues.push("Not updated in 30+ days");

    // Completeness (0-25): rationale, alternatives, consequences
    let completeness = 10; // base score for having content
    if (input.rationale) completeness += 5;
    else issues.push("Missing rationale");
    if (input.alternatives?.length) completeness += 5;
    if (input.consequences) completeness += 5;

    // Richness (0-25): content length + enrichment
    let richness = 0;
    const contentLen = input.content.length;
    if (contentLen >= 200) richness += 10;
    else if (contentLen >= 100) richness += 5;
    else issues.push("Content is thin (< 100 chars)");
    if (input.summary) richness += 8;
    else issues.push("Missing AI summary");
    if (input.hasEmbedding) richness += 7;
    else issues.push("Missing embedding");

    // Connectivity (0-25): connections
    let connectivity = 0;
    if (input.connectionCount >= 3) connectivity = 25;
    else if (input.connectionCount >= 1) connectivity = 15;
    else { connectivity = 0; issues.push("No connections (orphan)"); }

    const total = freshness + completeness + richness + connectivity;

    return {
        total: Math.min(100, total),
        breakdown: { freshness, completeness, richness, connectivity },
        issues,
    };
}
```

- [ ] **Step 4: Run tests**

Run: `cd packages/api && pnpm vitest run src/chunks/health-score.test.ts`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add packages/api/src/chunks/health-score.ts packages/api/src/chunks/health-score.test.ts
git commit -m "feat: add chunk health score computation"
```

---

## Task 2: Expose Health Score in API

**Files:**
- Modify: `packages/api/src/chunks/service.ts` — Add health to detail response

- [ ] **Step 1: Add health score to getChunkDetail**

In `packages/api/src/chunks/service.ts`, in the `getChunkDetail` function. **Note:** The function uses `.pipe(Effect.flatMap(...))` chaining, NOT a local `result` variable. You need to add an `Effect.map` step at the end of the pipe chain:

```ts
import { computeHealthScore } from "./health-score";

// Add .pipe(Effect.map(...)) at the end of the existing chain:
.pipe(
    Effect.map(result => {
        const healthScore = computeHealthScore({
            content: result.chunk.content,
            updatedAt: result.chunk.updatedAt,
            summary: result.chunk.summary,
            rationale: result.chunk.rationale,
            alternatives: result.chunk.alternatives,
            consequences: result.chunk.consequences,
            connectionCount: result.connections.length,
            hasEmbedding: result.chunk.embedding != null,
        });
        return { ...result, healthScore };
    })
)
```

Read the actual function to understand the pipe chain before inserting.

- [ ] **Step 2: Commit**

```bash
git add packages/api/src/chunks/service.ts
git commit -m "feat: include health score in chunk detail API response"
```

---

## Task 3: Health Badge UI Component

**Files:**
- Create: `apps/web/src/features/chunks/chunk-health-badge.tsx`
- Modify: `apps/web/src/routes/chunks.$chunkId.tsx`

- [ ] **Step 1: Create ChunkHealthBadge**

```tsx
// apps/web/src/features/chunks/chunk-health-badge.tsx
import { Heart } from "lucide-react";

interface ChunkHealthBadgeProps {
    score: number;
    issues?: string[];
}

function getColor(score: number) {
    if (score >= 80) return "text-green-600";
    if (score >= 60) return "text-yellow-600";
    if (score >= 40) return "text-orange-600";
    return "text-red-600";
}

function getLabel(score: number) {
    if (score >= 80) return "Healthy";
    if (score >= 60) return "Fair";
    if (score >= 40) return "Needs attention";
    return "Poor";
}

export function ChunkHealthBadge({ score, issues }: ChunkHealthBadgeProps) {
    return (
        <div className="flex items-center gap-1.5">
            <Heart className={`size-3.5 ${getColor(score)}`} />
            <span className={`text-xs font-medium ${getColor(score)}`}>
                {score}/100 — {getLabel(score)}
            </span>
            {issues && issues.length > 0 && (
                <span className="text-muted-foreground text-xs">
                    ({issues.length} issue{issues.length > 1 ? "s" : ""})
                </span>
            )}
        </div>
    );
}
```

- [ ] **Step 2: Add to chunk detail page**

In `apps/web/src/routes/chunks.$chunkId.tsx`, render `<ChunkHealthBadge>` in the metadata area (near the created/updated dates). Pass `score={data.healthScore.total}` and `issues={data.healthScore.issues}`.

- [ ] **Step 3: Commit**

```bash
git add apps/web/src/features/chunks/chunk-health-badge.tsx apps/web/src/routes/chunks.\$chunkId.tsx
git commit -m "feat: show chunk health score badge on detail page"
```
