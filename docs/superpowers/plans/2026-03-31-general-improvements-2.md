# General Improvements Plan (Round 2)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Improve architecture consistency, data quality, web UX, and CLI experience across 12 items.

**Architecture:** Four independent phases. Each produces shippable improvements.

**Tech Stack:** Commander.js, picocolors, bun, TanStack Router/Query, @xyflow/react, Elysia.

---

## Phase 1: CLI Architecture (Items 1-3)

Consolidate import commands, clarify local/server behavior, fix codebase detection.

---

### Task 1: Consolidate import commands

**Files:**
- Modify: `apps/cli/src/commands/import.ts` — expand to handle all import modes
- Delete: `apps/cli/src/commands/import-dir.ts`
- Delete: `apps/cli/src/commands/import-docs.ts`
- Modify: `apps/cli/src/index.ts` — remove old registrations

Currently three commands overlap:
- `import --file <path>` — JSON file or markdown directory (local store only)
- `import-dir <path>` — markdown directory (local store only)
- `import-docs <path> --codebase <name>` — markdown directory with frontmatter (server only)

- [ ] **Step 1: Read all three import commands**

Read `apps/cli/src/commands/import.ts`, `import-dir.ts`, and `import-docs.ts` to understand the full feature set.

- [ ] **Step 2: Rewrite import.ts to handle all modes**

Rewrite `apps/cli/src/commands/import.ts` to accept:
- `fubbik import <path>` — auto-detects: JSON file, single .md file, or directory of .md files
- `--server` — send to server API (uses `import-docs` endpoint with frontmatter parsing)
- `--local` — save to local store only (default when no server configured)
- `--codebase <name>` — codebase scope (implies `--server`)
- `--type <type>` — default chunk type
- `--no-recursive` — don't recurse subdirectories

Auto-detection logic:
```
if path ends with .json → JSON import
if path is a file ending with .md → single markdown file import
if path is a directory → recursive markdown directory import
```

When `--server` or `--codebase` is used, send to `/api/chunks/import-docs`. Otherwise, use local `addChunk()`.

- [ ] **Step 3: Update index.ts**

Remove `importDirCommand` and `importDocsCommand` imports and registrations. Keep only the consolidated `importCommand`.

- [ ] **Step 4: Delete old files**

Delete `apps/cli/src/commands/import-dir.ts` and `apps/cli/src/commands/import-docs.ts`.

- [ ] **Step 5: Test**

Run:
```bash
bun apps/cli/src/index.ts import CLAUDE.md --local
bun apps/cli/src/index.ts import docs/ --server --codebase fubbik
bun apps/cli/src/index.ts import export.json --local
```

- [ ] **Step 6: Commit**

```bash
git commit -m "refactor: consolidate import, import-dir, import-docs into single import command"
```

---

### Task 2: Add --local/--server flag awareness to commands

**Files:**
- Modify: `apps/cli/src/commands/list.ts`
- Modify: `apps/cli/src/commands/search.ts`
- Modify: `apps/cli/src/commands/recap.ts`

Currently `list` always uses local store, `recap` always uses server. Make behavior explicit.

- [ ] **Step 1: Update list.ts**

Read `apps/cli/src/commands/list.ts`. Add `--server` option. When `--server` is used (or when filtering by scope/exclude which requires server), fetch from server. Otherwise use local store. Show a note in output indicating which source was used.

- [ ] **Step 2: Update search.ts**

Read `apps/cli/src/commands/search.ts`. When server is available and `--semantic` is used, search via server. For text search, search local store by default, add `--server` to search via API.

- [ ] **Step 3: Update recap.ts**

`recap` already uses server. Add `--local` option that filters local store chunks by `updatedAt`. This makes `recap` work offline.

- [ ] **Step 4: Test all three**

```bash
bun apps/cli/src/index.ts list --server --limit 5
bun apps/cli/src/index.ts list --limit 5
bun apps/cli/src/index.ts search "convention" --server
bun apps/cli/src/index.ts recap --since 7d --local
```

- [ ] **Step 5: Commit**

```bash
git commit -m "feat: add --local/--server flag awareness to list, search, and recap commands"
```

---

### Task 3: Fix codebase auto-detection in compiled binary

**Files:**
- Modify: `apps/cli/src/lib/detect-codebase.ts`

`fubbik doctor` shows "Codebase not detected" even in the fubbik repo. The issue is likely that `execSync("git remote get-url origin")` fails or returns unexpected output in the compiled binary context.

- [ ] **Step 1: Debug the issue**

