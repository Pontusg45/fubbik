# Plans pages improvements

**Status:** draft
**Scope:** `apps/web/src/routes/plans.index.tsx`, `apps/web/src/routes/plans.new.tsx`, `apps/web/src/routes/plans.$planId.tsx`, `apps/web/src/features/plans/*`, `packages/api/src/plans/*`, `packages/db/src/repository/plan.ts`

## Why

The plans pages today are read-mostly: the list has no filters/sort/progress, the detail page has dead buttons, task statuses beyond `done` are invisible, and task–chunk links render as hash fragments. This plan lands the missing operations in four phases so each ships independently.

## Phase 0 — API + data extensions (unlocks UI work)

Additive; no UI changes.

### 0.1 — `GET /api/plans` carries rollups per row

Extend the list response with:
- `taskCount: { total, done }`
- `codebaseName: string | null`
- `nextAction: string | null` — title of the first non-`done` task in `displayOrder`
- `lastActivityAt: Date | null` — most recent `plan_activity` / task update timestamp (distinct from `updatedAt`)

Files:
- `packages/db/src/repository/plan.ts` — extend `listPlans` with a `LEFT JOIN plan_task` for counts + MIN-position filtered task, and a join on `codebase.name`.
- `packages/api/src/plans/service.ts`, `routes.ts` — pass through + update `t.Object` response schema.
- **Test:** 0-task, N-task, all-done; archived filter.

### 0.2 — Task list carries chunk titles

Task chunks today come back as `{ id, chunkId, relation }` (see `plan-task-card.tsx:17`). The detail page can't render a usable link without a second round-trip. Extend the join in `GET /api/plans/:id` to include `chunk.title` and `chunk.type`.

- `packages/db/src/repository/plan.ts`: `getPlanDetail` — `INNER JOIN chunk ON chunk.id = plan_task_chunk.chunk_id`.
- **Test:** task with 2 chunks, assert `title` comes through.

### 0.3 — `POST /api/plans/:id/duplicate`

Used by the currently-dead Duplicate button (`plan-detail-header.tsx:84`). In one transaction:
1. Insert a new `plan` row with title `"${src.title} (copy)"`, status `draft`, same `codebaseId`, same `description`, metadata cloned.
2. Clone `plan_requirement`, `plan_analyze_item`, `plan_task` (reset `status` to `pending`), `plan_task_chunk`, `plan_task_dependency` (rewriting IDs).

Returns the new plan's id so the route handler can redirect.

### 0.4 — `DELETE /api/plans/:id` idempotent + cascading

Verify cascade on `plan_task`, `plan_requirement`, etc. (should already be `onDelete: cascade` in the schema — confirm and add tests.) Wire the dead Delete button in 1.x.

### 0.5 — Acceptance-criteria shape migration

`acceptanceCriteria` is a `JSONB string[]` today. To make the checkboxes persist (see 2.3), migrate in place to `JSONB { text: string; done: boolean }[]`. Read path accepts both shapes for a release; write path always emits the object shape. Seed + tests updated.

- `packages/db/src/schema/plan.ts`: no change, JSONB column takes either.
- `packages/api/src/plans/tasks.ts`: normalise on read, coerce on write.

### 0.6 — `POST /api/plans/:id/tasks/reorder`

Confirm existence (CLAUDE.md says it exists) and that it accepts an ordered `taskIds: string[]`. If not, add it. Needed for 2.4.

### 0.7 — `GET /api/plans/:id/activity`

Tiny endpoint returning the activity stream scoped to this plan (schema already has `activity` with entity columns per CLAUDE.md). Used by the audit sidebar in 3.2.

## Phase 1 — Plan list overhaul

All edits in `plans.index.tsx` plus a small filter header component.

### 1.1 — Status tabs + codebase filter + search

- URL-synced search params: `?status=active|draft|ready|completed|archived|all` (default `active` = in_progress + analyzing + ready) + `?codebase=<id>` + `?q=<search>`.
- Status tabs along the top, similar to the Requirements page.
- Codebase `<Select>` next to the tabs.
- Search input matches title + description (client-side on the already-loaded list, same pattern as tags page).

### 1.2 — Per-row progress + metadata

- Render `done/total` tasks + a 1-line progress bar per row (uses 0.1).
- Codebase chip (uses 0.1).
- Second line: `Next: <first non-done task title>` when plan is in progress (uses 0.1). Makes the list scannable.
- Use `lastActivityAt` instead of `updatedAt` for the right-hand timestamp.

### 1.3 — Row actions menu

Right-hand kebab on each row (DropdownMenu, same pattern as the new tag pill):
- Open (default on row click)
- Duplicate → 0.3
- Archive / Unarchive → PATCH status
- Delete → ConfirmDialog → 0.4

### 1.4 — New-plan page richer

- Add optional "Start from requirements…" — autocomplete picker (`/api/requirements`) that sends `requirementIds[]` in the POST body.
- Add a collapsed "Bootstrap tasks" textarea — one task title per line; POSTed as `tasks[]`.
- Markdown Preview tab on the description `<Textarea>` using the existing `MarkdownRenderer`.

## Phase 2 — Task UX

All edits under `features/plans/`.

### 2.1 — Status dropdown instead of boolean checkbox

- `plan-task-card.tsx:48` checkbox → small `<Select>` or badge-menu with all 5 statuses (`pending | in_progress | blocked | done | skipped`).
- Visual colour per status matches the existing `plan-status-pill` palette.
- `done` still strikes through the title.

### 2.2 — Real chunk links

