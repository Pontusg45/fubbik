# Import Wizard Design

**Date:** 2026-05-07
**Status:** Approved

## Overview

Replace the flat import page at `/import` with a wizard-style four-step flow for importing markdown documentation. The page keeps a Quick/Wizard toggle — Quick mode preserves the current flat import behavior, Wizard mode adds file tree selection, server-side preview with template matching, per-file configuration, and a pipeline-style import with streaming progress.

## Goals

- Wire up the existing unused `POST /chunks/import-docs/preview` endpoint so users see template suggestions, extracted fields, and parsed metadata before importing
- Show the folder structure as an interactive tree with tri-state checkboxes and `part_of` connection hints
- Let users edit title, tags, type, and template per-file in a split-pane preview step
- Provide real-time per-file import progress via SSE streaming
- Keep the existing quick import as a toggle for users who don't need the full wizard

## Non-Goals

- Changing the underlying `importDocument` / `splitMarkdown` / `createFolderConnections` backend logic
- Adding new template matching rules or field extraction targets
- Drag-and-drop file reordering or custom connection creation during import

## Page Shell

The `/import` route renders a single page with a mode toggle in the header.

**Mode toggle:** A segmented control (Quick | Wizard) in the page header, persisted to localStorage. Quick mode renders the current flat import flow unchanged. Wizard mode renders the four-step wizard.

**Step indicator:** Horizontal numbered steps with connecting lines. Active step is indigo filled, completed steps show a checkmark, future steps are outlined. Steps: 1 Select Files → 2 Preview & Configure → 3 Review → 4 Import.

**Navigation footer:** Sticky bottom bar with Back/Next buttons and "Step N of 4" counter. The Next button label adapts: "Preview →", "Review →", "Start Import". Back is disabled on step 1. Both are hidden during and after import (step 4).

## Step 1: Select Files

**Top bar:** Folder picker button (hidden `<input webkitdirectory>`) showing selected folder name and file count badge. Codebase dropdown on the right (required).

**Summary bar:** Shows selected/deselected file counts with "Select all" and "Deselect all" actions.

**File tree:**
- Recursive tree with indent lines (border-left) and chevron expand/collapse
- Tri-state checkboxes: checking a folder checks all children; unchecking removes all children; partial state when some children are unchecked
- Folders show file count and partial-selection badge ("26 of 38" in orange) when partially checked
- **Index file badges:** README.md / index.md / _index.md get a yellow "index" pill
- **Connection hints:** Non-index files in the same directory as an index file show muted text "→ part_of README" to preview the connections import will create
- Auto-expand for small trees (< 30 files), auto-collapse folders for larger imports
- Max 500 files per import (existing limit)

**Next requires:** At least 1 file selected and a codebase chosen.

## Step 2: Preview & Configure

**On entry:** Calls `POST /chunks/import-docs/preview` with selected files and codebaseId. Shows a loading spinner on the tree panel while waiting for the response.

**Layout:** Split-pane — left panel (320px) is a compact navigable file tree, right panel is the detail editor for the selected file.

### Left Panel (File Tree)

- Filter/search input at the top
- Same folder structure as Step 1 but without checkboxes (selection is done)
- Click a file to show its details in the right panel
- Yellow dot (●) next to index files
- Green "N templates" badge on folders where children have template matches
- Active file highlighted with indigo background

### Right Panel (Detail Editor)

When a file is selected, shows:

- **File path** as breadcrumb (muted, small)
- **Title** — text input, pre-filled from frontmatter or H1 heading
- **Type** — dropdown (note, document, reference, schema, checklist), pre-filled from frontmatter or "document" default
- **Tags** — chip list with add/remove. Tags from frontmatter shown as solid chips. Tags derived from folder path shown with dashed border and legend. New tags can be typed in.
- **Template** — shows match status:
  - **No match:** muted "No template matched" with a dropdown to manually assign one
  - **Match found:** green border card showing template name, confidence score, dropdown to override, and "Extracted Fields" section listing the fields the template pulled out (rationale, alternatives, etc.) as read-only key-value pairs
- **Bulk template hint:** When a file matches a template, an inline prompt offers "Apply [Template] to all N files in [folder]?" with an "Apply to folder" button
- **Content preview** — read-only scrollable view of the markdown content (truncated)

All edits are stored in wizard component state (a `Map<filePath, FileConfig>` where `FileConfig` holds title, type, tags, templateId overrides). Nothing is persisted until the import step.

## Step 3: Review

**Summary stats:** Four cards in a row at the top:
- **New chunks** (green) — files that will create new chunks
- **Will be skipped** (yellow) — files whose content is unchanged since last import
- **Template matches** (indigo) — files with a template applied
- **Connections** (purple) — `part_of` connections that will be created

**Duplicate detection:** Client-side. The frontend SHA-256 hashes each file's content. The preview endpoint response includes existing document hashes for the codebase. Files with matching hashes are flagged as "will be skipped."

**Collapsible detail sections:**

1. **New chunks** (expanded by default) — table with columns: Path, Title, Type, Template, Tags. Template-matched rows have subtle green background. Clicking a row navigates back to Step 2 with that file selected for editing.
2. **Skipped files** (collapsed by default) — list with explanation "(content unchanged since last import)".
3. **Connections** (expanded by default) — list of `file → part_of → index_file` connections that will be created, derived from the same folder-grouping logic as `createFolderConnections`.

**Codebase reminder:** Bottom of the page confirms the target codebase.