Add error logging to `getGitRemoteUrl()` in `apps/cli/src/lib/detect-codebase.ts`:
```typescript
export function getGitRemoteUrl(): string | null {
    try {
        const url = execSync("git remote get-url origin", {
            encoding: "utf-8",
            stdio: ["pipe", "pipe", "pipe"],
            cwd: process.cwd()  // explicitly set cwd
        }).trim();
        return url || null;
    } catch (err) {
        return null;
    }
}
```

The issue may be that `cwd` isn't set correctly in the compiled binary. Add explicit `cwd: process.cwd()` to the `execSync` call.

- [ ] **Step 2: Also fix detectCodebase to handle missing server gracefully**

In `detectCodebase()`, if `getServerUrl()` returns undefined, return null without making API calls.

- [ ] **Step 3: Test**

Run from the fubbik project root:
```bash
bun apps/cli/src/index.ts doctor
```
Expected: "Codebase detected (fubbik)".

- [ ] **Step 4: Rebuild and verify compiled binary**

```bash
cd apps/cli && pnpm build
fubbik doctor
```

- [ ] **Step 5: Commit**

```bash
git commit -m "fix: codebase auto-detection in compiled binary"
```

---

## Phase 2: Data Quality (Items 4-6)

Clean up low-quality chunks, normalize tags, add quality scoring.

---

### Task 4: Add `fubbik cleanup` command

**Files:**
- Create: `apps/cli/src/commands/cleanup.ts`
- Modify: `apps/cli/src/index.ts`

Identifies and optionally removes low-value chunks.

- [ ] **Step 1: Create cleanup command**

Create `apps/cli/src/commands/cleanup.ts`:

Checks:
1. **Imported plan tasks** — chunks with type `guide` and tags containing plan file names (e.g., `2026-03-06-project-hardening.md`). These are implementation artifacts, not knowledge.
2. **Near-empty chunks** — content < 50 characters.
3. **Duplicate titles** — chunks with identical titles.

```typescript
export const cleanupCommand = new Command("cleanup")
    .description("Find and remove low-value chunks")
    .option("--dry-run", "show what would be removed without removing")
    .option("--type <type>", "only check specific type")
    .action(...)
```

Output format:
```
Cleanup analysis:
  42 plan task artifacts (type: guide, tagged with plan filenames)
  3 near-empty chunks (< 50 chars)
  2 duplicate titles

Run 'fubbik cleanup --confirm' to remove.
```

- [ ] **Step 2: Register and test**

- [ ] **Step 3: Commit**

```bash
git commit -m "feat: add fubbik cleanup command for identifying low-value chunks"
```

---

### Task 5: Add `fubbik tag-normalize` command

**Files:**
- Create: `apps/cli/src/commands/tag-normalize.ts`
- Modify: `apps/cli/src/index.ts`

- [ ] **Step 1: Create tag-normalize command**

Identifies tag issues and fixes them:
1. **Merge duplicates** — `documentation`/`docs` → keep `docs`
2. **Remove overly broad** — tags with >70% coverage are too broad to be useful
3. **Remove filename tags** — tags that look like filenames (contain `.md`, `/`, etc.)

```typescript
export const tagNormalizeCommand = new Command("tag-normalize")
    .description("Clean up and normalize tags across all chunks")
    .option("--dry-run", "show what would change")
    .action(...)
```

Output:
```
Tag normalization:
  Merge: documentation → docs (79 chunks)
  Remove: plans (76 chunks, 65% coverage — too broad)
  Remove: 2026-03-06-project-hardening.md (looks like a filename)

Run 'fubbik tag-normalize --confirm' to apply.
```

- [ ] **Step 2: Register and test**

- [ ] **Step 3: Commit**

```bash
git commit -m "feat: add fubbik tag-normalize command for tag cleanup"
```

---

### Task 6: Add quality scoring to `fubbik lint`

**Files:**
- Modify: `apps/cli/src/commands/lint.ts`

- [ ] **Step 1: Read current lint command**

Read `apps/cli/src/commands/lint.ts` to understand current checks.

- [ ] **Step 2: Add `--score` flag**

Add a quality score (0-100) per chunk based on:
- Content length > 100 chars: +20
- Has rationale: +20
- Has connections (> 0): +15
- Has appliesTo patterns: +15
- Has meaningful tags (not just generic): +15
- Has type other than "note": +5
- Has summary (AI enrichment): +10

With `--score`, output sorted by lowest score:
```
Quality scores (lowest first):
  12/100  [guide] Task 14: Update CLAUDE.md
  15/100  [guide] Task 5: Rich Markdown Preview
  ...
  85/100  [convention] Use render prop pattern, not asChild
  90/100  [reference] Service Layer

Average: 42/100 | Below 50: 68 chunks | Above 80: 12 chunks
```