- `plan-task-card.tsx:96-101`: render `task.chunks` as `<Link to="/chunks/:id">{title}</Link>` pills with the relation as a muted prefix (`context · Chunk title`). Uses 0.2.

### 2.3 — Working acceptance-criteria checkboxes

- `plan-task-card.tsx:85-89`: migrate to the object shape (0.5). Clicking toggles `done` and PATCHes the task. Persisted progress.
- Optional: show a `(N/M)` rollup next to the criteria header.

### 2.4 — Task reorder (drag handle)

- Use `@dnd-kit/core` + `@dnd-kit/sortable` (add dep).
- Drag handle icon on the left of each task row. Dropping fires 0.6.
- Keyboard fallback: `Alt+Up` / `Alt+Down` on a focused task.

### 2.5 — Task edit-in-place

- Expand view (`plan-task-card.tsx:77-104`) becomes editable:
  - Title — click to edit (same pattern as plan-detail-header).
  - Description — `<Textarea>` with a Save button.
  - Chunk attach/detach — a "+ chunk" button that opens a chunk picker (reuse `chunk-picker` / `combobox` if present).
- All PATCH via `/plans/:id/tasks/:taskId`.

### 2.6 — Task dependencies UI

- Expand view gets a "Depends on…" multi-picker listing other tasks in the plan.
- Adds/removes via the existing endpoints (or new ones if missing — confirm during execution).
- Visual: in the collapsed task row, a small "⬆ 2" when the task is blocked and a "⬇ 3" when other tasks wait on it.

### 2.7 — "Add task" accepts description

- `plan-tasks-section.tsx:46-62`: inline form keeps the title input but adds a collapsible description `<Textarea>` (toggled by a small "+ details" link). Shift+Enter submits with description.

## Phase 3 — Detail page polish

### 3.1 — Header fixes

- `plan-detail-header.tsx:29-33`: replace status cycle with an explicit `<Select>` dropdown that includes `archived`.
- Wire Duplicate (`:84`) → 0.3 then navigate.
- Wire Delete (`:87`) → `ConfirmDialog` → 0.4 then navigate back to `/plans`.
- Add codebase chip + requirements count + links chip (from external-link tables).
- Copy-URL icon (`Link2` from lucide) that toasts "URL copied".
- Export-markdown icon that renders the full plan (description + requirements + analyze + tasks + chunks) and opens the browser print dialog. Mirrors the docs-page print action.

### 3.2 — Activity sidebar

- Right-rail on lg viewports, sheet on small. Uses 0.7.
- Each entry: actor + action + target + relative time. Group by day.

### 3.3 — Burndown sparkline

- Small inline SVG sparkline in the sticky header showing `done %` over the last N days, computed from activity events. Optional; skip if 0.7 is deferred.

### 3.4 — Metadata surfaces

- Show `tokenEstimate` and `effortHours` (already persisted via the plan-flexibility work) as small muted chips in the header. Editable via a popover.

### 3.5 — Keyboard shortcuts

- List page: `/` focuses search, `j`/`k` move row focus, `Enter` opens.
- Detail page: `a` opens add-task, `x` on a focused task toggles done, `Cmd+D` duplicates.

## Ordering & effort

| Phase | Blocker for | Est. effort |
|-------|-------------|-------------|
| 0.1   | 1.1, 1.2, 1.3 | 2h |
| 0.2   | 2.2 | 1h |
| 0.3   | 1.3, 3.1 Duplicate | 2h |
| 0.4   | 1.3, 3.1 Delete | 30min (mostly confirmation the cascade works) |
| 0.5   | 2.3 | 2h |
| 0.6   | 2.4 | 1h (add if missing) |
| 0.7   | 3.2 | 1h |
| 1.1   | — | 2h |
| 1.2   | — | 1h |
| 1.3   | — | 1–2h |
| 1.4   | — | 2h |
| 2.1   | — | 1h |
| 2.2   | — | 30min |
| 2.3   | — | 1–2h |
| 2.4   | — | 3h (dnd-kit wiring) |
| 2.5   | — | 2h |
| 2.6   | — | 2–3h |
| 2.7   | — | 45min |
| 3.1   | — | 1.5h (Select + wire dead buttons + Copy/Export) |
| 3.2   | — | 2h |
| 3.3   | — | 1.5h |
| 3.4   | — | 30min |
| 3.5   | — | 1h |

**Ship order:**
- **PR 1 — Backend groundwork:** 0.1 + 0.2 + 0.3 + 0.4 + 0.5 + 0.6 + 0.7. One PR so the UI can be built against a stable API. Tests update alongside.
- **PR 2 — List + header value-payload:** 1.1 + 1.2 + 1.3 + 3.1. Fixes the "unusable at scale" + "dead buttons" bugs first.
- **PR 3 — Task UX core:** 2.1 + 2.2 + 2.3 + 2.5 + 2.7. The day-to-day-use improvements.
- **PR 4 — Reorder + dependencies:** 2.4 + 2.6. Heavier; batched together because they share the expand-view surface.
- **PR 5 — Polish:** 1.4 + 3.2 + 3.3 + 3.4 + 3.5. Optional per priority.

## Non-goals

- Plan comments / threaded discussion. The activity log (3.2) is enough for v1.
- Plan templates with variable placeholders. "Start from template" in 1.4 uses the existing template system as-is.
- Cross-plan dependencies (only within-plan via 2.6). Track in a later plan if we ever need epic-style rollups.
- Undo/redo for task operations. Actions are small and toast-confirmed; full history lives in the activity log.
- Burndown across multiple plans (roadmap-wide). 3.3 is per-plan only.
