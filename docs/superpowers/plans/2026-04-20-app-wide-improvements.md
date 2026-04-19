# App-wide improvements — next wave

**Status:** draft
**Scope:** cross-cutting (`apps/web/src/**`), with touches to `/chunks/*`, `/dashboard`, `/search`, `/graph`, and a small server ripple for testing infra.

## Why

Tags and plans are now in shape. The next-biggest leverage lives in things that affect every page: a command palette, global nav keys, consistent empty/error states, and a detail-page edit loop that doesn't bounce through a separate route. This plan bundles those wins with targeted polish on the heaviest-used pages (`/chunks/:id`, `/dashboard`, `/search`, `/graph`) and the infra gaps that keep biting us (web-side tests, loading skeletons, error boundaries).

Five PRs again, ordered so each is independently shippable.

## Phase 0 — Infra groundwork

### 0.1 — Web-side test harness

`apps/web` has **zero** tests today; the API has 109. When we break Eden types or wire mutations wrong, we find out in the browser. Add:
- `vitest` + `@testing-library/react` + `jsdom` env config in `apps/web/vitest.config.ts`.
- A shared `test-utils.tsx` wrapping: `QueryClientProvider`, TanStack Router `createMemoryHistory`, a mock `api` via `msw` or a hand-rolled eden mock.
- Three smoke tests: list filter on `/plans`, tag rename collision toast, task status dropdown fires a PATCH. Not exhaustive — just enough to ratchet further work.

### 0.2 — Route error boundary

Add `apps/web/src/components/route-error.tsx`: consumes `useRouteError` (if available) or wraps children in an `ErrorBoundary`. Renders a friendly panel with title, error message, "retry" (re-mount the route), and a collapsible stack trace. Wire via `errorComponent` in `__root.tsx` so every route inherits it.

### 0.3 — `PageEmpty` variants

`PageEmpty` exists but callers roll their own "no matches" and "nothing here yet" markup. Extend it with:
- `variant: "none" | "filtered" | "error"`
- Preset copy + icons for each
- Optional `onReset` for the filtered variant

Migrate `/plans`, `/tags`, `/chunks`, `/requirements`, `/codebases` in one pass.

### 0.4 — Loading skeletons

Replace generic `PageLoading count={6}` with per-page skeleton components that mirror the real layout:
- `ChunkCardSkeleton`, `PlanRowSkeleton`, `RequirementRowSkeleton`, `TagGroupSkeleton`.
- Each is a dumb Tailwind component with `animate-pulse`. Keep `PageLoading` as the fallback for pages without a specific skeleton.

### 0.5 — Search endpoint unification

The command palette (Phase 1) wants one endpoint that returns mixed-entity results. Either:
- Extend the existing `search` route with `entityTypes: string[]` (chunks/plans/tags/requirements/codebases/documents) — preferred, lower ripple.
- Or compose client-side across the existing entity endpoints.

Pick 1: add `GET /api/search/global?q=&types=chunk,plan,tag,...&limit=` returning a flat `{ entityType, id, title, subtitle, score }[]`.

## Phase 1 — Global navigation

### 1.1 — Command palette (`⌘K` / `Ctrl+K`)

- New `apps/web/src/features/cmdk/command-palette.tsx` using the existing `Command` primitive (base-ui/react).
- Open triggers: `Cmd/Ctrl+K` globally, a button in the header.
- Sections:
  - **Recent** — last 5 visited pages (localStorage).
  - **Results** — hits from 0.5, grouped by entity type.
  - **Actions** — "Create chunk", "Create plan", "Create requirement", "Open settings", "Switch codebase…".
- Enter navigates to the canonical route for the hit; `Shift+Enter` opens in a new tab.
- Debounce input 120ms; keyboard up/down arrows, Esc closes.

### 1.2 — Global nav hotkeys

- `apps/web/src/hooks/use-nav-hotkeys.ts`: listens on the document for `g` followed by a second key within 800ms.
- Mapping:
  - `g c` → `/chunks`
  - `g p` → `/plans`
  - `g r` → `/requirements`
  - `g g` → `/graph`
  - `g d` → `/dashboard`
  - `g s` → `/search`
  - `g t` → `/tags`
  - `g h` → `/knowledge-health`
- Skip when target is input/textarea/contenteditable (same pattern as page-level `/`).
- Mount the hook in `__root.tsx`.

### 1.3 — Hotkey discoverability

- Small `Kbd` chip in the header ("`⌘K`") and in the footer of the command palette ("Go to: `g` then `p`").
- A `?` key opens a help overlay listing every hotkey. Cheap but surfaces the whole system.

## Phase 2 — Chunk detail UX

### 2.1 — Edit-in-place on `/chunks/:id`

