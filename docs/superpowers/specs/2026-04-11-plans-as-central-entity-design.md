# Plans as a Central Entity

## Problem

Today `Plan` is a secondary entity in fubbik. It has a title, description, and a flat list of steps. Richer context — what the plan must satisfy, what the codebase looks like today, what's risky, what's assumed — lives scattered across:

- **Requirements** (linked only to individual steps via `plan_step.requirementId`, not to the plan itself)
- **Implementation sessions** (separate `implementation_session` tables that track chunks touched, assumptions discovered, and requirements addressed during execution)
- **Chunks** (linked to plans via `plan_chunk_ref` with a `context/created/modified` relation)

As a result, to see "what is this plan about, what must it satisfy, what have we figured out, and what's left to do?" you have to stitch together a plan, its steps, its sessions, its chunk refs, and its loosely-linked requirements. Plans are a thin coordinator, not a home.

We want plans to be the *home* for a unit of work: a single entity that holds the description, the requirements it addresses, the structured analysis of the codebase, and the tasks to execute. Everything else points into the plan, not the other way around.

## Goal

Redesign Plan as a four-part container:

1. **Description** — long-form markdown explaining what the plan is
2. **Requirements** — a plan-level list of existing `Requirement` entities the plan addresses
3. **Analyze** — structured, typed investigation fields: related chunks, affected files, risks, assumptions, open questions
4. **Tasks** — enriched execution items (title, description, acceptance criteria, multiple chunk links, dependencies)

Merge `implementation_session` into plans (sessions are gone). Swap `Requirements` out of the primary nav in favor of `Plans`. This is a **clean rewrite** — existing plan and session data is dropped.

---

## 1. Data Model

Six new tables replace the following dropped tables: `plan`, `plan_step`, `plan_chunk_ref`, `implementation_session`, `session_chunk_ref`, `session_assumption`, `session_requirement_ref`.

### `plan`

| column | type | notes |
|---|---|---|
| `id` | `uuid` pk | |
| `title` | `text` not null | |
| `description` | `text` | long-form markdown, nullable |
| `status` | `text` not null | `draft \| analyzing \| ready \| in_progress \| completed \| archived` |
| `userId` | `uuid` fk | owner |
| `codebaseId` | `uuid` fk nullable | optional codebase scope |
| `createdAt` | `timestamptz` not null | |
| `updatedAt` | `timestamptz` not null | |
| `completedAt` | `timestamptz` nullable | set when status → `completed`, cleared otherwise |

Indexes: `(userId)`, `(codebaseId)`, `(status)`.

### `plan_requirement` (many-to-many to existing `requirement`)

| column | type | notes |
|---|---|---|
| `id` | `uuid` pk | |
| `planId` | `uuid` fk → `plan.id` cascade delete | |
| `requirementId` | `uuid` fk → `requirement.id` cascade delete | |
| `order` | `integer` not null | for user-controlled ordering |
| `createdAt` | `timestamptz` not null | |

Unique: `(planId, requirementId)`. Index: `(requirementId)` for reverse lookup ("which plans address this requirement?").

### `plan_analyze_item`

One discriminated table holding all five analyze kinds. Keeps the count of tables down and makes ordering/CRUD uniform across kinds.

| column | type | notes |
|---|---|---|
| `id` | `uuid` pk | |
| `planId` | `uuid` fk → `plan.id` cascade delete | |
| `kind` | `text` not null | `chunk \| file \| risk \| assumption \| question` (CHECK constraint) |
| `order` | `integer` not null | |
| `chunkId` | `uuid` fk → `chunk.id` nullable | set when `kind=chunk`, else null |
| `filePath` | `text` nullable | set when `kind=file`, else null |
| `text` | `text` nullable | note for chunks/files; body for risks/assumptions/questions |
| `metadata` | `jsonb` not null default `{}` | kind-specific fields (see below) |
| `createdAt` | `timestamptz` not null | |
| `updatedAt` | `timestamptz` not null | |

Indexes: `(planId)`, `(planId, kind)`, `(chunkId)`.

**`metadata` shapes by `kind`:**

