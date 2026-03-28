# Workflow & UI Expansions Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add PR review with knowledge, knowledge changelog, validate command, dashboard widgets, search preview, and bulk chunk editor.

**Architecture:** PR review fetches a GitHub diff and matches against chunks. Changelog queries recent changes. Validate checks filesystem against chunk refs. Dashboard uses localStorage for widget layout. Bulk editor extends existing selection system.

**Tech Stack:** Elysia, Effect, React, CLI, GitHub API (optional)

---

## Task 1: fubbik review <PR-url> (#13)

Fetch PR diff, match against chunks, output review checklist.

**Files:**
- Create: `apps/cli/src/commands/review-pr.ts`

- [ ] **Step 1:** Create command that:
1. Parses PR URL to extract owner/repo/number (or accepts a local diff file)
2. For GitHub PRs: `fetch("https://api.github.com/repos/{owner}/{repo}/pulls/{number}/files")` to get changed files
3. For local diffs: parse `git diff` output for file paths
4. For each changed file, query `/api/context/for-file?path=<file>`
5. Output a review checklist:

```
Review checklist for PR #123:

src/auth/middleware.ts (3 chunks):
  ✓ Check: Authentication System (convention)
  ✓ Check: Effect Error Handling (pattern)
  ✓ Check: API Endpoints: Chunks (reference)

src/db/schema.ts (1 chunk):
  ✓ Check: Database Schema: Chunks (schema)

2 files with no chunk coverage:
  ⚠ src/utils/new-helper.ts
  ⚠ src/types/index.ts
```

- [ ] **Step 2:** Support both `fubbik review https://github.com/org/repo/pull/123` and `fubbik review --diff <file>`.

- [ ] **Step 3:** Register in `apps/cli/src/index.ts`.

- [ ] **Step 4:** Commit.

---

## Task 2: fubbik changelog (#14)

Generate knowledge changelog between dates or git tags.

**Files:**
- Create: `apps/cli/src/commands/changelog.ts`

- [ ] **Step 1:** Create command:
- `fubbik changelog --since 7d` (default)
- `fubbik changelog --since 2026-03-01 --until 2026-03-28`

Uses the existing `GET /api/chunks?after=N&sort=updated` endpoint (same as `kb-diff` but with richer output).

Output format:
```markdown
# Knowledge Changelog (Mar 21 - Mar 28)

## New Chunks (12)
- [note] Auth Middleware Convention
- [document] Workspace Architecture
...

## Updated Chunks (5)
- Database Schema: Chunks (3 versions)
- Effect Error Handling (content updated)
...

## Plans Completed (2)
- Frontend Polish (6 steps)
- Documentation Improvement (5 steps)

## Requirements Status Changes
- "Graph layouts" → passing
- "Stale detection" → failing
```

Fetch plans via `GET /api/plans?status=completed` and requirements via `GET /api/requirements`.

- [ ] **Step 2:** Support `--format markdown` (default) and `--format json`.

- [ ] **Step 3:** Register in index.ts.

- [ ] **Step 4:** Commit.

---

## Task 3: fubbik validate (#15)

Check referential integrity against the filesystem.

**Files:**
- Create: `apps/cli/src/commands/validate.ts`

- [ ] **Step 1:** Create command that:
1. Fetches all chunks with file references: `GET /api/health/knowledge` (fileRefs section)
2. Fetches all chunks with appliesTo patterns
3. For each file reference, checks if the file exists on disk: `fs.existsSync(path)`
4. For each appliesTo glob, checks if any files match: use the glob-match utility
5. Reports broken references:

```
Validation Report:

Broken file references (3):
  ✗ src/old-module.ts (referenced by "Old Module Docs")
  ✗ packages/legacy/index.ts (referenced by "Legacy Package")

Applies-to patterns with no matches (1):
  ⚠ src/deprecated/** (used by "Deprecated Code Guide")

Valid references: 42/45 (93%)
```

- [ ] **Step 2:** Add `--fix` flag that removes broken file references via `PUT /api/chunks/:id/file-refs`.

- [ ] **Step 3:** Register. Commit.

---

## Task 4: Dashboard Widgets (#16)

Customizable dashboard with show/hide toggles for sections.

**Files:**
- Modify: `apps/web/src/routes/dashboard.tsx`

- [ ] **Step 1:** Read the dashboard. It has sections: Stats, Favorites, Recently Viewed, Recent Chunks, Health, Requirements, Activity.

- [ ] **Step 2:** Add a localStorage-persisted widget visibility state:
```tsx
const [visibleWidgets, setVisibleWidgets] = useLocalStorage<string[]>("fubbik-dashboard-widgets",
    ["stats", "favorites", "recent-viewed", "recent-chunks", "health", "requirements", "activity"]
);
```

- [ ] **Step 3:** Add a "Customize" button that opens a popover with toggles for each section.

- [ ] **Step 4:** Wrap each section in a conditional: `{visibleWidgets.includes("stats") && <StatCards ... />}`.

- [ ] **Step 5:** Commit.

---

## Task 5: Search Preview in Command Palette (#17)

Show content preview on search results.

**Files:**
- Modify: `apps/web/src/features/command-palette/command-palette.tsx`

- [ ] **Step 1:** Read the command palette. Chunk search results currently show title + type badge.

- [ ] **Step 2:** Add a `detail` field to chunk search results showing `chunk.content?.slice(0, 120)`. The command palette item's secondary text shows this preview.

- [ ] **Step 3:** The search results from the API already include `content` (or `summary`). Show whichever is available — prefer `summary` (shorter) over truncated `content`.

- [ ] **Step 4:** Commit.

---

## Task 6: Bulk Chunk Editor (#18)

Batch edit shared fields on multiple selected chunks.

**Files:**
- Modify: `apps/web/src/routes/chunks.index.tsx` — extend bulk action bar

- [ ] **Step 1:** Read the existing bulk action bar (appears when chunks are selected). It currently has: Add/Remove Tags, Set Type, Set Review Status, Archive, Delete.

- [ ] **Step 2:** Add "Move to Codebase" action:
- Dropdown showing available codebases
- On select, bulk-update chunk-codebase associations

Use `POST /api/chunks/bulk-update` with action `set_codebases` and the selected codebase ID. Read the bulk-update endpoint to understand supported actions.

- [ ] **Step 3:** Add "Set Type" dropdown if not already present (it may already exist from earlier work).

- [ ] **Step 4:** Commit.
