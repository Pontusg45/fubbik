# Chunk Detail Page Redesign

## Problem

The current chunk detail page (`apps/web/src/routes/chunks.$chunkId.tsx`, ~818 lines) has accumulated features organically over time. It has:

- **10+ action buttons** in a single horizontal row (Split, Find path, Export, Reader, Similar, Focus, Entry Point, Edit, Archive, Delete). Unreadable on smaller screens.
- **10+ collapsible sections** stacked vertically (Applies To, File References, AI Enrichment, Comments, Connections, Dependency Tree, Suggested Connections, Related Chunks, Related Suggestions, Version History). Overwhelming; hard to find things.
- **Overlapping relation features** (Suggested, Related, Similar) with unclear differentiation.
- **No clear hero**: prose content has the same visual weight as all the collapsible sections beneath it.
- **Mobile experience poor**: sidebar ToC hidden, metadata cramped, button row scrolls awkwardly.

The page needs to be fully reimagined around a reading-first experience.

## Goal

Redesign `/chunks/$chunkId` into a **reading-first three-pane layout** where content is the hero, navigation and minimal metadata live in slim side panes, and all supporting data (connections, context, comments, history) lives behind a "More context" drawer.

Preserve all existing functionality. Reorganize the UI around priority — common actions stay visible, rare actions hide in menus.

---

## 1. Overall Layout

Three-pane grid at `xl:` breakpoint, responsive degradation at smaller sizes.

```
┌──────────────────────────────────────────────────────────────┐
│  ← Back          ★ Favorite  ⤓ Export  ✎ Edit  ⋯            │
├──────────────────────────────────────────────────────────────┤
│  ▓▓▓▓▓▓▓▓▓▓░░░░░░░░░░░░░  (reading progress bar)            │
├─────────┬────────────────────────────────┬──────────────────┤
│ sibling │                                │ On this page     │
│ chunks  │       Title                    │  - Section A     │
│         │       ── content ──            │  - Section B     │
│ prev /  │                                │                  │
│ next    │                                │ Details          │
│         │                                │  Health · Tags   │
│ (180px) │        (700px max)             │  (220px)         │
└─────────┴────────────────────────────────┴──────────────────┘
                                            [▸ More context]
```

### Breakpoints

| Viewport | Layout |
|----------|--------|
| `xl` (≥1280px) | All three panes visible, left pane with chunk navigator, center content, right pane with ToC + metadata |
| `lg` (1024–1279px) | Left pane collapses to a "📚 Siblings" dropdown in the top bar. Center + right pane remain. |
| `md` (768–1023px) | Right pane also collapses. Center content full width. ToC becomes a button opening a dropdown. Metadata collapses to a horizontal strip below the title. |
| `sm` and below | Single column. Everything stacks. Top bar reduces to Back / Edit / ⋯. ToC and siblings accessible via buttons that open overlays. |

### Container

Outer: `max-w-[1400px] mx-auto`. No sidebars at the page level (existing nav stays).

### Focus mode

When focus mode is active (from the existing `useFocusMode` hook), the left pane, right pane, top bar actions (except Back), and "More context" button all hide via `data-focus-hide="true"`. The content expands to `max-w-3xl mx-auto` and reader settings apply.

---

## 2. Top Bar

Replaces the current 10-button action row.

### Left side

- **Back button** — navigates to `router.history.back()` with fallback to `/chunks`. Uses `ArrowLeft` icon.

### Right side

Four buttons always visible (except on mobile, where Favorite and Export collapse into ⋯):

- **★ Favorite** — ghost button. Filled star when favorited. Uses existing `useFavorites` hook.
- **⤓ Export** — dropdown with two items: "Copy as markdown" and "Download .md". Reuses the markdown-build logic from the compose view.
- **✎ Edit** — outline button. Navigates to `/chunks/$chunkId/edit`.
- **⋯ More menu** — everything else, grouped into sections with separators.

### More menu contents

Ordered by frequency, with separators between groups:

**Navigation / View**
- Focus mode (keyboard: `f`)
- Reader settings (font, line height, width) — uses existing `ReaderSettingsPopover` inline inside the dropdown item
- Find path in graph — navigates to `/graph?from=$chunkId`
- Show similar chunks — navigates to `/search?q=similar-to:"$title"`

