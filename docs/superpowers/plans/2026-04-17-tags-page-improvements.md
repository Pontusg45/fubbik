# Tags page improvements

**Status:** draft
**Scope:** `apps/web/src/routes/tags.tsx`, `packages/api/src/tags/`, `packages/api/src/tag-types/`, `packages/db/src/repository/`

## Why

The tags page today only lets you delete tags. It can't create, rename, merge, reassign types, or show how each tag is used. That makes it impossible to clean up tag drift (e.g. `front-end` vs `frontend`) without breaking chunk links. This plan lands the missing operations in three phases so each phase is independently shippable.

## Phase 0 — API extensions (unlocks UI work)

Purely additive; no UI changes yet.

### 0.1 — `GET /api/tags` returns per-tag chunk counts

- `packages/db/src/repository/tag.ts` (or whichever file owns `listTags`): add `LEFT JOIN chunk_tag GROUP BY tag.id` and return `chunkCount: number` alongside existing fields.
- `packages/api/src/tags/service-new.ts`: pass through.
- `packages/api/src/tags/routes.ts`: extend response schema (`chunkCount: t.Number()`).
- **Test:** `packages/api/src/tags/service-new.test.ts` — create 1 tag with 3 chunks, 1 tag with 0, assert counts.

### 0.2 — `PATCH /api/tags/:id` (rename + change type)

- Body: `{ name?: string; tagTypeId?: string | null }`.
- Service validates unique name per user, returns 409 on collision.
- Repository `updateTag(id, userId, patch)` returns the updated row.
- **Test:** rename conflict → 409; successful rename → existing `chunk_tag` rows unchanged.

### 0.3 — `POST /api/tags/merge`

- Body: `{ sourceId: string; targetId: string }`.
- In a single transaction:
  1. `UPDATE chunk_tag SET tag_id = :targetId WHERE tag_id = :sourceId`
  2. De-dupe `(chunk_id, tag_id)` duplicates created by the update (one chunk already had both tags).
  3. `DELETE FROM tag WHERE id = :sourceId`.
- Returns `{ targetId, chunkCount }` so the UI can refresh.
- **Test:** merge case where a chunk had both tags (dedupe path); merge where only source existed (no dedupe).

### 0.4 — Tag-type icon field (for phase 3, but migration fits with phase 0)

- `packages/db/src/schema/tag-type.ts`: add `icon: text("icon")` nullable.
- Drizzle generate + push. No data migration (null = existing).
- Extend `POST`/`PATCH /api/tag-types` body schemas + list response.

## Phase 1 — Core management UX

All edits in `apps/web/src/routes/tags.tsx` plus one new menu component.

### 1.1 — Create Tag from the page

- Replace the empty-state CTA (line ~206) that misleadingly opens `/chunks/new`.
- Add a `+` button next to the search input (top of main column) that reveals an inline form: `name` + optional `tagTypeId` select. Submit → `POST /api/tags`, invalidate `["tags"]`.
- Empty-state CTA points at the same inline form.

### 1.2 — Per-tag action menu (replaces hover-X)

- New component `apps/web/src/features/tags/tag-actions-menu.tsx` using the existing `DropdownMenu` primitive.
- Trigger: `MoreHorizontal` on each pill, visible on hover *and* on touch devices (use `focus-within` + always-visible on coarse pointers via `@media (pointer: coarse)`).
- Items:
  - **Rename** → inline editable text field on the pill (Enter saves, Esc cancels). Calls 0.2.
  - **Merge into…** → opens dialog with searchable tag picker, then calls 0.3. Confirm shows target's current chunk count and the number of links about to move.
  - **Assign type** → submenu listing existing tag types + "None". Calls 0.2 with `tagTypeId`.
  - **Delete** → existing `ConfirmDialog` path.
- Remove the hover-only `X` button (current `tags.tsx:249`).

## Phase 2 — Discovery & scale

### 2.1 — Usage count badge on each pill

- Render `chunkCount` from 0.1 as a small muted number after the tag name. Hide when `chunkCount === 0` — let the "unused" filter (2.3) surface those.

### 2.2 — Sort dropdown

- `Select` next to search: Alphabetical (default) / Most used / Recently used.
- "Recently used" requires a `lastUsedAt` column on `tag` populated whenever a `chunk_tag` row is inserted. Out of scope for v1 — ship with the two sorts that don't need schema changes, leave "Recently used" disabled with a tooltip or skip until later.

### 2.3 — "Unused only" toggle

- Checkbox/pill filter that filters client-side to `chunkCount === 0`.
- Pairs with bulk-select (2.4) for one-click cleanup.

### 2.4 — Bulk selection mode

- Toggle button "Select" in the header. When active:
  - Each pill grows a checkbox on the left.
  - Sticky bottom action bar appears: `N selected · Delete · Move to type… · Cancel`.
  - "Move to type…" reuses the tag-type submenu from 1.2, sending one `PATCH` per tag (or a batch endpoint if we add `PATCH /api/tags/bulk` later).
- Deselect-all on route change.

### 2.5 — Search matches tag-type names

- Current filter at `tags.tsx:141` only checks `tag.name`. Extend to also match `tag.tagTypeName`. Typing "arch" surfaces the Architecture group.

## Phase 3 — Polish

### 3.1 — `/` focuses the search input

- Mirror the pattern in `document-browser.tsx:496`: global `keydown` listener on the route, skip when target is input/textarea/contentEditable.

### 3.2 — Tag-type icon + curated color palette

- Tag-type create/edit form (currently `tags.tsx:278`) picks up:
  - Color picker: replace native `<input type="color">` with a swatch grid (8–12 curated hex values matching the vocabulary/chunk-type palette).
  - Icon picker: reuse the icon-picker component from the vocabulary/chunk-type editor (lucide names).
- Render the icon next to the name in the sidebar (`tags.tsx:318-322`) and in the group header (`tags.tsx:215-228`).

### 3.3 — Uncategorized group visuals

- Give the "Uncategorized" header (`tags.tsx:224`) a neutral outlined circle (`border border-muted-foreground/50 rounded-full size-3`) so row rhythm matches typed groups.

## Ordering & effort

| Phase | Blocker for | Est. effort |
|-------|-------------|-------------|
| 0.1   | 2.1, 2.3    | 1–2h        |
| 0.2   | 1.2 rename, 1.2 assign-type, 2.4 | 1–2h |
| 0.3   | 1.2 merge   | 2h          |
| 0.4   | 3.2 icons   | 30min       |
| 1.1   | —           | 1h          |
| 1.2   | —           | 3–4h (menu + rename + merge + assign) |
| 2.1   | —           | 30min       |
| 2.2   | —           | 45min (2 sorts only) |
| 2.3   | —           | 30min       |
| 2.4   | —           | 2–3h        |
| 2.5   | —           | 15min       |
| 3.1   | —           | 20min       |
| 3.2   | —           | 1–2h        |
| 3.3   | —           | 10min       |

**Ship order:** 0.1 + 0.2 + 0.3 → 1.1 + 1.2 (single PR, this is the value-payload) → 2.1 + 2.3 + 2.5 (quick wins) → 2.4 → 3.x polish.

## Non-goals

- Tag hierarchies / parent-child tags.
- Tag aliases (different names surfacing same tag). The merge action covers the main cleanup need.
- Per-codebase tag scoping UI — tags already have codebase scope in the DB; surface that if the list becomes too noisy but not in this plan.
- Activity-log entries for tag rename/merge. Tag changes aren't high-stakes enough to warrant audit trails today; revisit if/when auth is reintroduced.