- [ ] **Step 3: Test**

Run: `bun apps/cli/src/index.ts lint --score`

- [ ] **Step 4: Commit**

```bash
git commit -m "feat: add quality scoring to fubbik lint --score"
```

---

## Phase 3: Web App (Items 7-9)

Dashboard onboarding, graph context menu, bulk tag editor.

---

### Task 7: Dashboard onboarding milestone cards

**Files:**
- Create: `apps/web/src/features/onboarding/milestone-cards.tsx`
- Modify: `apps/web/src/routes/dashboard.tsx`

- [ ] **Step 1: Create milestone cards component**

Create `apps/web/src/features/onboarding/milestone-cards.tsx`:

Shows actionable cards that disappear as milestones are hit. Check milestones from the stats query data:

```typescript
interface Milestone {
    key: string;
    title: string;
    description: string;
    action: { label: string; to: string };
    check: (stats: any) => boolean; // true = completed
}

const MILESTONES: Milestone[] = [
    {
        key: "first-convention",
        title: "Add your first convention",
        description: "Capture a coding pattern or rule that your team follows",
        action: { label: "Create Convention", to: "/chunks/new" },
        check: (stats) => (stats.conventionCount ?? 0) > 0
    },
    {
        key: "connect-chunks",
        title: "Connect two chunks",
        description: "Link related knowledge to build your graph",
        action: { label: "Open Graph", to: "/graph" },
        check: (stats) => (stats.connections ?? 0) > 0
    },
    {
        key: "import-docs",
        title: "Import documentation",
        description: "Bring in existing markdown docs from a folder",
        action: { label: "Import Docs", to: "/import" },
        check: (stats) => (stats.chunks ?? 0) > 10
    },
    {
        key: "applies-to",
        title: "Set up file patterns",
        description: "Link chunks to file paths so context-for works",
        action: { label: "View Chunks", to: "/chunks" },
        check: (stats) => (stats.appliesToCount ?? 0) > 0
    }
];
```

Track dismissed milestones in localStorage. Show as a horizontal row of small cards with check/todo indicators.

- [ ] **Step 2: Add to dashboard**

In `apps/web/src/routes/dashboard.tsx`, add `<MilestoneCards stats={stats} />` after the stats section but before recent chunks. Only show when there are incomplete milestones.

- [ ] **Step 3: Verify build and commit**

```bash
git commit -m "feat: add onboarding milestone cards to dashboard"
```

---

### Task 8: Graph right-click context menu

**Files:**
- Create: `apps/web/src/features/graph/graph-context-menu.tsx`
- Modify: `apps/web/src/features/graph/graph-view.tsx`

- [ ] **Step 1: Create context menu component**

Create `apps/web/src/features/graph/graph-context-menu.tsx`:

A floating menu that appears on right-click with actions:
- **On node right-click:** Edit, Open detail, Connect to..., Delete
- **On canvas right-click:** Create new chunk here, Fit view, Reset layout

Use absolute positioning based on the click coordinates. Style as a simple dropdown.

```typescript
interface GraphContextMenuProps {
    x: number;
    y: number;
    nodeId?: string;
    onClose: () => void;
    onAction: (action: string, nodeId?: string) => void;
}
```

- [ ] **Step 2: Wire into graph-view.tsx**

Add `onNodeContextMenu` and `onPaneContextMenu` handlers to ReactFlow. Track `contextMenu` state with `{x, y, nodeId?}`. Render the menu when state is set. Close on click outside or Escape.

- [ ] **Step 3: Implement "Create new chunk" action**

When "Create new chunk" is selected from canvas context menu, open a dialog to enter title/type, then create the chunk via API and add it as a node to the graph.

When "Connect to..." is selected from node context menu, enter connection mode (similar to the existing pending connection flow).

- [ ] **Step 4: Verify build and commit**

```bash
git commit -m "feat: add right-click context menu to knowledge graph"
```

---

### Task 9: Bulk tag spreadsheet editor

**Files:**
- Create: `apps/web/src/features/chunks/bulk-tag-editor.tsx`
- Modify: `apps/web/src/routes/chunks.index.tsx`

- [ ] **Step 1: Create bulk tag editor component**

A dialog/panel that shows selected chunks in a table with editable tag cells:

```
| Chunk Title                    | Tags                          |
|-------------------------------|-------------------------------|
| Use render prop pattern        | [convention] [ui] [base-ui] × |
| Backend: Repo -> Service       | [convention] [architecture] × |
| Effect error handling          | [convention] [effect] +       |
```