**Actions**
- Split chunk — opens existing `SplitChunkDialog`
- Mark as entry point / Unmark — toggles existing `isEntryPoint` flag

**Review** (only shown for AI chunks with `reviewStatus !== "approved"`)
- Approve
- Mark as needs review

**Danger zone** (separator above)
- Archive — triggers archive mutation
- Delete — opens `ConfirmDialog`

### Mobile

On `sm` and below, only Back / Edit / ⋯ are visible. Favorite and Export collapse into the ⋯ menu at the top of the first group.

---

## 3. Left Pane — Chunk Navigator

Slim 180px sidebar showing sibling chunks for orientation.

### Sibling logic

- If the chunk belongs to a codebase, siblings are other chunks in the same codebase, sorted by `updated` (newest first)
- Shows a sliding window of 10 chunks: up to 4 before the current + current + up to 5 after
- If no codebase, the pane shows top 10 chunks sharing the current chunk's top tag
- If neither codebase nor tags exist, the pane is hidden entirely

### Layout

```
┌────────────────────┐
│ IN THIS CODEBASE   │  (uppercase label, text-[9px])
│ ── fubbik          │  (codebase name, clickable)
│                    │
│   Auth middleware  │
│   Session storage  │
│ ● Architecture     │  (current — highlighted)
│   Data flow        │
│   API conventions  │
│   See all (23) →   │
├────────────────────┤
│ [← Prev]  [Next →] │
└────────────────────┘
```

### Behavior

- Current chunk highlighted: `bg-indigo-500/15 text-indigo-400 font-semibold` with `border-l-2 border-indigo-500`
- Each row is a `<Link>` — click navigates
- "See all (N)" link at the bottom of the list when there are more than 10 — opens `/chunks?codebaseId=...`
- Prev/Next buttons navigate to the adjacent siblings in the sliding window
- Keyboard: `h` / `l` (new additions to global vim shortcuts) navigate prev/next

### Data query

New `useQuery` in the detail page:
```typescript
api.api.chunks.get({
    query: { codebaseId, sort: "updated", limit: "50" }
})
```

Filters out the current chunk client-side, computes the sliding window, slices to 10.

---

## 4. Center Pane — Reading Content

The hero. ~700px max width for optimal reading line length.

### Meta row (above title)

Compact one-line meta:
```
📄 document · 5 min read · Updated 3 days ago · ●
```
- Type badge with icon (uses `ChunkTypeIcon`)
- Reading time (uses existing `estimateReadingTime`)
- Last updated (relative time)
- Small health dot (full badge in sidebar)

`text-xs text-muted-foreground` with interpunct separators.

### Title

```tsx
<h1 className="text-3xl font-bold tracking-tight leading-tight mb-2">
    {chunk.title}
</h1>
```

### Summary (if present)

```tsx
{chunk.summary && (
    <p className="text-lg italic text-muted-foreground mb-6">
        {chunk.summary}
    </p>
)}
```

### Staleness banner

If the chunk has staleness flags, amber callout between summary and content:
```tsx
<StalenessBanner chunkId={chunk.id} />
```

### Content body

```tsx
<div className={`prose dark:prose-invert max-w-none ${readerClasses}`}>
    <ChunkLinkRenderer content={chunk.content} currentChunkId={chunk.id} />
</div>
```

Reader settings hook (`useReaderSettings`) provides the classes. `ChunkLinkRenderer` handles markdown + wiki-style auto-linking of chunk titles.

### Decision context callout (inline, if present)

If the chunk has any of `rationale`, `alternatives`, or `consequences`, render a styled callout at the end of the content:

```tsx
<aside className="mt-8 rounded-md border-l-2 border-amber-500/40 bg-amber-500/5 p-4">
    <div className="text-xs font-semibold uppercase tracking-wider text-amber-500 mb-3">
        Decision context
    </div>
    {chunk.rationale && <div>...</div>}
    {chunk.alternatives && <div>...</div>}
    {chunk.consequences && <div>...</div>}
</aside>
```

### What's NOT in the center pane