| kind | metadata |
|---|---|
| `chunk` | `{}` |
| `file` | `{ lineStart?: number, lineEnd?: number }` |
| `risk` | `{ severity: "low" \| "medium" \| "high" }` |
| `assumption` | `{ verified: boolean }` |
| `question` | `{ answer?: string, answered: boolean }` |

The shape is validated at the service layer (not in SQL) so adding kind-specific fields in future migrations is additive.

### `plan_task` (replaces `plan_step`)

| column | type | notes |
|---|---|---|
| `id` | `uuid` pk | |
| `planId` | `uuid` fk → `plan.id` cascade delete | |
| `title` | `text` not null | short summary |
| `description` | `text` nullable | longer body |
| `acceptanceCriteria` | `jsonb` not null default `[]` | array of strings |
| `status` | `text` not null | `pending \| in_progress \| done \| skipped \| blocked` |
| `order` | `integer` not null | |
| `createdAt` | `timestamptz` not null | |
| `updatedAt` | `timestamptz` not null | |

Indexes: `(planId)`, `(planId, status)`.

**No `parentTaskId`** — tasks are flat. If we need grouping later we add it in a follow-up.

### `plan_task_chunk` (many-to-many — replaces the single `plan_step.chunkId`)

| column | type | notes |
|---|---|---|
| `id` | `uuid` pk | |
| `taskId` | `uuid` fk → `plan_task.id` cascade delete | |
| `chunkId` | `uuid` fk → `chunk.id` cascade delete | |
| `relation` | `text` not null | `context \| created \| modified` |
| `createdAt` | `timestamptz` not null | |

Unique: `(taskId, chunkId, relation)`. Indexes: `(taskId)`, `(chunkId)`.

### `plan_task_dependency`

| column | type | notes |
|---|---|---|
| `id` | `uuid` pk | |
| `taskId` | `uuid` fk → `plan_task.id` cascade delete | |
| `dependsOnTaskId` | `uuid` fk → `plan_task.id` cascade delete | |
| `createdAt` | `timestamptz` not null | |

Unique: `(taskId, dependsOnTaskId)`. Index: `(dependsOnTaskId)` for the auto-unblock query.

**Auto-unblock behavior:** when a task's status flips to `done`, the service marks any `blocked` tasks whose `dependsOnTaskId` is this task as `pending`. Only `blocked → pending`; tasks in other states are not touched.

### What's Gone

- `plan_chunk_ref` — plan-level chunk linking rolls into `plan_analyze_item` with `kind=chunk`
- `plan_step.requirementId` — requirements link at the plan level via `plan_requirement`, not the task level
- `plan_step.parentStepId` — no nesting
- `plan_step.note` — folded into `plan_task.description`
- `implementation_session`, `session_chunk_ref`, `session_assumption`, `session_requirement_ref` — the whole sessions subsystem

---

## 2. Status Lifecycle

The `status` column is an enum with six values:

| status | meaning | set by |
|---|---|---|
| `draft` | just created, still figuring out what this plan is | user, on create (default) |
| `analyzing` | actively filling in requirements + analyze fields; tasks may not exist yet | user |
| `ready` | analyze + tasks drafted, work can begin | user |
| `in_progress` | at least one task has moved past `pending` | user (optional: service can auto-advance — out of scope for v1) |
| `completed` | plan is done; sets `completedAt = now()` | user |
| `archived` | hidden from default lists but preserved | user |

**No gating.** All transitions are free. The user can jump from `draft` to `in_progress`, move backwards, or skip labels entirely. The labels are advisory.

**Side effects (the only automatic behavior):**
- Moving *into* `completed` sets `completedAt = now()`
- Moving *out of* `completed` clears `completedAt`

**List filters:** `GET /api/plans` defaults to showing all statuses except `archived`. A `?includeArchived=true` query flag brings them back.

---

## 3. API Surface

The entire `/api/plans/*` surface is replaced. All `/api/sessions/*` routes are deleted.

### Plan CRUD