- Follow the docs-page pattern: each section (content, rationale, alternatives, consequences, summary) gets an inline `<textarea>` revealed on click, saves on blur, toasts on error.
- Keep `/chunks/:id/edit` as a deep-edit form (it has fields the detail doesn't surface — scope, aliases, notAbout, etc.) but the 90% flow stays on the detail page.
- Debounced autosave with 500ms idle; visual "Saved" pill.

### 2.2 — Similar-chunk merge

- The "Similar chunks" panel exists; add a `GitMerge` icon per row.
- Clicks open a confirm dialog ("Merge this chunk into the current one — their content + connections will be moved here, and the source deleted"). Reuses the tag-merge pattern.
- New endpoint `POST /api/chunks/merge { sourceId, targetId }`:
  1. Copy connections from source → target (dedupe on conflict).
  2. Copy tags from source → target.
  3. Copy file-refs, applies-to from source → target.
  4. For plan_task_chunk / plan_analyze_item rows pointing at source, repoint to target.
  5. For chunk_connection references, repoint.
  6. Append source's content to target under a `## Merged from "<source title>"` heading (optional — behind a flag).
  7. Delete source.

### 2.3 — Inline tag chips with the new menu

- Chunk detail currently renders tag pills; audit whether they use the `MoreHorizontal` + `DropdownMenuSub` pattern we just shipped on `/tags`. If not, unify so "detach", "edit tag", "assign type" all work without leaving the page.

## Phase 3 — Dashboard + search

### 3.1 — Dashboard "next action" panel

- New widget `plan-next-actions.tsx` that lists every non-archived plan with its `nextAction` (already in `listPlansWithRollups`). One row per plan → one-click deep link to the plan detail with that task scrolled into view.
- Slots above the existing Focus Stream feed.

### 3.2 — Unified quick-add

- Header gets a single `+` button → popover with four big choices (Chunk / Plan / Requirement / Task). Routes to the canonical `new` page with the right codebase + context pre-filled.
- Command palette gets the same via keyboard (`n c`, `n p`, etc.).

### 3.3 — Search URL-sync + scoped chips

- `/search` today resets filters on reload. Move all filter state to URL params (same as `/plans`).
- Small chip group above the input: "In: codebase X · tag Y" with `×` to remove. Chips also writable by clicking from a chunk/plan/tag row.
- Shareable links to scoped queries.

## Phase 4 — Graph + polish

### 4.1 — Saved graph views on dashboard

- `saved_graph` already exists per CLAUDE.md. Add a "My graph views" card to `/dashboard` showing up to 4 saved views, each with a snapshot thumbnail and a click-to-open.
- Nav gains a "Pin to sidebar" toggle per saved view so one can live in the sidebar.

### 4.2 — Graph node hover preview

- On `graph-node.tsx` hover (after 250ms), render a floating card with: title, summary, top 3 tags, chunk type. Uses the existing `Popover` primitive with `react-flow`'s viewport coordinates.
- Reduces click-exploration friction on large graphs.

### 4.3 — Auth re-enablement shape

- Not actually re-enabling (that's a product decision). Sketching the shape so it lands cleanly later:
  - `<RequireAuth>` wrapper for write routes in TanStack Router. Currently bypassed server-side; stub the client guard with a feature flag.
  - `/login` already exists; add a "Sign in to continue" banner that shows on write attempts when the feature flag is on.
  - Document the flip: one `isDev` → `READ_ONLY_WITHOUT_AUTH` boolean in `packages/api/src/index.ts` and the client reads the effective value from an `/api/auth/config` endpoint.

## Ordering & effort

| Phase | Item | Est. effort |
|-------|------|-------------|
| 0.1   | Web test harness | 2–3h |
| 0.2   | Route error boundary | 1h |
| 0.3   | PageEmpty variants + migration | 1.5h |
| 0.4   | Loading skeletons | 2h |
| 0.5   | `/api/search/global` | 2h |
| 1.1   | Command palette | 4–5h |
| 1.2   | Global nav hotkeys | 1h |
| 1.3   | Hotkey help overlay | 1h |
| 2.1   | Chunk edit-in-place | 3–4h |
| 2.2   | Similar-chunk merge | 3h (+ API) |
| 2.3   | Inline tag menu unification | 1h |
| 3.1   | Dashboard next-action panel | 1.5h |
| 3.2   | Unified quick-add | 2h |
| 3.3   | Search URL-sync + chips | 2h |
| 4.1   | Saved graph views on dashboard | 2–3h |
| 4.2   | Graph hover preview | 2h |
| 4.3   | Auth re-enablement shape | 2h |

**Ship order (5 PRs):**

- **PR 1 — Infra:** 0.1 + 0.2 + 0.3 + 0.4. Lands tests + error boundary + empty/loading consistency before the fun stuff.
- **PR 2 — Palette + nav:** 0.5 + 1.1 + 1.2 + 1.3. Compounding value — every future page gets free search and nav.
- **PR 3 — Chunk detail:** 2.1 + 2.2 + 2.3. Hits the highest-traffic page.
- **PR 4 — Dashboard + search:** 3.1 + 3.2 + 3.3. Makes the app feel cohesive rather than a grab-bag of routes.
- **PR 5 — Graph + auth shape:** 4.1 + 4.2 + 4.3. Polish + prep for re-enabling auth without a panic.

## Non-goals

- Rewriting the graph layout engine (we just did the Phase 1/1.5/2 force-directed work).
- Real-time collaboration / multi-user presence indicators. Single-user local-first remains the default.
- Mobile-first redesign. Responsive tweaks where cheap, but the target stays desktop.
- Migrating the client away from TanStack Router / React Query. Too much leverage in the current stack.
- Full i18n. English-only for now; leaving `t()` strings stubbed would be a premature abstraction.
