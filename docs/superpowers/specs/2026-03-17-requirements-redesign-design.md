# Requirements Feature Redesign

## Overview

Comprehensive redesign of the requirements feature covering usability, missing functionality, and integration improvements. The existing BDD-style requirement system (Given/When/Then steps, vocabulary parsing, AI structuring, use case grouping, export) is preserved and enhanced.

## Changes

### 1. List Page — Card + Sidebar Layout

Replace the current flat list with raw HTML selects with a two-panel layout.

**Left Sidebar:**
- Search input for text filtering across requirement titles and descriptions
- Status filter: checkboxes for passing/failing/untested
- Priority filter: checkboxes for must/should/could/wont
- Origin filter: human/AI toggle
- Use case tree: clickable list of use cases with requirement counts, plus "Ungrouped" at bottom. Clicking a use case filters the main area. Active use case highlighted.

**Main Area:**
- Requirement cards showing:
  - Title (clickable, navigates to detail)
  - Status badge (color-coded)
  - Step preview: condensed "Given X · When Y · Then Z" summary
  - Linked chunk count
  - Priority badge
  - Checkbox for bulk selection
- Failing requirements get a subtle red border/tint
- Cards are clickable to navigate to detail

**Bulk Operations:**
- Checkboxes on each card for multi-select
- When items are selected, a floating/sticky action bar appears with:
  - "N selected" count
  - "Set Status" dropdown (passing/failing/untested)
  - "Assign Use Case" dropdown
  - "Delete" button with confirmation
- Bulk operations call a new `PATCH /api/requirements/bulk` endpoint

**Pagination:**
- Bottom pagination with page numbers
- "Showing X-Y of Z" text
- Configurable page size (20 default)

**Stats:**
- Keep the existing stats bar (total/passing/failing/untested) above the main content area

### 2. Detail Page — Inline Edit Mode

Replace the current read-only detail page with a view/edit toggle on the same route.

**View Mode (default):**
- Title, description, status/priority/origin badges (existing, cleaned up)
- Status toggle buttons (existing)
- Review controls for AI requirements (existing)
- Steps display with keyword coloring (existing)
- Linked chunks list (existing)
- Export buttons (existing)
- "Edit" button in the header to switch to edit mode
- Delete button (existing, with ConfirmDialog)

**Edit Mode:**
- Title becomes an input field
- Description becomes a textarea
- Priority becomes a select dropdown
- Use case becomes a select dropdown
- Steps get the full step builder UI:
  - Keyword selector (given/when/then/and/but)
  - Step text input
  - Drag handle for reordering (grip icon)
  - Remove button per step
  - "Add step" button
  - Vocabulary parsing badges (existing, from create page)
  - Inline "Add word" UI for unknown terms (existing, from create page)
- Linked chunks section gets the search-and-select UI (existing, from create page)
- Save/Cancel buttons at the bottom
- Client-side validation (same as create page)
- On save, calls `PATCH /api/requirements/:id` with all changed fields, then `PUT /api/requirements/:id/chunks` if chunks changed

**Implementation approach:** Extract the step builder, chunk linker, and validation logic from the create page into shared components/hooks that both create and detail pages use.

### 3. Coverage Matrix Page

New page at `/requirements/coverage` showing chunk-requirement coverage.

**Layout:**
- Page header with title "Requirement Coverage" and summary stats:
  - Total chunks, covered chunks, coverage percentage (progress bar)
  - Uncovered chunk count
- Filter by codebase (respects active codebase)

**Matrix:**
- Table/grid with:
  - Rows = chunks (showing chunk title)
  - Columns = requirements (showing requirement title, truncated)
  - Cells = checkmark if the requirement is linked to the chunk, empty otherwise
- Column headers are clickable (navigate to requirement detail)
- Row labels are clickable (navigate to chunk detail)
- Color coding: covered rows get a subtle green tint, uncovered rows get a subtle amber tint

**For large datasets:**
- Limit to top N chunks/requirements with "show all" toggle
- Sticky column headers and row labels for scrolling

**Data source:** Uses existing `GET /requirements/coverage` API endpoint which returns covered chunks (with requirementCount), uncovered chunks, and percentage stats. May need enhancement to return the actual requirement-chunk pairs for the matrix cells.

### 4. Backend Changes

**New endpoint — Bulk operations:**
```
PATCH /api/requirements/bulk
Body: {
  ids: string[]
  action: "set_status" | "set_use_case" | "delete"
  status?: "passing" | "failing" | "untested"
  useCaseId?: string | null
}
```
- Validates ownership of all requirements
- Applies the action to all specified requirements
- Returns count of affected rows

**Enhanced list query:**
- Add `search` query parameter to `GET /api/requirements` — searches across title and description using ILIKE
- Add `useCaseId` query parameter to `GET /api/requirements` — filters by use case (needed for sidebar use case navigation)
- Both combine with existing filters (status, priority, origin, reviewStatus)

**Coverage matrix data:**
- Enhance `GET /requirements/coverage` to optionally return the full matrix (requirement-chunk pairs) via a `?detail=true` query parameter
- Returns: `{ covered: [...], uncovered: [...], matrix: [{ chunkId, requirementId }], stats: { total, covered, percentage } }`

**Reverse lookup (already exists partially):**
- The `requirement_chunk` join table already supports reverse lookups
- Add repository method `getRequirementsForChunk(chunkId)` if not already present
- Expose as needed for the coverage matrix

### 5. Shared Components to Extract

From the create page, extract into reusable components:

- **StepBuilder** — keyword selector, text input, add/remove, drag-to-reorder, vocabulary parsing, inline add word
- **ChunkLinker** — search input, filtered dropdown, selected badges with remove
- **RequirementValidator** — step validation logic (Given/When/Then ordering rules)

These will be used by both the create page and the detail page's edit mode.

## Files to Create/Modify

### New files:
- `apps/web/src/features/requirements/step-builder.tsx` — extracted step builder component
- `apps/web/src/features/requirements/chunk-linker.tsx` — extracted chunk linker component
- `apps/web/src/features/requirements/validation.ts` — shared validation logic
- `apps/web/src/features/requirements/requirement-card.tsx` — card component for list page
- `apps/web/src/features/requirements/sidebar-filters.tsx` — sidebar filter component
- `apps/web/src/features/requirements/bulk-actions.tsx` — bulk action bar component
- `apps/web/src/routes/requirements_.coverage.tsx` — coverage matrix page
- `packages/api/src/requirements/bulk.ts` — bulk operations service

### Modified files:
- `apps/web/src/routes/requirements.tsx` — rewrite list page with sidebar + cards layout
- `apps/web/src/routes/requirements_.new.tsx` — refactor to use shared StepBuilder/ChunkLinker
- `apps/web/src/routes/requirements_.$requirementId.tsx` — add inline edit mode
- `packages/api/src/requirements/routes.ts` — add bulk endpoint, add search parameter
- `packages/api/src/requirements/service.ts` — add bulk operations, search logic
- `packages/db/src/repository/requirement.ts` — add bulk update/delete, search, reverse lookup
- `packages/api/src/coverage/routes.ts` — enhance coverage endpoint with `?detail=true`
- `packages/api/src/coverage/service.ts` — add matrix detail logic
- `packages/db/src/repository/coverage.ts` — enhance for matrix detail data

## Out of Scope

- Drag-and-drop requirement ordering within use cases (can be added later)
- Automated status checks (e.g., running tests to determine passing/failing)
- Graph visualization integration for requirements
- Requirements in the CLI (already exists, no changes needed)