Each tag is a removable badge. A "+" button adds a new tag (with autocomplete from existing tags). Changes are batched and applied on "Save".

- [ ] **Step 2: Add trigger to chunks list**

Add a "Edit Tags" button to the bulk action bar that opens the bulk tag editor with the selected chunks.

- [ ] **Step 3: Verify build and commit**

```bash
git commit -m "feat: add bulk tag spreadsheet editor for selected chunks"
```

---

## Phase 4: CLI Quality of Life (Items 10-12)

Diff before sync, shell completions, and open command.

---

### Task 10: Add `fubbik diff` preview before sync

**Files:**
- Modify: `apps/cli/src/commands/sync.ts`

- [ ] **Step 1: Read current sync.ts**

Read `apps/cli/src/commands/sync.ts`.

- [ ] **Step 2: Add `--dry-run` flag**

Add a `--dry-run` option that shows what would happen without doing it:

```
Sync preview (dry run):
  → Push: 5 local-only chunks
  ← Pull: 3 server-only chunks
  ~ Modified: 2 chunks (local newer: 1, server newer: 1)

Run 'fubbik sync' to execute.
```

This reuses the existing comparison logic in `diff.ts` but integrates it into the sync workflow.

- [ ] **Step 3: Also add `--confirm` for destructive syncs**

If there are conflicts (both sides modified), prompt for confirmation unless `--confirm` is passed.

- [ ] **Step 4: Test and commit**

```bash
git commit -m "feat: add --dry-run to fubbik sync for preview before syncing"
```

---

### Task 11: Dynamic shell completions for chunk IDs

**Files:**
- Modify: `apps/cli/src/lib/completions.ts`

- [ ] **Step 1: Read current completions.ts**

Read `apps/cli/src/lib/completions.ts`.

- [ ] **Step 2: Add dynamic completions**

Enhance the zsh completions to:
1. Complete chunk IDs/titles for `fubbik get <TAB>`, `fubbik cat <TAB>`, `fubbik update <TAB>`, `fubbik remove <TAB>`
2. Complete template names for `fubbik add --template <TAB>`
3. Complete codebase names for `--codebase <TAB>`

For dynamic completions, the zsh script calls `fubbik list -q` to get IDs or uses a cache file:

```zsh
_fubbik_chunk_ids() {
    local -a chunks
    chunks=($(fubbik list -q 2>/dev/null | head -20))
    _describe 'chunk' chunks
}
```

- [ ] **Step 3: Test**

Run: `bun apps/cli/src/index.ts completions zsh | head -30`

- [ ] **Step 4: Commit**

```bash
git commit -m "feat: add dynamic zsh completions for chunk IDs, templates, and codebases"
```

---

### Task 12: Add `fubbik open` command

**Files:**
- Create: `apps/cli/src/commands/open.ts`
- Modify: `apps/cli/src/index.ts`

- [ ] **Step 1: Create open command**

```typescript
import { execSync } from "node:child_process";
import { Command } from "commander";
import { outputError } from "../lib/output";
import { getServerUrl } from "../lib/store";

const ROUTES: Record<string, string> = {
    dashboard: "/dashboard",
    graph: "/graph",
    chunks: "/chunks",
    requirements: "/requirements",
    plans: "/plans",
    import: "/import",
    settings: "/settings",
    health: "/knowledge-health",
    tags: "/tags",
    docs: "/docs",
};

function openUrl(url: string) {
    try {
        // macOS
        execSync(`open "${url}"`, { stdio: "ignore" });
    } catch {
        try {
            // Linux
            execSync(`xdg-open "${url}"`, { stdio: "ignore" });
        } catch {
            console.log(`Open: ${url}`);
        }
    }
}

export const openCommand = new Command("open")
    .description("Open fubbik in the browser")
    .argument("[target]", "page or chunk ID (e.g., dashboard, graph, <chunk-id>)")
    .action((target: string | undefined) => {
        const webUrl = "http://localhost:3001"; // could be made configurable

        if (!target) {
            openUrl(`${webUrl}/dashboard`);
            return;
        }

        // Check if it's a known route
        if (ROUTES[target]) {
            openUrl(`${webUrl}${ROUTES[target]}`);
            return;
        }

        // Assume it's a chunk ID
        openUrl(`${webUrl}/chunks/${target}`);
    });
```

- [ ] **Step 2: Register and test**

```bash
bun apps/cli/src/index.ts open graph
```

- [ ] **Step 3: Commit**

```bash
git commit -m "feat: add fubbik open command to launch browser pages"
```