- No tag editor (moved to right pane)
- No connections, comments, version history, etc. (all moved to drawer)
- No inline "add comment" button (accessible via drawer or keyboard shortcut `c`)

---

## 5. Right Pane — Metadata + ToC

220px sidebar, sticky positioning (`sticky top-8`), scrolls independently when long.

### Section 1 — On this page (ToC)

Uses existing `ChunkToc` component. Generated from markdown headings in the content (`##`, `###`, `####`). Indented by heading level.

**Enhancement:** Active heading highlighted as you scroll, using `IntersectionObserver`. The currently-visible heading gets `text-foreground font-semibold`.

If the content has fewer than 2 headings, this section is hidden entirely (the Details section still shows).

### Section 2 — Details

Compact key-value pairs:

```
Health           87  [good]

Tags
[architecture] [core] [onboarding] [+ add]

Type             document
Created          Mar 12
Updated          3d ago
Size             1.2 KB · 42 lines
Connections      7
Codebase         fubbik
Origin           human
Review           approved
```

**Details items:**
- **Health** — clickable, opens a tooltip showing the 5-dimension breakdown (existing `ChunkHealthBadge` logic)
- **Tags** — inline editable via restyled `InlineTagEditor` (restyled to fit sidebar width)
- **Type** — clickable, filters chunks by type: `/chunks?type=<type>`
- **Codebase** — clickable, navigates to `/codebases/$codebaseId`
- **Connections count** — clickable, opens the More context drawer to the Links tab
- **Created/Updated** — tooltip shows full timestamp on hover
- **Origin** — `human` / `ai` badge
- **Review** — `draft` / `reviewed` / `approved` badge

Styling: `text-xs` labels, small key-value rows with `flex justify-between`, generous vertical spacing.

### Responsive

- `lg` (no right pane space): becomes a compact horizontal strip above the title showing health + tags + key metadata. ToC accessible via a "Contents" button in the top bar that opens a dropdown.
- `md` and below: metadata strip collapses into a single chip row: `● 87 · 5 min read · 7 links`. Tap to expand into a dropdown.

---

## 6. More Context Drawer

The key innovation. A slide-in drawer that holds all the supporting data without cluttering the main view.

### Trigger

Floating pill button, fixed at `bottom-4 right-4`:
```tsx
<button className="...">
    ▸ More context
    <span className="opacity-60">(N signals)</span>
</button>
```
- **N** = total count of connections + file refs + comments + suggestions + deps
- `z-40`, hidden in focus mode (`data-focus-hide="true"`) and print mode (`print:hidden`)
- Keyboard: `m` opens/closes

### Drawer behavior

- Slides in from the right, `w-[480px]` (on mobile, `w-full`)
- Overlays the page with a `bg-background/60 backdrop-blur` backdrop
- `z-50`
- Click backdrop or press `Escape` to close
- Content scrollable within the drawer body
- Animation: `transition-transform duration-200 ease-out`

### Structure

```
┌─────────────────────────────────┐
│ More context              [×]   │  (header with close button)
├─────────────────────────────────┤
│ [Links][Context][Comments][Hist]│  (tab strip with count badges)
├─────────────────────────────────┤
│                                 │
│   (tab content, scrolls)        │
│                                 │
└─────────────────────────────────┘
```

### Four tabs

**1. Links** (badge: total connection count)
- **Outgoing** (→) — list of connections from this chunk
- **Incoming** (←) — list of connections to this chunk
- **Dependency tree** (if applicable) — uses existing `DependencyTree` component
- **Suggested connections** — uses existing `SuggestedConnections` component
- **Related chunks** — uses existing `RelatedChunks` component
- Each section has a small heading with count, compact list of items. Click to navigate to target chunk.

**2. Context** (badge: sum of applies-to + file-refs)
- **Applies to** — glob patterns (existing rendering logic from current page, restyled for drawer)
- **File references** — files with line ranges (existing rendering logic)
- **AI enrichment** — existing `AiSection` component
- **Edit decision context** — form to add/edit rationale, alternatives, consequences. If the chunk has no decision context, this is where users add it.

