# Docs Page Filter & Grouping

**Date:** 2026-05-06
**Scope:** Add graph-style filter/grouping controls to the existing `/docs` document browser page

## Overview

Enhance the existing `/docs` page with a filter bar and grouping system inspired by the graph page. Filters (tags, chunk type) narrow which documents are visible. Group-by mode (folder or tag) controls how the sidebar and main content organize documents. Filter presets are saveable to localStorage.

## Approach

**Client-side filtering (Approach A):** Fetch all documents with tags/type upfront, filter and group entirely in the browser. Same pattern as the graph's `apply-prefilter.ts`. Document lists are small enough to hold in memory.

## Design

### 1. API: Enriched document list

The current `listDocuments` repository function returns document metadata with chunk count aggregates but no tag or type information. Enrich it to include:

- `tags: string[]` — tags from the document's first chunk (tags are shared across a document's chunks)
- `type: string` — type of the first chunk

This requires joining through the first chunk (by `documentId` + minimum `documentOrder`) to get tags and type. The enrichment can happen at the repository level (extending the existing `listDocuments` query) or at the service level (fetching tags separately after the list query). Documents with zero chunks get `tags: []` and `type: "document"` (default).

**Endpoint:** The existing documents list endpoint used by the document browser. No new endpoints.

#### Files

- `packages/db/src/repository/document.ts` — extend `listDocuments` to join chunk tags and type
- `packages/api/src/documents/service.ts` — pass through enriched data if service-level join is needed

### 2. Filter bar layout

Compact filter controls stacked above the folder tree / grouped list in the left sidebar of the document browser.

#### Structure (top to bottom)

1. **Search input** — existing search, stays at top
2. **Group-by toggle** — pill buttons: `📁 Folder` (default) | `🏷️ Tag`
3. **"Filters ▸" toggle** — collapses/expands the filter area
4. **Expanded filter area** (when open):
   - **TAGS** label + toggleable tag pills (OR logic). Active tags highlighted with ✕.
   - **TYPE** label + toggleable type pills (same toggle pattern)
   - **Save preset** link + **Clear all** link
   - Result count: "Showing N of M documents"
5. **Sidebar tree/list** — narrows to show only matching documents

#### Behavior

- Filters default to collapsed (all documents visible)
- Toggling a tag/type pill immediately updates the sidebar and main content
- Group-by toggle switches the sidebar between folder tree and tag groups
- Active filter state persisted in URL search params: `?groupBy=tag&tags=auth,backend&types=document`

#### File

- `apps/web/src/features/documents/document-filter-bar.tsx` — new component for the filter controls

### 3. Tag grouping mode

When group-by is set to "Tag":

**Sidebar:** The folder tree is replaced by a flat list of tag groups, sorted alphabetically. Each tag group is collapsible and shows a document count badge. Documents with no tags appear under an "Untagged" group at the bottom. A document tagged with multiple tags appears under each matching tag group.

**Main content (All Documents view):** Documents grouped under tag headings instead of folder headings. Same document cards/rows, organized by tag. Each group is collapsible.

**Interaction with filters:** If tag filters are active, the tag grouping only shows tags with matching documents. If both tag and type filters are active, both apply (AND between dimensions, OR within a dimension).

**URL state:** Group-by mode and active filters persisted in URL search params so the view is shareable and survives reload.

#### File

- `apps/web/src/features/documents/document-browser.tsx` — modify to support tag grouping mode alongside existing folder grouping

### 4. Filter presets

Stored under localStorage key `"fubbik-docs-filter-presets"`, separate from graph presets.

**Preset structure:**

```typescript
interface DocFilterPreset {
    name: string;
    filters: {
        activeTags: string[];
        activeTypes: string[];
        groupBy: "folder" | "tag";
    };
}
```

**Interactions:**
- "Save preset" link in expanded filter area — inline text input for naming
- Presets appear as a dropdown/list below "Save preset"
- Click to apply, trash icon to delete
- User-local only (localStorage), no backend storage

#### File

- `apps/web/src/features/documents/document-filter-presets.tsx` — new component, same pattern as graph's `filter-presets.tsx`

### 5. Data flow and filtering logic

**Data fetching:** Document browser calls the enriched `listDocuments` endpoint once on mount. Response includes `tags: string[]` and `type: string` per document. Held in React Query cache.

**Client-side filter function:** Pure function, similar to graph's `apply-prefilter.ts`:

```typescript
function filterDocuments(
    documents: EnrichedDocument[],
    filters: { activeTags: string[]; activeTypes: string[] }
): EnrichedDocument[]
```

- `activeTags` non-empty: keep documents with at least one matching tag (OR)
- `activeTypes` non-empty: keep documents whose type matches at least one (OR)
- Both active: AND between dimensions

**Grouping function:** Pure function:

```typescript
function groupDocuments(
    documents: EnrichedDocument[],
    groupBy: "folder" | "tag"
): Map<string, EnrichedDocument[]>
```

- `"folder"`: group by directory from `sourcePath` (existing logic)
- `"tag"`: group by tags, documents appear under each matching tag

Both run in `useMemo` keyed on document list + filter state.

#### Files

- `apps/web/src/features/documents/filter-documents.ts` — pure filter + group functions
- `apps/web/src/features/documents/document-browser.tsx` — integrate `useMemo` with filter/group functions

## Testing

- **Filter function tests:** Pure function, easy to unit test. Verify OR within tags, OR within types, AND between dimensions. Verify empty filters return all documents.
- **Group function tests:** Verify folder grouping matches existing behavior. Verify tag grouping places multi-tagged documents under each tag. Verify "Untagged" group for documents with no tags.
- **URL state:** Verify filter params round-trip through URL search params.
- **Presets:** Verify save/load/delete from localStorage.
- **Browser verification:** Filters narrow sidebar and main content. Group toggle switches between folder tree and tag groups. Presets apply correctly.
