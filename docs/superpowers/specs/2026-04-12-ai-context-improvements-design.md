# AI Context Improvements

## Problem

The fubbik context system is file-centric: you give it a file path and it returns relevant chunks. AI agents often think in concepts, plans, or diffs — not individual file paths. The context output is flat text with no freshness signals, no staleness warnings, and no awareness of pending proposals. AI agents can't get context scoped to their current plan or task.

## Goal

Eight improvements to make the context system more useful for AI consumption:

1. **Plan-scoped context** — get all chunks relevant to a plan
2. **Concept-based context** — get chunks about a concept via semantic search
3. **Multi-file/glob context** — get context for multiple files at once
4. **Diff-aware context** — get context for changed files in a git diff
5. **Structured sections** — output grouped by type with health/staleness/proposal metadata
6. **Plan-aware MCP context** — MCP tool that merges plan + file context
7. **Task-scoped MCP context** — tightly-scoped context for a specific plan task
8. **Context snapshots** — freeze context at a point in time for stable AI work sessions

---

## Architecture

All features share a unified context pipeline:

```
Input Source → Chunk Resolver → Scorer + Budgeter → Structured Formatter → Output
```

- **Chunk Resolver** — each feature has its own resolver that produces candidate chunks with match reasons. Resolvers are composable (plan resolver + file resolver can be combined).
- **Scorer + Budgeter** — reuses the existing `scoreChunk()` + `estimateTokens()` + greedy selection from `packages/api/src/context-export/service.ts`.
- **Structured Formatter** — new: groups chunks by type into labeled sections with per-chunk metadata (health score, staleness flags, pending proposals).

---

## 1. Plan-Scoped Context

### API

`GET /api/context/for-plan?planId=<id>&maxTokens=<n>&codebaseId=<id>`

### Resolver

Collects chunks from three sources, deduplicates by chunk ID:

1. `plan_analyze_item` where `kind=chunk` — direct references from the analyze section
2. `plan_requirement` → `requirement_chunk` — chunks linked to the plan's requirements
3. `plan_task_chunk` — chunks linked to the plan's tasks

All three are queryable via existing repository functions (`listAnalyzeItems`, `listPlanRequirements` + `getRequirementsForChunks`, `listTaskChunks`).

### CLI

`fubbik context for-plan <planId>` — new subcommand under `context` group.

Flags: `--max-tokens <n>`, `--format <md|json>`, `--codebase <id>`

---

## 2. Concept-Based Context

### API

`GET /api/context/about?q=<concept>&maxTokens=<n>&codebaseId=<id>`

### Resolver

Combines three strategies, deduplicates:

1. **Semantic search** — existing `semanticSearchRepo()` returns top 20 chunks by embedding similarity. These get a +10 score boost.
2. **Tag match** — chunks tagged with terms matching the query string (case-insensitive substring match against tag names).
3. **Title/content text match** — existing `pg_trgm` fuzzy search.

If Ollama is unavailable (no embeddings), falls back to strategies 2 + 3 only.

### CLI

`fubbik context about "<concept>"` — new subcommand under `context` group.

Flags: `--max-tokens <n>`, `--format <md|json>`, `--codebase <id>`

---

## 3. Multi-File / Glob Context

### API

`GET /api/context/for-files?paths=<csv>&maxTokens=<n>&codebaseId=<id>`

Accepts comma-separated file paths or glob patterns (e.g., `src/auth/**/*.ts,src/middleware/*.ts`).

### Resolver

1. Expand globs by matching against `chunk_applies_to` patterns and `chunk_file_ref` paths in the database (no filesystem access needed — the patterns are already in the DB).
2. For each expanded path, run the existing `getContextForFile()` resolver.
3. Deduplicate by chunk ID. Chunks matched by multiple files keep the highest match score.

### CLI

`fubbik context for "<glob-or-path>"` — the existing `context for` command is extended. When the argument contains `*` or `**`, it switches to multi-file mode via the new endpoint. Non-glob arguments use the existing single-file endpoint as before.

---

## 4. Diff-Aware Context

### Implementation

CLI-side feature — no new API endpoint needed. The CLI runs `git diff`, extracts changed file paths, and calls the multi-file endpoint (Feature 3).

### CLI

`fubbik context for-diff [--staged]` — new subcommand under `context` group.

1. Runs `git diff [--staged] --name-only` locally
2. Passes the resulting file list to `GET /api/context/for-files?paths=<csv>`
3. Returns context for all changed files

Flags: `--staged` (diff staged changes only), `--max-tokens <n>`, `--format <md|json>`, `--codebase <id>`

---

## 5. Structured Sections in Output

Applies to ALL context output — features 1-4 and the existing `GET /api/chunks/export/context`.

