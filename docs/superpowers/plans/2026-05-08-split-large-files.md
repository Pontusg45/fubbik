# Split Large Files Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Split 5 large files (750-1375 lines each) into focused modules of ~250-300 lines, improving maintainability without changing behavior.

**Architecture:** Pure refactor — extract hooks, components, and service functions into separate files. Each file should have a single clear responsibility and a well-defined interface.

**Tech Stack:** React (hooks, components), Elysia/Effect (backend services), TanStack Router

---

## Task 1: Split `document-browser.tsx` (1375 → 5 files)

**Files:**
- Keep: `apps/web/src/features/documents/document-browser.tsx` (main component, ~300 lines)
- Create: `apps/web/src/features/documents/document-types.ts`
- Create: `apps/web/src/features/documents/document-utils.ts`
- Create: `apps/web/src/features/documents/document-tree.tsx`
- Create: `apps/web/src/features/documents/document-detail.tsx`

### `document-types.ts` (~40 lines)
Type definitions: `DocumentListItem`, `DocumentChunk`, `DocumentDetail`, `SearchResult`, `DocumentBrowserProps`.

### `document-utils.ts` (~120 lines)
Pure functions:
- `folderFromPath`, `filenameFromPath`
- `buildFolderTree` + `FolderNode` type + sorting
- `extractSnippet`, `highlightMatches`
- `getStaleness` (freshness badge logic)
- `estimateReadingTime`

### `document-tree.tsx` (~200 lines)
Sidebar tree components:
- `FolderTreeNode` — recursive folder tree with expand/collapse
- `IndexTree` — main content "All Documents" folder view
- `TagGroupNode` — tag-grouped sidebar with group selection

### `document-detail.tsx` (~300 lines)
Document detail rendering:
- Document header with breadcrumbs, print button, staleness
- Section rendering with edit-in-place and add-section
- Previous/next navigation
- Table of contents sidebar

### `document-browser.tsx` (~300 lines, slimmed)
Main component keeping:
- State, queries, mutations
- `setSelectedId`, `setSelectedGroup`, search handlers
- `renderSidebar` function
- Top-level layout grid (sidebar + content + TOC)
- Group combined view

- [ ] **Step 1:** Create `document-types.ts` with extracted types
- [ ] **Step 2:** Create `document-utils.ts` with extracted utility functions
- [ ] **Step 3:** Create `document-tree.tsx` with sidebar tree components
- [ ] **Step 4:** Create `document-detail.tsx` with detail rendering
- [ ] **Step 5:** Slim `document-browser.tsx` to import from new files
- [ ] **Step 6:** Run `pnpm run check-types`, verify no errors
- [ ] **Step 7:** Commit: `refactor: split document-browser.tsx into focused modules`

---

## Task 2: Split `chunks.index.tsx` (785 → 4 files)

**Files:**
- Keep: `apps/web/src/routes/chunks.index.tsx` (route + layout, ~250 lines)
- Create: `apps/web/src/features/chunks/use-chunks-data.ts`
- Create: `apps/web/src/features/chunks/chunks-toolbar.tsx`
- Create: `apps/web/src/features/chunks/chunks-results.tsx`

### `use-chunks-data.ts` (~200 lines)
Custom hook `useChunksData(search)` returning:
- `chunksQuery` (infinite query with pagination)
- `federatedQuery` (all-codebases search)
- `tagsQuery`
- `processedChunks` (filtered, pinned)
- Inline title editing state + mutation
- Chunk hover prefetch handler
- Keyboard navigation handlers (j/k, number shortcuts)

### `chunks-toolbar.tsx` (~250 lines)
Toolbar components:
- Search bar with debounce
- Filter pills (active filters display)
- Saved filter presets
- Collections dropdown
- View toggle (list/kanban/grid)
- `SubGroupSelect` and `TagTypeGroupSelect` dropdowns

### `chunks-results.tsx` (~250 lines)
Results rendering:
- Kanban view
- Grid/list view with `LazyGroupList`
- Bulk action bar
- Empty/loading states
- Pagination intersection observer

### `chunks.index.tsx` (~250 lines, slimmed)
Route file keeping:
- Route definition with search validation
- State orchestration
- Composing toolbar + results
- Dialog renders (delete confirm, connection dialog)

- [ ] **Step 1:** Create `use-chunks-data.ts` with extracted hook
- [ ] **Step 2:** Create `chunks-toolbar.tsx` with toolbar components
- [ ] **Step 3:** Create `chunks-results.tsx` with results rendering
- [ ] **Step 4:** Slim `chunks.index.tsx` to compose the new files
- [ ] **Step 5:** Run `pnpm run check-types`, verify no errors
- [ ] **Step 6:** Commit: `refactor: split chunks.index.tsx into focused modules`

---

## Task 3: Split `tags.tsx` (769 → 4 files)

**Files:**
- Keep: `apps/web/src/routes/tags.tsx` (route + orchestration, ~250 lines)
- Create: `apps/web/src/features/tags/tag-types.ts`
- Create: `apps/web/src/features/tags/use-tags-data.ts`
- Create: `apps/web/src/features/tags/tag-pill.tsx`

### `tag-types.ts` (~40 lines)
Type definitions: `Tag`, `TagType`, `SortMode` interfaces.

### `use-tags-data.ts` (~250 lines)
Custom hook `useTagsData()` returning:
- `tagsQuery`, `tagTypesQuery`
- All mutations (create, rename, assign type, merge, delete for both tags and types)
- Tag type form helpers (resetTagTypeForm, startEditTagType, handleTagTypeSubmit)
- Tag form helpers (handleCreateTagSubmit, startRename, commitRename)
- Filtering and grouping logic (filteredTags, sorted groups, unusedCount, mergeCandidates)