**Footer:** Next button shows "Start Import" as the label.

## Step 4: Import

### New Backend Endpoint

`POST /chunks/import-docs/stream` — SSE endpoint that imports files one at a time and streams per-file results.

**Request body:** Same as `/chunks/import-docs`: `{ files: {path, content}[], codebaseId: string, templateOverrides?: Record<string, string | null> }`

**SSE events:**

```
event: file
data: {"path":"getting-started/README.md","status":"created","created":1}

event: file
data: {"path":"getting-started/quick-start.md","status":"unchanged"}

event: file
data: {"path":"features/overlays/deltas.md","status":"error","error":"Frontmatter parse error: ..."}

event: done
data: {"created":71,"skipped":17,"errors":1,"connections":24,"elapsed":4200}
```

**Implementation:** Wraps the existing `importDocument` function per-file inside an Effect.forEach with concurrency 1 (sequential for predictable streaming order). After all files, runs `createFolderConnections` and emits the `done` event with totals. The existing batch endpoint stays unchanged for quick mode.

**SSE via POST:** Since `EventSource` only supports GET, the frontend uses `fetch()` with `ReadableStream` to consume the SSE stream from a POST request. The response uses `Content-Type: text/event-stream` and the frontend parses the `event:` / `data:` lines from the stream chunks. Elysia supports this by returning a generator or async iterable from a route handler.

**Rate limiting:** Same as the batch endpoint — 5 requests per 60 seconds per user.

### Frontend

**Progress bar:** Top of the step, shows `N / total` with smooth animated width.

**Live stats:** Below the progress bar — running counts of created, skipped, error, pending.

**Pipeline table:** All files shown from the start as rows with "pending" status (muted). Columns: Status icon, File path, Title, Chunks created, Detail. As each SSE event arrives, the corresponding row updates:
- ✓ green — created (shows chunk count)
- ○ yellow — skipped/unchanged
- ✕ red — error (clickable to expand inline error message)
- ◌ indigo spinning — currently importing
- ⋯ muted — pending

**Auto-scroll:** Table scrolls to keep the currently-processing row visible.

**No back button** once import starts. Footer navigation is hidden.

**Completion state:** When the `done` event arrives:
- Progress bar fills to 100%
- Success banner replaces the progress section: checkmark, "Import Complete", total time
- Final stats row: created, skipped, errors, connections
- Action buttons: "View imported chunks" (navigates to `/chunks` filtered by codebase), "View in graph" (navigates to `/graph` with codebase), "Import more" (resets wizard to step 1)

## State Management

All wizard state lives in the route component using `useState`:

```typescript
type WizardState = {
  step: 1 | 2 | 3 | 4;
  mode: "quick" | "wizard";
  files: FileEntry[];               // {path, content} from folder picker
  selectedPaths: Set<string>;        // tri-state resolved to flat set
  codebaseId: string | null;
  preview: PreviewFileResult[];      // response from /preview endpoint
  overrides: Map<string, FileConfig>; // per-file edits from step 2
  importStatus: Map<string, ImportFileStatus>; // step 4 live status
};
```

Mode preference is persisted to localStorage. Wizard state resets on "Import more" or page navigation.

## Component Breakdown

| Component | Location | Purpose |
|-----------|----------|---------|
| `ImportPage` | `routes/import.tsx` | Page shell, mode toggle, step routing |
| `ImportQuickMode` | `features/import/quick-mode.tsx` | Current flat import (extracted from existing code) |
| `ImportWizard` | `features/import/wizard.tsx` | Wizard state machine, step navigation |
| `StepSelectFiles` | `features/import/steps/select-files.tsx` | Folder picker, codebase dropdown, file tree |
| `FileTree` | `features/import/file-tree.tsx` | Recursive tree with tri-state checkboxes |
| `StepPreview` | `features/import/steps/preview.tsx` | Split-pane layout, tree nav + detail panel |
| `FileDetailPanel` | `features/import/file-detail-panel.tsx` | Right panel: title, tags, type, template, content preview |
| `StepReview` | `features/import/steps/review.tsx` | Summary stats, collapsible tables |
| `StepImport` | `features/import/steps/import.tsx` | Pipeline table, SSE consumer, completion state |

## Backend Changes

1. **New route:** `POST /chunks/import-docs/stream` in `packages/api/src/chunks/routes.ts` — SSE endpoint wrapping per-file `importDocument` calls with streaming output
2. **Extend preview response:** Add `existingHashes: Record<string, string>` to the preview endpoint response so the frontend can do client-side duplicate detection without an extra round-trip. This maps `sourcePath → contentHash` for documents already imported in the target codebase.

No changes to `importDocument`, `splitMarkdown`, `createFolderConnections`, template matching, or field extraction.

## Edge Cases

- **Empty folder selected:** Step 1 shows empty state with message, Next disabled
- **All files deselected:** Next disabled with tooltip "Select at least one file"
- **Preview endpoint fails:** Show error banner in Step 2 with "Retry" button; don't block — allow proceeding without preview data (fields show defaults from client-side parsing)
- **SSE connection drops mid-import:** Show warning banner "Connection lost — N files may have been imported. Check /chunks for results." with a "View chunks" link
- **0 template matches:** Step 2 works normally, template section just shows "None" for every file
- **Large imports (500 files):** File tree uses virtual scrolling for the visible portion; preview endpoint call may take a few seconds (loading state covers this)