### Section mapping

Chunks are grouped by type:

| chunk type | Section heading |
|---|---|
| `note` with tag containing "convention" | **Conventions** |
| `note` (other) | **Notes** |
| `document` | **Architecture** |
| `reference` | **API Reference** |
| `schema` | **Schemas** |
| `checklist` | **Checklists** |

Additionally, chunks with health score < 50 or active staleness flags are collected into a **Known Issues** section at the bottom (they also appear in their primary section).

### Per-chunk metadata

Each chunk in the output includes:

- **Health score** — `[health: N]` (0-100, from the existing `computeHealthScore`)
- **Staleness flag** — `⚠ STALE` if the chunk has an undismissed staleness flag (query `chunk_staleness` table)
- **Pending proposal** — `⚠ PENDING PROPOSAL` if the chunk has a pending `chunk_proposal` (query `chunk_proposal` table where `status=pending`)

### Markdown format

```markdown
## Conventions
[health: 85] Authentication uses JWT tokens stored in httpOnly cookies.
[health: 72] ⚠ STALE (120 days) API rate limiting: 100 req/min per user.

## Architecture
[health: 92] Session management flow: login → token → refresh → logout.

## API Reference
[health: 88] ⚠ PENDING PROPOSAL POST /api/auth/login endpoint.

## Known Issues
[health: 45] ⚠ STALE Error handling in auth middleware is incomplete.
```

### JSON format

Same structure but metadata as typed fields:

```json
{
    "sections": [
        {
            "heading": "Conventions",
            "chunks": [
                {
                    "id": "...",
                    "title": "JWT auth convention",
                    "content": "...",
                    "health": 85,
                    "stale": false,
                    "pendingProposal": false
                }
            ]
        }
    ],
    "knownIssues": [...]
}
```

### Opt-in

The structured format is a new `format` option: `format=structured-md` or `format=structured-json`. The existing `format=md` and `format=json` outputs remain unchanged for backward compatibility. The new format becomes the default for new endpoints (features 1-4) but is opt-in for the existing `GET /api/chunks/export/context`.

---

## 6. Plan-Aware MCP Context

### New MCP tool: `get_context`

A unified context tool that dispatches to the appropriate resolver based on which params are provided.

Parameters:
- `planId?: string` — scope to a plan (Feature 1 resolver)
- `filePath?: string` — scope to a file (existing resolver)
- `concept?: string` — scope to a concept (Feature 2 resolver)
- `maxTokens?: number` — token budget (default 4000)
- `codebaseId?: string` — codebase filter

When `planId` is provided alongside `filePath` or `concept`:
- Both resolvers run independently
- Results are merged and deduplicated
- Plan-scoped chunks get a +15 score boost (same pattern as the existing `forPath` boost)

Returns structured markdown (Feature 5 format).

### Existing `sync_claude_md` tool

Unchanged. It serves a different purpose (generating `.claude/CLAUDE.md` from tagged chunks). The new `get_context` tool is for in-session context retrieval.

---

## 7. Task-Scoped MCP Context

### New MCP tool: `get_context_for_task`

Parameters:
- `planId: string` (required)
- `taskId: string` (required)
- `maxTokens?: number` — token budget (default 4000)

Resolver:
1. Fetch the task's directly linked chunks (`plan_task_chunk` where `taskId`)
2. Fetch the plan's analyze chunks (`plan_analyze_item` where `kind=chunk`)
3. Fetch 1-hop connected chunks (via `connection` table) for all chunks from steps 1-2
4. Deduplicate. Task-linked chunks get +20 boost, analyze chunks get +10, connected chunks get +5.

Returns structured markdown (Feature 5 format), tightly scoped to what matters for that specific task.

---

## 8. Context Snapshots

### Data model

New table: `context_snapshot`

| column | type | notes |
|---|---|---|
| `id` | `text` pk | `$defaultFn(() => crypto.randomUUID())` |
| `userId` | `text` fk → `user.id` | who created it |
| `query` | `jsonb` not null | the input params that produced this snapshot |
| `chunks` | `jsonb` not null | array of `{ id, title, content, type, health, stale, pendingProposal }` — frozen at snapshot time |
| `tokenCount` | `integer` not null | total tokens in the snapshot |
| `createdAt` | `timestamp` not null | |

No TTL — snapshots persist until explicitly deleted.

### API

| method | path | notes |
|---|---|---|
| `POST` | `/api/context/snapshot` | body: `{ planId?, taskId?, filePaths?, concept?, maxTokens? }`. Runs the appropriate resolver, freezes the result, returns the snapshot. |
| `GET` | `/api/context/snapshot/:id` | Returns the frozen snapshot content in structured format. |
| `DELETE` | `/api/context/snapshot/:id` | Deletes the snapshot. |
| `GET` | `/api/context/snapshots` | Lists snapshots for the current user. |