### `tag-pill.tsx` (~200 lines)
The `TagPill` component with:
- Inline rename
- Type assignment dropdown
- Merge action
- Delete action
- Usage count badge

### `tags.tsx` (~250 lines, slimmed)
Route file keeping:
- Route definition
- State for dialogs (merge target, delete target)
- Toolbar (search, sort, unused filter, create form)
- Tag groups display (composing TagPill)
- Tag types sidebar
- Confirmation dialogs

- [ ] **Step 1:** Create `tag-types.ts` with extracted types
- [ ] **Step 2:** Create `use-tags-data.ts` with extracted hook
- [ ] **Step 3:** Create `tag-pill.tsx` with extracted component
- [ ] **Step 4:** Slim `tags.tsx` to import from new files
- [ ] **Step 5:** Run `pnpm run check-types`, verify no errors
- [ ] **Step 6:** Commit: `refactor: split tags.tsx into focused modules`

---

## Task 4: Split `command-palette.tsx` (757 → 4 files)

**Files:**
- Keep: `apps/web/src/features/command-palette/command-palette.tsx` (render, ~250 lines)
- Create: `apps/web/src/features/command-palette/command-types.ts`
- Create: `apps/web/src/features/command-palette/use-command-search.ts`
- Create: `apps/web/src/features/command-palette/command-items.ts`

### `command-types.ts` (~50 lines)
Type definitions: `CommandGroup`, `CommandItem`, `RecentPage`.
Constants: `PAGE_ITEMS`, `ACTION_ITEMS`.
Export `useRecentPages` hook.

### `use-command-search.ts` (~250 lines)
Custom hook `useCommandSearch(search, open)` returning:
- All search queries (chunks, federated, tags, requirements, plans, codebases, recent)
- Built items list (the big useMemo that assembles filtered/grouped results)
- Search state management

### `command-items.ts` (~100 lines)
Pure functions for building command items:
- `buildChunkItems`, `buildPageItems`, `buildTagItems`
- `buildRequirementItems`, `buildPlanItems`, `buildCodebaseItems`
- `buildActionItems`
- Grouping helper for rendering

### `command-palette.tsx` (~250 lines, slimmed)
Component keeping:
- Open/close state and keyboard shortcut (Cmd+K)
- `handleKeyDown` for arrow/enter/escape
- Render: backdrop, search input, results groups, footer

- [ ] **Step 1:** Create `command-types.ts` with types, constants, and useRecentPages
- [ ] **Step 2:** Create `command-items.ts` with item building functions
- [ ] **Step 3:** Create `use-command-search.ts` with search hook
- [ ] **Step 4:** Slim `command-palette.tsx` to compose the new files
- [ ] **Step 5:** Run `pnpm run check-types`, verify no errors
- [ ] **Step 6:** Commit: `refactor: split command-palette.tsx into focused modules`

---

## Task 5: Split `chunks/service.ts` (789 → 4 files)

**Files:**
- Keep: `packages/api/src/chunks/service.ts` (re-exports + list/detail, ~250 lines)
- Create: `packages/api/src/chunks/chunk-mutations.ts`
- Create: `packages/api/src/chunks/chunk-import.ts`
- Create: `packages/api/src/chunks/chunk-search.ts`

### `chunk-mutations.ts` (~300 lines)
- `resolveDocumentLinkageForNewChunk`
- `createChunk` (with tags, codebases, versions, events)
- `updateChunk` (with versioning, tag/codebase changes, enrichment)
- `deleteChunk`, `deleteMany`, `archiveChunk`, `restoreChunk`, `listArchivedChunks`
- `mergeChunks`

### `chunk-import.ts` (~250 lines)
- `importDocs` (batch import with folder connections)
- `importDocsStream` (SSE streaming import)
- `createFolderConnections` (internal helper)
- `previewImportDocs` (preview with template matching)
- `getExistingHashes`

### `chunk-search.ts` (~100 lines)
- `semanticSearch`
- `getChunkNeighbors`
- `exportChunks`, `importChunks`
- `listUpdatesByTag`, `listUpdateTags`

### `service.ts` (~250 lines, slimmed)
Keeping:
- All imports
- `listChunks` (complex with filtering, pagination, feature deltas)
- `getChunkDetail` (complex with connections, health, features)
- `getChunkHistory`
- Re-exports from the three new files

**Important:** The routes file (`chunks/routes.ts`) imports from `./service` via `import * as chunkService`. The re-exports in `service.ts` must preserve this interface so the routes file doesn't need changes.

- [ ] **Step 1:** Create `chunk-mutations.ts` with extracted functions
- [ ] **Step 2:** Create `chunk-import.ts` with extracted functions
- [ ] **Step 3:** Create `chunk-search.ts` with extracted functions
- [ ] **Step 4:** Slim `service.ts` to keep list/detail + re-export everything
- [ ] **Step 5:** Run `pnpm run check-types` and `pnpm test`, verify no errors
- [ ] **Step 6:** Commit: `refactor: split chunks/service.ts into focused modules`

---

## Execution Order

Tasks are independent — can be done in any order or in parallel. Recommended sequence:
1. Task 5 (backend, simplest — just moving functions)
2. Task 1 (document-browser, biggest impact)
3. Task 4 (command-palette, clean boundaries)
4. Task 3 (tags, moderate complexity)
5. Task 2 (chunks.index, most interconnected)

## Rules

- Pure refactor — zero behavior changes
- Each new file should be importable independently
- Run `pnpm run check-types` after each task
- Keep the same public API (especially for service.ts re-exports)
- Don't change any import paths in files outside the split target