| method | path | body/query | notes |
|---|---|---|---|
| `GET` | `/api/plans` | `codebaseId?`, `status?`, `requirementId?`, `includeArchived?` | list |
| `POST` | `/api/plans` | `{ title, description?, codebaseId?, requirementIds?: string[], tasks?: Array<{title, description?, acceptanceCriteria?: string[]}> }` | create, optional initial tasks |
| `GET` | `/api/plans/:id` | — | full detail: plan + requirements + analyze (grouped by kind) + tasks + task-chunk links + dependencies |
| `PATCH` | `/api/plans/:id` | `{ title?, description?, status?, codebaseId? }` | update |
| `DELETE` | `/api/plans/:id` | — | cascade deletes all child rows |

### Plan-level requirements

| method | path | body |
|---|---|---|
| `POST` | `/api/plans/:id/requirements` | `{ requirementId }` |
| `DELETE` | `/api/plans/:id/requirements/:requirementId` | — |
| `POST` | `/api/plans/:id/requirements/reorder` | `{ requirementIds: string[] }` |

### Analyze items

One set of routes for all five kinds; `kind` lives in body/query.

| method | path | body/notes |
|---|---|---|
| `GET` | `/api/plans/:id/analyze` | returns `{ chunks: [], files: [], risks: [], assumptions: [], questions: [] }` |
| `POST` | `/api/plans/:id/analyze` | `{ kind, chunkId?, filePath?, text?, metadata? }` — service validates kind-specific fields |
| `PATCH` | `/api/plans/:id/analyze/:itemId` | `{ text?, metadata?, chunkId?, filePath? }` |
| `DELETE` | `/api/plans/:id/analyze/:itemId` | — |
| `POST` | `/api/plans/:id/analyze/reorder` | `{ kind, itemIds: string[] }` |

### Tasks

| method | path | body |
|---|---|---|
| `POST` | `/api/plans/:id/tasks` | `{ title, description?, acceptanceCriteria?: string[], chunks?: Array<{chunkId, relation}>, dependsOnTaskIds?: string[] }` |
| `PATCH` | `/api/plans/:id/tasks/:taskId` | any task field |
| `DELETE` | `/api/plans/:id/tasks/:taskId` | — |
| `POST` | `/api/plans/:id/tasks/reorder` | `{ taskIds: string[] }` |
| `POST` | `/api/plans/:id/tasks/:taskId/chunks` | `{ chunkId, relation }` |
| `DELETE` | `/api/plans/:id/tasks/:taskId/chunks/:linkId` | — |

### Removed Routes

- `GET/POST/PATCH/DELETE /api/sessions/*` — entire session subsystem
- `POST /api/plans/import-markdown` — markdown import, parked
- `POST /api/plans/generate-from-requirements` — templated generation, parked
- `GET /api/plans/templates` — plan templates, parked
- `POST /api/plans/:id/chunks` / `DELETE /api/plans/:id/chunks/:refId` — plan-level chunk refs, now live as analyze items with `kind=chunk`

### MCP Tools

Regenerate `packages/mcp/src/plan-tools.ts` to match the new API. New tool set:

- `create_plan`
- `list_plans`
- `get_plan`
- `update_plan`
- `link_requirement` / `unlink_requirement`
- `add_analyze_item` / `update_analyze_item` / `delete_analyze_item`
- `add_task` / `update_task` / `delete_task`
- `link_task_chunk`

Delete all session-related MCP tools (`begin_implementation`, session updates, etc.) from `packages/mcp/src/session-tools.ts`.

---

## 4. UI & Navigation

### Primary nav swap

Before (in `apps/web/src/routes/__root.tsx`):

```
Dashboard · Chunks · Graph · Requirements
```

After:

```
Dashboard · Chunks · Graph · Plans
```

### Manage dropdown

- **Add:** `Requirements` entry (icon: `ClipboardList`) to the top "Navigate" section, next to Features / Docs
- **Remove:** `Reviews` entry (the `/reviews` route is deleted with sessions)
- Everything else in Manage is unchanged

### Routes changed or removed

| route | status |
|---|---|
| `/plans` | stays; list UI rebuilt for the richer plan shape |
| `/plans/new` | stays; simplified form (title + description + optional codebase) |
| `/plans/$planId` | replaced with the new four-section detail layout (see Section 5) |
| `/requirements` | stays, still tabbed; **the Plans tab is removed** |
| `/reviews` | **deleted** |
| `/reviews/$sessionId` | **deleted** |

### Dashboard widget

