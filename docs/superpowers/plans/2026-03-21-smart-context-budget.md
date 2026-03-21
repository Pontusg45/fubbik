# Smart Context Budget Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make `fubbik context` smarter by prioritizing chunks with higher health scores, stronger relevance to the target, and recent updates — maximizing value per token.

**Architecture:** Extend the existing context export scoring algorithm to factor in health score, embedding freshness, and optional file-path relevance. The current scoring is: `connectionCount * 2 + typeScore + hasRationale`. The new scoring adds: `healthScore / 10 + freshnessBonus + relevanceBonus`. This replaces the hardcoded type-based scoring with a multi-factor ranking.

**Tech Stack:** Effect, existing context export service, health score computation

---

## File Structure

### Files to modify:
- `packages/api/src/context-export/service.ts` — Upgrade scoring algorithm
- `packages/api/src/context-export/routes.ts` — Add `forPath` query param
- `apps/cli/src/commands/context.ts` — Add `--for <path>` flag

---

## Task 1: Upgrade Context Scoring Algorithm

**Files:**
- Modify: `packages/api/src/context-export/service.ts`

- [ ] **Step 1: Read existing scoring**

Read `packages/api/src/context-export/service.ts` to understand the current `exportContext` function, especially lines where chunks are scored and selected.

Current scoring:
```ts
const typeScore = c.type === "document" ? 3 : c.type === "note" ? 1 : 2;
const hasRationale = c.rationale ? 2 : 0;
const score = connectionCount * 2 + typeScore + hasRationale;
```

- [ ] **Step 2: Replace with multi-factor scoring**

**IMPORTANT:** The scoring function must operate on the RAW Drizzle DB row type (which has `updatedAt`, `reviewStatus`, `embedding`, etc.), NOT on the stripped-down `ScoredChunk` interface. Apply the scoring BEFORE creating `ScoredChunk` objects — in the `.map()` that processes raw chunks.

Also: `computeHealthScore` already includes a freshness component in its 0-100 score. Don't double-count freshness with a separate `freshnessPoints`.

```ts
import { computeHealthScore } from "../chunks/health-score";

// Call this on the raw DB chunk row, before creating ScoredChunk
function scoreChunk(c: typeof chunk.$inferSelect, connectionCount: number): number {
    const health = computeHealthScore({
        content: c.content,
        updatedAt: c.updatedAt,
        summary: c.summary,
        rationale: c.rationale,
        alternatives: c.alternatives,
        consequences: c.consequences,
        connectionCount,
        hasEmbedding: c.embedding != null,
    });
    const healthPoints = health.total / 10; // 0-10 (includes freshness already)

    // Type relevance
    const typePoints = c.type === "document" ? 3 : c.type === "note" ? 1 : 2;

    // Rationale bonus
    const rationalePoints = c.rationale ? 2 : 0;

    // Connection weight (capped)
    const connectionPoints = Math.min(connectionCount * 2, 10);

    // Review status: approved > reviewed > draft
    const reviewPoints = c.reviewStatus === "approved" ? 2 : c.reviewStatus === "reviewed" ? 1 : 0;

    // NO separate freshnessPoints — already in healthPoints
    return healthPoints + typePoints + rationalePoints + connectionPoints + reviewPoints;
}
```

Apply `scoreChunk()` in the `.map()` step where raw chunks are processed, THEN create `ScoredChunk` objects with the computed score.

- [ ] **Step 3: Commit**

```bash
git commit -m "feat: upgrade context export with multi-factor chunk scoring"
```

---

## Task 2: File-Path Relevance Boost

**Files:**
- Modify: `packages/api/src/context-export/service.ts`
- Modify: `packages/api/src/context-export/routes.ts`

- [ ] **Step 1: Add `forPath` query param**

In `routes.ts`, add to the context export route:
```ts
forPath: t.Optional(t.String()),
```

- [ ] **Step 2: Boost chunks matching the target file**

When `forPath` is provided, fetch matching chunks from the context-for-file service and boost their scores:

```ts
if (query.forPath) {
    // Use the existing context-for-file service to find relevant chunks
    const fileContext = yield* getContextForFile(userId, query.forPath, query.codebaseId);
    const fileContextIds = new Set(fileContext.map(c => c.id));

    // Boost scores for file-relevant chunks
    for (const item of scoredChunks) {
        if (fileContextIds.has(item.id)) {
            item.score += 15; // significant relevance boost
        }
    }
}
```

This means chunks that directly reference the target file get priority in the token budget.

- [ ] **Step 3: Commit**

```bash
git commit -m "feat: add file-path relevance boost to context export"
```

---

## Task 3: CLI Integration

**Files:**
- Modify: `apps/cli/src/commands/context.ts`

- [ ] **Step 1: Read existing context command**

Read `apps/cli/src/commands/context.ts` to understand the current flags and fetch pattern.

- [ ] **Step 2: Add `--for <path>` flag**

```ts
.option("--for <path>", "boost chunks relevant to this file path")
```

Pass as `&forPath=<path>` to the API call.

When combined with `--max-tokens`, the smart scoring ensures the most relevant chunks for that file fill the budget first:

```bash
# Before: generic context, type-weighted
fubbik context --max-tokens 4000

# After: context optimized for a specific file
fubbik context --max-tokens 4000 --for src/auth/middleware.ts
```

- [ ] **Step 3: Commit**

```bash
git commit -m "feat(cli): add --for flag to context command for file-targeted context"
```