**3. Comments** (badge: comment count)
- Existing `ChunkComments` component (extracted from the inline definition in the current `chunks.$chunkId.tsx` into its own file `apps/web/src/features/chunks/chunk-comments.tsx`)
- Full CRUD preserved

**4. History** (no badge)
- Existing `VersionHistory` component
- Staleness flag history below it

### State persistence

- Drawer open/closed: local state (not URL)
- Active tab: `sessionStorage` keyed by `chunk-detail-drawer-tab` so users return to the same tab they left

### Mobile

- Drawer becomes full-width (`w-full`)
- Same tab structure
- Floating button visible in the same position

---

## 7. What Changes and What Doesn't

### Changes (UI layer only)

**Rewrite:**
- `apps/web/src/routes/chunks.$chunkId.tsx` — full rewrite of the route component. Becomes a thin composition of the new feature components. Should drop from ~818 lines to ~250.

**New files under `apps/web/src/features/chunks/detail/`:**
- `chunk-detail-top-bar.tsx` — new top bar (Back / Favorite / Export / Edit / ⋯ menu)
- `chunk-sibling-navigator.tsx` — left pane with sibling chunks + prev/next buttons
- `chunk-metadata-panel.tsx` — right pane (ToC + Details)
- `chunk-detail-content.tsx` — center content wrapper (title + meta row + summary + content + decision callout)
- `more-context-drawer.tsx` — drawer shell with tabs
- `more-context-links-tab.tsx` — consolidates outgoing/incoming/dependency/suggested/related
- `more-context-context-tab.tsx` — applies-to, file-refs, AI enrichment, decision context edit

**Extract to own file:**
- `apps/web/src/features/chunks/chunk-comments.tsx` — extracted from the inline `ChunkComments` function in the current file

### Unchanged

All existing feature components get reused inside the new shells:
- `ChunkLinkRenderer` — markdown rendering with wiki links
- `ChunkToc` — heading extraction
- `StalenessBanner`
- `InlineTagEditor` (restyled, same logic)
- `AiSection`
- `DependencyTree`, `SuggestedConnections`, `RelatedChunks`
- `VersionHistory`
- `SplitChunkDialog`
- `ConfirmDialog`
- `ReaderSettingsPopover`, `SimilarButton`
- All hooks: `useFocusMode`, `useReaderSettings`, `useReadingTrail`, `useFavorites`
- All mutations: tag, review, archive, delete, toggleEntryPoint, favorite

### Data layer

No API changes. No schema changes. Same `api.api.chunks({ id }).get()` fetch. Sibling navigator adds one new query that hits the existing `GET /api/chunks` endpoint with filtering.

### Keyboard shortcuts

New additions to the global vim shortcuts:
- `h` / `l` — prev/next sibling chunk
- `m` — toggle More context drawer
- `c` — focus comment input (opens drawer to Comments tab)

Existing shortcuts preserved: `j/k/g/G/e/f/?`.

### Out of scope

- Inline editing of chunk content (still navigate to `/edit`)
- Multi-select operations
- Mobile-specific drawer variant (uses same drawer full-width)
- Drawer state persistence in URL
- Auto-sync of sibling list with real-time updates (single query on page load is enough)

---

## Risks and Mitigations

| Risk | Mitigation |
|------|------------|
| Users miss rare features moved to drawer | Badge on "More context" button shows count — signals there's something to see |
| Drawer content is too dense in "Links" tab | Use section headings with count badges, consistent compact list style |
| Sibling navigator confuses users without codebases | Fallback to tag-sharing chunks, or hide pane entirely |
| Focus mode loses action access | Top bar Back button always visible; escape focus mode via `f` key always works |
| Tab state in sessionStorage gets stale | Reset to "Links" tab if the stored value isn't valid |
| Current file is 818 lines — big rewrite | Break into 7 new component files with clear single responsibilities |

---

## Success Criteria

- Main chunks/$chunkId.tsx file is under 300 lines
- Top bar shows at most 4 visible buttons + ⋯ menu
- Main reading column is clean — title, content, no clutter
- All existing functionality is preserved (verified via checklist)
- Three-pane layout works at `xl`, gracefully degrades at smaller sizes
- Focus mode still works and hides all side panes cleanly
- Drawer opens with `m` key and closes with `Escape`
