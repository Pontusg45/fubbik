# Search & Discovery Expansions Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add chunk recommendations, search operators, and bookmarks for better knowledge discovery.

**Architecture:** Recommendations use session co-reference patterns from `session_chunk_ref`. Search operators parse a structured query syntax client-side. Bookmarks use a new `chunk_bookmark` table.

**Tech Stack:** Elysia, Effect, Drizzle, React, CLI

---

## Task 1: Chunk Recommendations (#10)

"Frequently referenced together" based on session co-reference.

**Files:**
- Create: `packages/db/src/repository/recommendations.ts` — co-reference query
- Create: `packages/api/src/chunks/recommendations.ts` — service
- Modify: `packages/api/src/chunks/routes.ts` — add `GET /chunks/:id/recommendations`
- Modify: `apps/web/src/routes/chunks.$chunkId.tsx` — show recommendations section

- [ ] **Step 1:** Create `getRecommendations(chunkId)` repo function:
```sql
SELECT cr2.chunk_id, COUNT(*) as co_count
FROM session_chunk_ref cr1
JOIN session_chunk_ref cr2 ON cr1.session_id = cr2.session_id AND cr1.chunk_id != cr2.chunk_id
WHERE cr1.chunk_id = $1
GROUP BY cr2.chunk_id
ORDER BY co_count DESC
LIMIT 5
```
Returns chunks frequently referenced in the same sessions.

- [ ] **Step 2:** Add `GET /chunks/:id/recommendations` endpoint.

- [ ] **Step 3:** On chunk detail page, add a "Frequently Referenced With" section (inside a CollapsibleSection, default closed) showing recommended chunks.

- [ ] **Step 4:** Commit.

---

## Task 2: Search Operators (#11)

Structured search syntax: `type:document tag:auth updated:>7d rationale:exists`.

**Files:**
- Create: `apps/cli/src/lib/search-parser.ts` — parse operator syntax
- Modify: `apps/cli/src/commands/search.ts` — use parsed operators
- Modify: `apps/web/src/features/command-palette/command-palette.tsx` — parse operators in search input

- [ ] **Step 1:** Create parser:
```ts
interface ParsedSearch {
    text: string;           // free text portion
    type?: string;
    tags?: string[];
    after?: string;         // e.g., "7d"
    enrichment?: string;    // "missing" | "complete"
    origin?: string;        // "human" | "ai"
    reviewStatus?: string;
    minConnections?: string;
    hasRationale?: boolean;
}

function parseSearchQuery(query: string): ParsedSearch {
    const operators: Record<string, string> = {};
    const textParts: string[] = [];
    for (const token of query.split(/\s+/)) {
        const match = token.match(/^(\w+):(.+)$/);
        if (match) operators[match[1]!] = match[2]!;
        else textParts.push(token);
    }
    return {
        text: textParts.join(" "),
        type: operators.type,
        tags: operators.tag ? [operators.tag] : undefined,
        after: operators.updated?.replace(">", ""),
        enrichment: operators.enrichment,
        origin: operators.origin,
        hasRationale: operators.rationale === "exists",
    };
}
```

- [ ] **Step 2:** In CLI search command, parse the query and pass structured params to the API.

- [ ] **Step 3:** In command palette, detect operators and pass as separate query params. Show a hint: "Operators: type:note tag:auth updated:>7d".

- [ ] **Step 4:** Commit.

---

## Task 3: Chunk Bookmarks (#12)

Personal bookmarks with optional notes.

**Files:**
- Create: `packages/db/src/schema/bookmark.ts` — bookmark table
- Create: `packages/db/src/repository/bookmark.ts` — CRUD
- Create: `packages/api/src/bookmarks/routes.ts` — API
- Modify: `apps/web/src/routes/chunks.$chunkId.tsx` — bookmark button
- Modify: `apps/web/src/routes/dashboard.tsx` — bookmarks section

- [ ] **Step 1:** Create `chunk_bookmark` table:
```ts
{
    id: text PK,
    chunkId: text FK → chunk (cascade),
    userId: text FK → user (cascade),
    note: text (nullable),
    createdAt: timestamp
}
```
Unique constraint on (chunkId, userId). Export, push.

- [ ] **Step 2:** CRUD repo + routes:
- `POST /bookmarks` — add bookmark `{ chunkId, note? }`
- `GET /bookmarks` — list user's bookmarks
- `DELETE /bookmarks/:id` — remove
- `PATCH /bookmarks/:id` — update note

- [ ] **Step 3:** On chunk detail, add a bookmark button (different from favorite/pin — bookmarks have notes). Show a small modal for the note when bookmarking.

- [ ] **Step 4:** On dashboard, add a "Bookmarks" section showing bookmarked chunks with notes.

- [ ] **Step 5:** Commit.
