# Chunk Detail Page Declutter

**Date:** 2026-05-06
**Scope:** Simplify the chunk detail page by removing the sibling navigator and relocating decision context, proposals, and feature overlays into the existing More Context drawer.

## Overview

The chunk detail page currently has a 3-column layout with several inline sections that add visual clutter. This redesign removes the left sibling navigator sidebar entirely (no replacement) and moves three inline sections — decision context callout, proposals, and feature overlays — into existing tabs of the More Context drawer. The result is a clean 2-column layout (content + metadata panel) focused on reading.

## Approach

**Fold into existing drawer tabs (Approach A):** No new tabs. Decision context and feature overlays go to the Context tab, proposals go to the Links tab. Keeps the drawer compact at 4 tabs.

## Design

### 1. New page layout

The layout changes from 3-column + inline sections to a clean 2-column:

**Before:**
```
flex gap-8: [SiblingNav 180px] [Content + DecisionContext + Proposals + Overlays] [MetadataPanel 220px]
```

**After:**
```
flex gap-8: [Content] [MetadataPanel 220px]
```

- `ChunkSiblingNavigator` — removed entirely (component deleted). The `prevDoc`/`nextDoc` state, the `currentIndex` computation, and the keyboard handler for `ArrowUp`/`ArrowDown`/`h`/`l`/`k`/`j` chunk traversal are also removed from the route file.
- `ChunkProposalsSection` — removed from inline rendering
- `FeatureOverlaysSection` — removed from inline rendering
- Decision context callout — removed from `ChunkDetailContent`
- Content area gains more horizontal space (no left sidebar consuming 180px + gap)

### 2. Drawer tab reorganization

The drawer keeps its 4 tabs: Links, Context, Comments, History.

**Context tab** gains two new sections:
1. **Decision Context** — rendered at the top of the tab, before the existing "Applies to" section. Shows rationale, alternatives, consequences in the same amber-tinted callout style currently used inline. Only renders if at least one of rationale, alternatives, or consequences is non-null.
2. **Feature Overlays** — rendered after the existing "AI enrichment" section. Shows active deltas with feature color dot, feature name, "active" badge, and modified keys list. Only renders if the deltas array is non-empty.

**Links tab** gains one new section:
- **Proposals** — rendered after the existing "Discover" section. Shows the `ChunkProposalsSection` content. Only renders if the chunk has pending proposals.

**Comments** and **History** tabs — unchanged.

The "More context" floating button badge count stays the same (connections + appliesTo + fileRefs). The relocated sections don't contribute to the badge.

### 3. Implementation changes

**Files modified:**
- `apps/web/src/routes/chunks.$chunkId.tsx` — remove `ChunkSiblingNavigator`, `ChunkProposalsSection`, and `FeatureOverlaysSection` from the flex layout. Pass decision context fields (rationale, alternatives, consequences) and overlay data (deltas, appliedFeatures) as props to the drawer.
- `apps/web/src/features/chunks/detail/more-context-drawer.tsx` — accept new props and pass to the appropriate tabs.
- `apps/web/src/features/chunks/detail/more-context-context-tab.tsx` — add Decision Context section at top (amber callout) and Feature Overlays section at bottom.
- `apps/web/src/features/chunks/detail/more-context-links-tab.tsx` — add Proposals section after Discover.
- `apps/web/src/features/chunks/detail/chunk-detail-content.tsx` — remove the decision context callout rendering (the amber aside with rationale/alternatives/consequences).

**Files deleted:**
- `apps/web/src/features/chunks/detail/chunk-sibling-navigator.tsx`

**No new files.** Pure relocation of existing components.

## Testing

- **Layout verification:** Page renders as 2-column (content + metadata). No left sidebar visible.
- **Decision context in drawer:** Open Context tab, verify rationale/alternatives/consequences appear at top with amber styling. Verify section hidden when all three fields are null.
- **Proposals in drawer:** Open Links tab, verify proposals section appears after Discover. Verify section hidden when no proposals.
- **Feature overlays in drawer:** Open Context tab, verify overlays section appears at bottom. Verify section hidden when no deltas.
- **Keyboard shortcuts:** Verify h/l/prev/next shortcuts are removed (no errors in console).
- **Responsive:** Verify metadata panel still hides on smaller screens. Drawer still works on mobile.