- The existing "Recent Plans" / "Active Plans" widget stays but renders the richer plan card: title, status pill, requirement count, task progress (`3/7`), time since last update
- The existing "Review Queue" widget (sessions) is **removed**
- The "Attention Needed" staleness widget is unchanged

### File layout

- **Delete:** `apps/web/src/routes/reviews*`, `apps/web/src/features/reviews/`, `apps/web/src/features/sessions/` (if present)
- **Rebuild:** `apps/web/src/features/plans/` — new components for the four-section detail layout
- **Unchanged:** `apps/web/src/features/requirements/` (the requirements entity itself is not touched)

---

## 5. Plan Detail Page Layout

The detail page at `/plans/$planId` is a single scrollable column with a sticky header and an optional right rail on wide screens.

### Sticky header

Always visible at top:

- Title — inline editable
- Status pill — click to cycle through `draft → analyzing → ready → in_progress → completed`; long-press / dropdown menu to jump to any status (including `archived`)
- Progress counter (`3 / 7 tasks`) and a thin progress bar
- Meta row: codebase badge, last updated, created-by
- Actions: `Archive`, `Delete`, `Duplicate`

### Section 1 — Description

- Full-width markdown editor, collapsible
- Collapsed by default if over 200 characters (shows first 3 lines + "expand")
- Inline edit on click, autosave on blur
- Empty state: "Describe what this plan is about"

### Section 2 — Requirements

- Header: `Requirements` · count · `+ Add` button
- `+ Add` opens a picker that searches existing requirements by title (reuses `GET /api/requirements` with `search` param)
- List of requirement cards showing:
  - Title
  - Status pill (`passing | failing | untested`)
  - Priority pill (`must | should | could | wont`)
  - Chevron → navigates to `/requirements/$id`
- Drag-to-reorder (`POST /api/plans/:id/requirements/reorder`)
- Empty state: "No requirements linked. Add one to document what this plan must satisfy."

### Section 3 — Analyze

- Header: `Analyze` with five collapsible sub-sections, each with an item count
- Sub-sections in order: `Chunks · Files · Risks · Assumptions · Questions`
- Each sub-section is a flat list of items with drag-to-reorder, inline edit, and delete

| sub-section | row shape | add button |
|---|---|---|
| **Chunks** | chunk title + chunk type badge + note field | `+ Add chunk` → chunk picker (reuses existing picker) |
| **Files** | file path + optional `L10-20` range + note | `+ Add file` → plain text input |
| **Risks** | text + severity dropdown (`low \| medium \| high`) | `+ Add risk` |
| **Assumptions** | text + `verified` checkbox | `+ Add assumption` |
| **Questions** | text + answer field + `answered` checkbox | `+ Add question` |

Each sub-section is collapsible; collapsed sub-sections show only the count. Empty sub-sections show a one-line hint.

### Section 4 — Tasks

- Header: `Tasks` · `2 / 7 done` · `+ Add task` button
- Body: vertical list of task cards

Task card (collapsed):

- Status checkbox — toggles `pending ↔ done`; right-click or status menu for `in_progress | blocked | skipped`
- Title (bold) + one-line description (truncated)
- Small chunk chips (first 3, "+N more" if overflow)
- Dependency indicator if `blocked` because of another task

Task card (expanded, click to toggle):

- Full description (markdown-rendered)
- Acceptance criteria — checklist of the `acceptanceCriteria` array, purely visual (not persisted state)
- All linked chunks with relation type badges
- Dependencies: `Depends on: Task 3, Task 5`
- Inline edit for each field

**Keyboard shortcuts within the tasks section:**

- `j / k` — navigate between tasks
- `space` — toggle current task `pending ↔ done`
- `n` — create new task (focuses the title input)
- `enter` — expand/collapse current task

Drag-to-reorder tasks.

### Right rail (desktop `≥1280px`)

- Compact summary: status, progress ring, linked requirement count, analyze item counts by kind, tasks remaining
- "Jump to" anchor links: Description · Requirements · Analyze · Tasks
- Collapses below 1280px — sections remain in the main column

### Create flow

`/plans/new` is a short form: `title` + `description` + optional `codebase`. On submit, POST and redirect to `/plans/$id`, where the user fills in the richer fields. No wizard.

