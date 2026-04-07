# Requirements & Plans Integration Design

## Problem

The core plumbing between requirements, plans, sessions, and chunks is solid — full M:N relationships, session auto-sync, MCP tooling. But four key systems don't leverage this data at all: CLAUDE.md export, context-for-file, health scoring, and staleness detection. The dashboard also underrepresents requirements/plans. The result is that requirements and plans feel like optional side features rather than integral parts of the knowledge lifecycle.

## Goal

Make requirements and plans visible everywhere knowledge is consumed — by AI agents (CLAUDE.md, context-for-file, MCP), by health monitoring (scoring, staleness), and by humans (dashboard). No new schema tables; this is purely about surfacing existing relationships.

---

## 1. CLAUDE.md Export Enhancement

**File:** `packages/api/src/context-export/claude-md.ts`

`GET /api/chunks/export/claude-md` currently outputs chunks grouped by type. Add three new sections appended after chunks, sharing the existing token budget.

### Requirements Section

Group by status: failing first, then untested, then passing. Each requirement shows:
- Title, priority (must/should/could/wont), status
- BDD steps (Given/When/Then) as a compact list
- Linked chunk titles (for cross-reference)
- `<!-- ACTION NEEDED -->` marker on failing/untested requirements

### Active Plans Section

Only plans with status `active`. Each shows:
- Title, completion percentage (e.g. "4/7 steps done")
- Pending step descriptions (skip completed/skipped steps)
- Exclude completed/archived plans

### Recent Sessions Section

Last 5 completed sessions. Each shows:
- Title, review status (pending/approved/rejected)
- Requirements addressed count
- Unresolved assumptions (if any)

### Token Budget

Requirements and plans are compact (a few lines each). They are appended after chunks within the existing token budget. If the budget is tight, chunks take priority — requirements/plans are trimmed last-in-first-out (sessions first, then plans, then requirements).

---

## 2. Context-for-File Enhancement

**File:** `packages/api/src/context-for-file/service.ts`

`GET /api/context/for-file?path=<path>` returns chunks relevant to a file. Extend to also surface requirements linked to those matched chunks.

### Lookup Logic

After matching chunks (via file-refs, appliesTo globs, dependency matching), query `requirement_chunk` for any requirements linked to matched chunk IDs. Deduplicate by requirement ID.

### Response Shape Change

```typescript
{
  chunks: ChunkWithMatchReason[],  // existing
  requirements: Array<{            // new
    id: string;
    title: string;
    status: "passing" | "failing" | "untested";
    priority: "must" | "should" | "could" | "wont";
    steps: Array<{ keyword: string; text: string }>;
    matchedChunkIds: string[];
  }>
}
```

`matchedChunkIds` indicates which file-relevant chunks triggered this requirement — useful for MCP and VS Code to explain why a requirement appeared.

### Downstream Consumers

- MCP `context-tools.ts`: agents see requirements when asking about a file
- VS Code extension: can show requirement badges on file-relevant chunks
- Web UI `/context` page: can list requirements alongside chunks

---

## 3. Health Score Rebalance

**File:** `packages/api/src/chunks/health-score.ts`

### Current: 4 dimensions at 0-25 each (max 100)

### New: 5 dimensions at 0-20 each (max 100)

**Existing dimensions (rescaled from 0-25 to 0-20):**
- **Freshness** (0-20): days since last update, same logic scaled down
- **Completeness** (0-20): has rationale/alternatives/consequences
- **Richness** (0-20): content length + AI enrichment (summary, aliases)
- **Connectivity** (0-20): number of connections

**New dimension:**
- **Coverage** (0-20): requirement backing
  - 0 — no requirements linked
  - 10 — linked to at least one requirement (any status)
  - 15 — all linked requirements are "passing"
  - 20 — all linked requirements passing AND chunk was referenced in a completed implementation session

### API Response Change

The chunk detail endpoint already returns `{ freshness, completeness, richness, connectivity, total }` in the health score breakdown. Add `coverage` to this object. The total remains max 100 (5 × 20).

### Implication

Chunks with no requirement links score max 80/100. This creates gentle pressure to link chunks to requirements without making unlinked chunks look broken.

---

## 4. Staleness Detection Enhancement

**Files:** `packages/api/src/staleness/service.ts` (or equivalent), `packages/db/src/repository/staleness.ts`

### New Staleness Reason: `requirement_failing`

When a requirement's status changes to "failing", create staleness flags for all chunks linked to that requirement via `requirement_chunk`.

- **Detail:** Requirement title and ID
- **Trigger:** `PATCH /api/requirements/:id` setting status to "failing", or session completion syncing a requirement to "failing"
- **Dismissable:** Yes (same as existing flags)

### New Staleness Reason: `requirement_uncovered`

During age-based scans (`POST /api/chunks/stale/scan-age`), flag chunks that are 30+ days old with zero requirement links.

- **Detail:** "No requirements linked — consider adding requirement coverage"
- **Trigger:** Extends existing age-based scan logic
- **Suppressible:** Yes, permanently per-chunk (some chunks like runbooks legitimately don't need requirements)

### Where These Surface

All existing staleness UI already works:
- Dashboard "Attention Needed" widget — new reasons appear alongside file-changed and age flags
- Chunk detail page — amber staleness banner
- Nav badge count — counts undismissed flags (new reasons add to count)
- `GET /api/chunks/stale` — already supports `reason` filter, new reason values just work

### Staleness Reason Enum

Current: `file_changed`, `age`, `diverged_duplicate`

New: add `requirement_failing`, `requirement_uncovered`

The `reason` column in `chunk_staleness` is a text field, so no schema migration needed — just new values.

---

## 5. Dashboard Enhancement

**File:** `apps/web/src/routes/dashboard.tsx`

### Expand Requirements Stat Card

Replace the single "Requirements" count with a breakdown:
- Passing count (green)
- Failing count (red)
- Untested count (amber)
- Each count links to `/requirements?status=<status>`

### Add "Active Plans" Widget

Below the stats bar, alongside existing recent chunks widget:
- Lists plans with status `active` (max 5)
- Each shows: title, progress bar (completed/total steps), last updated time
- Click navigates to `/plans/:id`
- Empty state: "No active plans"
- Data source: `GET /api/plans?status=active&limit=5`

### Add "Recent Sessions" Widget

Compact list of last 3 completed sessions:
- Title, completion date, requirements addressed count, review status badge (pending/approved/rejected)
- Click navigates to `/reviews/:sessionId`
- Empty state: "No recent sessions"
- Data source: `GET /api/sessions?limit=3`

No new API endpoints needed — all data is available from existing endpoints.

---

## Out of Scope

- New schema tables or migrations (all relationships already exist)
- Changes to the MCP tool set (existing tools already cover the workflow)
- VS Code extension changes (it can consume the enhanced context-for-file response in a future iteration)
- Requirement creation prompts on plan step completion (future feature)
- Automatic requirement generation from specs (future feature)