### MCP tools

- `create_context_snapshot` — params mirror the `POST` body. Returns `{ snapshotId, tokenCount, chunkCount }`.
- `get_context_snapshot` — params: `snapshotId`. Returns the frozen content.

### CLI

- `fubbik context snapshot create --plan <id>` / `--task <planId> <taskId>` / `--about "<concept>"` / `--files <glob>`
- `fubbik context snapshot get <snapshotId>`
- `fubbik context snapshot list`
- `fubbik context snapshot delete <snapshotId>`

### Workflow

1. AI starts a work session: `create_context_snapshot planId=X taskId=Y`
2. AI reads the snapshot throughout the session: `get_context_snapshot snapshotId=abc`
3. The snapshot stays frozen even if chunks are edited mid-session
4. When the AI finishes and wants fresh context: create a new snapshot

---

## Files Changed

### New files

| Path | Responsibility |
|---|---|
| `packages/api/src/context/resolvers.ts` | Resolver functions for plan, concept, multi-file, diff |
| `packages/api/src/context/formatter.ts` | Structured section formatter with metadata |
| `packages/api/src/context/routes.ts` | New endpoints: for-plan, about, for-files |
| `packages/db/src/schema/context-snapshot.ts` | Snapshot table schema |
| `packages/db/src/repository/context-snapshot.ts` | Snapshot CRUD |
| `packages/api/src/context/snapshot-service.ts` | Snapshot create/get/list/delete |
| `packages/api/src/context/snapshot-routes.ts` | Snapshot API endpoints |
| `apps/cli/src/commands/context-for-plan.ts` | CLI: `context for-plan` |
| `apps/cli/src/commands/context-about.ts` | CLI: `context about` |
| `apps/cli/src/commands/context-for-diff.ts` | CLI: `context for-diff` |
| `apps/cli/src/commands/context-snapshot.ts` | CLI: `context snapshot` subgroup |

### Modified

| Path | Change |
|---|---|
| `packages/api/src/context-export/service.ts` | Extract `scoreChunk` + `estimateTokens` + budgeting into shared utils (or import from new `context/` module) |
| `packages/api/src/context-for-file/service.ts` | Export resolver function for reuse by multi-file feature |
| `packages/api/src/index.ts` | Mount new context routes + snapshot routes |
| `packages/mcp/src/context-tools.ts` | Add `get_context`, `get_context_for_task`, `create_context_snapshot`, `get_context_snapshot` tools |
| `packages/db/src/schema/index.ts` | Add snapshot schema export |
| `packages/db/src/repository/index.ts` | Add snapshot repo export |
| `apps/cli/src/commands/context-group.ts` | Add new subcommands: for-plan, about, for-diff, snapshot |
| `apps/cli/src/commands/context-for.ts` | Extend to handle glob patterns (multi-file mode) |

### Unchanged

- `packages/api/src/context-export/claude-md.ts` — CLAUDE.md generation stays as-is
- `packages/api/src/context-for-file/` — internal logic unchanged, just exported for reuse
- Existing `format=md` and `format=json` outputs — backward compatible

---

## Out of Scope

- **Freshness signal #5 (from original list)** — incorporated into Feature 5 (structured sections) as per-chunk metadata. Not a separate feature.
- **Automatic snapshot expiry** — snapshots don't expire. Manual deletion only.
- **Real-time snapshot invalidation** — snapshots are frozen by design. No push notifications when underlying chunks change.
- **Web UI for snapshots** — CLI and MCP only. No web page.
- **Streaming context** — all responses are complete documents, not streamed.
- **Cross-codebase context** — each feature operates within a single codebase scope.

---

## Success Criteria

- `GET /api/context/for-plan?planId=X` returns chunks from the plan's analyze items, requirements, and tasks
- `GET /api/context/about?q=auth` returns semantically relevant chunks about authentication
- `GET /api/context/for-files?paths=src/auth/*.ts` returns context for all matching files
- `fubbik context for-diff --staged` returns context for staged git changes
- All context endpoints support `format=structured-md` and `format=structured-json` with health/stale/proposal metadata
- MCP `get_context` tool combines plan + file + concept resolvers with score boosting
- MCP `get_context_for_task` returns tightly-scoped context for a specific task
- `POST /api/context/snapshot` freezes context that doesn't change when chunks are edited
- `GET /api/context/snapshot/:id` returns the frozen snapshot
- CLI commands: `context for-plan`, `context about`, `context for-diff`, `context snapshot [create|get|list|delete]` all work