---

## 6. Migration

"Wipe it" approach: drop all existing plan and session data, create the new schema fresh. Accepted because this is a local-first dev tool and current plan data is mostly seed/scratch.

### Single Drizzle migration

One migration file that drops old tables and creates new ones in this order:

1. Drop `session_requirement_ref`
2. Drop `session_assumption`
3. Drop `session_chunk_ref`
4. Drop `implementation_session`
5. Drop `plan_chunk_ref`
6. Drop `plan_step`
7. Drop `plan`
8. Create `plan` (new schema)
9. Create `plan_requirement`
10. Create `plan_analyze_item` (with `kind` CHECK constraint)
11. Create `plan_task`
12. Create `plan_task_chunk`
13. Create `plan_task_dependency`

### Seed data

`scripts/seed.ts` updated to create three sample plans in the new shape:

1. One `completed` plan with a few tasks all done
2. One `in_progress` plan with requirements, a partial analyze section, and 2/5 tasks done
3. One `analyzing` plan with requirements and analyze items but no tasks yet

Chunk and requirement seed data is unchanged.

### Code cleanup (same PR)

- Delete `packages/api/src/sessions/` (routes, service, repository, schemas)
- Delete `packages/db/src/repository/session.ts`
- Delete `packages/db/src/schema/session.ts` (or the session parts of a shared schema file)
- Delete `apps/web/src/routes/reviews*` and `apps/web/src/features/reviews/` (and any `features/sessions/` directory)
- Remove session-related MCP tools from `packages/mcp/`
- Remove any `fubbik session*` commands from the CLI (`apps/cli/`)
- Update `fubbik plan*` CLI commands (`create`, `list`, `show`, `step-done`, `add-step`, `activate`, `complete`) to match the new API shape — rename step commands to task commands, drop `import` (markdown import is parked)
- Update `packages/db/src/schema/index.ts` and type exports to drop session exports
- Update CLAUDE.md "Core Concepts" and "API Endpoints" sections to reflect the new plan shape and removed sessions

### No rollback

Accepted. If you need old data back, it's in git history.

---

## 7. Out of Scope

Explicitly *not* in this spec (follow-ups if needed):

- **Markdown import** — the old `POST /api/plans/import-markdown` is gone. Can come back later.
- **Generate-from-requirements** — templated plan generation (`requirement-standard`, `requirement-detailed`) is gone. Users link existing requirements manually.
- **Plan templates** — the five built-in templates are gone. Users start from a blank plan.
- **Task nesting / milestones** — tasks are flat. No `parentTaskId`. Revisit if phases become necessary.
- **Multi-user assignment** — tasks have no `assigneeId`.
- **Task time tracking** — no estimates, no logged time.
- **Cross-plan analytics** — no "all risks across plans" view. Analyze items are scoped to their plan.
- **Plan versioning / history** — no append-only version table. Git/backups handle recovery.
- **AI-assisted analyze population** — no "analyze this plan for me" LLM call that fills risks/assumptions. Future feature.
- **Comments / discussion threads** — not included.
- **Notifications** — not included.
- **External issue tracker sync** (Linear, GitHub Issues) — not included.

---

## Success Criteria

- The `plan` table holds title, description, status (six-label enum, ungated), and the owner/codebase scope
- `plan_requirement` links plans to existing `requirement` entities many-to-many
- `plan_analyze_item` holds the five analyze kinds (chunk, file, risk, assumption, question) in one discriminated table
- `plan_task` holds enriched tasks (title + description + acceptance criteria); each task can link multiple chunks via `plan_task_chunk`
- Session tables and routes are fully deleted from the codebase
- Primary nav shows `Dashboard · Chunks · Graph · Plans`; `Requirements` moves into the Manage dropdown
- `/plans/$planId` renders a sticky header + four stacked sections (Description, Requirements, Analyze, Tasks)
- `POST /api/plans` creates a plan; `GET /api/plans/:id` returns the full plan with all four sections populated
- Auto-unblock still works: marking a task `done` flips dependent `blocked` tasks to `pending`
- The seed script creates three sample plans that exercise each of the new sections
- `pnpm run check-types`, `pnpm lint`, `pnpm build`, and `pnpm test` all pass
