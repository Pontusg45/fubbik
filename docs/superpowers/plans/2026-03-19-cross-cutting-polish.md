# Cross-Cutting Polish Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Polish cross-cutting concerns — connection creation UX, graph state management, mobile layout fixes, and kanban bulk operations.

**Architecture:** Mostly web UI changes. Graph refactor extracts state into a custom hook. Mobile fixes are Tailwind responsive class additions. Connection UX simplification modifies existing dialog components.

**Tech Stack:** React, TanStack Router, @xyflow/react, Tailwind CSS

**Codebase notes:**
- Graph view at `features/graph/graph-view.tsx` has 20+ useState calls
- Mobile nav at `features/nav/mobile-nav.tsx` uses Sheet component
- Settings page uses fixed widths (`w-64`, `w-80`) that break on mobile
- Kanban view at `features/chunks/kanban-view.tsx` has no bulk operations
- Connection creation requires modal with search + relation type in both graph and chunk detail

---

## File Structure

### New files to create:
- `apps/web/src/features/graph/use-graph-state.ts` — Extracted graph state reducer

### Files to modify:
- `apps/web/src/features/graph/graph-view.tsx` — Use extracted state, simplify connection UX
- `apps/web/src/routes/settings.tsx` — Responsive layout fixes
- `apps/web/src/features/chunks/kanban-view.tsx` — Add multi-select and bulk actions

---

## Task 1: Simplify Connection Creation UX

Add a "quick connect" mode with a default relation type to reduce clicks.

**Files:**
- Modify: `apps/web/src/features/chunks/link-chunk-dialog.tsx` — The actual dialog component (NOT in the route file)

**Note:** `chunks.$chunkId.tsx` only imports and renders `<LinkChunkDialog>`. The dialog implementation is in `features/chunks/link-chunk-dialog.tsx`.

- [ ] **Step 1: Read link-chunk-dialog.tsx**

Find where the dialog is implemented. The current flow: search for chunk + pick relation type + confirm. The relation state already has a default of `"related"` (line 15) — but the valid enum value is `"related_to"`. This is likely a bug.

- [ ] **Step 2: Fix default relation type**

The existing default `"related"` should be `"related_to"` to match the valid relation enum:

```tsx
const [relation, setRelation] = useState("related_to"); // fix from "related" to valid enum value
```

- [ ] **Step 3: Add "quick link" from search results**

When search results appear, allow clicking a result to immediately create the connection with the default relation. Show a small "Change type" link to expand the relation picker only when needed.

- [ ] **Step 4: Commit**

```bash
git add apps/web/src/routes/chunks.\$chunkId.tsx
git commit -m "feat(web): simplify connection creation with default relation type"
```

---

## Task 2: Graph State Management Refactor

Extract the 20+ useState calls from graph-view.tsx into a `useReducer`-based hook.

**Files:**
- Create: `apps/web/src/features/graph/use-graph-state.ts`
- Modify: `apps/web/src/features/graph/graph-view.tsx`

- [ ] **Step 1: Read graph-view.tsx state declarations**

Catalog all useState calls. Group them into logical categories:
- **View state:** layout algorithm, edge bundling, timeline cutoff, panel width
- **Selection state:** selectedChunkId (focused node), multi-selected IDs
- **Interaction state:** pending connection (`{ source, target }` — both fields), explore mode, path start/end, show help
- **Filter state:** filter types, filter relations, search query
- **Drag state:** dragged positions

- [ ] **Step 2: Create useGraphState hook**

```tsx
// apps/web/src/features/graph/use-graph-state.ts
import { useReducer, useCallback } from "react";

interface GraphState {
    // View
    layoutAlgorithm: "force" | "hierarchical" | "radial";
    bundleEdges: boolean;
    timelineCutoff: string;
    panelWidth: number;
    // Selection
    focusedNodeId: string | null;
    multiSelectedIds: Set<string>;
    // Interaction
    // NOTE: pendingConnection has BOTH source and target (set after edge drop)
    pendingConnection: { source: string; target: string } | null;
    exploreMode: boolean;
    exploredNodeIds: Set<string>;
    pathStartId: string | null;
    pathEndId: string | null;
    showHelp: boolean;
    // NOTE: Do NOT include showWelcome here — it's managed by the GraphWelcome component (web-polish plan Task 5)
    // Filter
    filterTypes: Set<string>;
    filterRelations: Set<string>;
    searchQuery: string;
}

type GraphAction =
    | { type: "SET_LAYOUT"; layout: GraphState["layoutAlgorithm"] }
    | { type: "TOGGLE_EDGE_BUNDLING" }
    | { type: "SET_FOCUSED_NODE"; id: string | null }
    | { type: "TOGGLE_MULTI_SELECT"; id: string }
    | { type: "CLEAR_SELECTION" }
    | { type: "SET_PENDING_CONNECTION"; connection: { source: string; target: string } | null }
    | { type: "TOGGLE_EXPLORE_MODE" }
    | { type: "SET_PATH_START"; id: string | null }
    | { type: "SET_PATH_END"; id: string | null }
    | { type: "TOGGLE_HELP" }
    | { type: "TOGGLE_FILTER_TYPE"; chunkType: string }
    | { type: "TOGGLE_FILTER_RELATION"; relation: string }
    | { type: "SET_SEARCH"; query: string }
    // ... add more as needed from reading the actual state

function graphReducer(state: GraphState, action: GraphAction): GraphState {
    switch (action.type) {
        case "SET_FOCUSED_NODE":
            return { ...state, focusedNodeId: action.id };
        case "TOGGLE_MULTI_SELECT": {
            const next = new Set(state.multiSelectedIds);
            next.has(action.id) ? next.delete(action.id) : next.add(action.id);
            return { ...state, multiSelectedIds: next };
        }
        case "CLEAR_SELECTION":
            return { ...state, focusedNodeId: null, multiSelectedIds: new Set() };
        case "TOGGLE_HELP":
            return { ...state, showHelp: !state.showHelp };
        case "DISMISS_WELCOME":
            return { ...state, showWelcome: false };
        // ... implement all cases
        default:
            return state;
    }
}

export function useGraphState(initialOverrides?: Partial<GraphState>) {
    const [state, dispatch] = useReducer(graphReducer, {
        layoutAlgorithm: "force",
        bundleEdges: false,
        timelineCutoff: "",
        panelWidth: 380,
        focusedNodeId: null,
        multiSelectedIds: new Set(),
        pendingConnection: null,
        exploreMode: false,
        exploredNodeIds: new Set(),
        pathStartId: null,
        pathEndId: null,
        showHelp: false,
        filterTypes: new Set(),
        filterRelations: new Set(),
        searchQuery: "",
        ...initialOverrides,
    });

    return { state, dispatch };
}
```

- [ ] **Step 3: Replace useState calls in graph-view.tsx**

Replace all the individual useState calls with the hook:
```tsx
const { state: gs, dispatch } = useGraphState();
```

Update all state reads from `focusedNodeId` to `gs.focusedNodeId`, etc. Update all state setters from `setFocusedNodeId(id)` to `dispatch({ type: "SET_FOCUSED_NODE", id })`.

**Important:** This is a large refactor. Do it incrementally — start with one group (e.g., selection state), verify it works, then continue. Don't try to replace all 20 at once.

- [ ] **Step 4: Commit**

```bash
git add apps/web/src/features/graph/use-graph-state.ts apps/web/src/features/graph/graph-view.tsx
git commit -m "refactor(web): extract graph state into useGraphState reducer"
```

---

## Task 3: Mobile Layout Fixes

Fix pages with hardcoded widths that break on mobile.

**Files:**
- Modify: `apps/web/src/routes/settings.tsx`
- Modify: `apps/web/src/features/graph/graph-view.tsx` (panel width)

- [ ] **Step 1: Fix settings page responsive layout**

Read `settings.tsx`. Find fixed width classes: `w-32`, `w-48`, `w-64`, `w-80`. Replace with responsive alternatives:
- `w-64` → `w-full max-w-64`
- `w-80` → `w-full max-w-80`
- `w-48` → `w-full max-w-48`
- `w-32` on numeric inputs is fine (small inputs don't need full width)
- Fixed tab panels → `w-full` with `max-w-2xl`

- [ ] **Step 2: Fix graph detail panel on mobile**

The graph detail panel uses `panelWidth: 380` fixed. On mobile, this should take full width. Look for the panel rendering and add:
```tsx
className={`${isMobile ? "w-full" : `w-[${panelWidth}px]`}`}
```

The `isMobile` state already exists in graph-view.tsx (check for `useState` with media query or window width check).

- [ ] **Step 3: Commit**

```bash
git add apps/web/src/routes/settings.tsx apps/web/src/features/graph/graph-view.tsx
git commit -m "fix(web): responsive layout for settings page and graph panel"
```

---

## Task 4: Kanban Multi-Select and Bulk Actions

Add multi-select to kanban columns and expose bulk actions.

**Files:**
- Modify: `apps/web/src/features/chunks/kanban-view.tsx`

- [ ] **Step 1: Read kanban-view.tsx**

Understand the current card rendering and drag-drop logic.

- [ ] **Step 2: Add selection state**

```tsx
const [selectedIds, setSelectedIds] = useState<Set<string>>(new Set());

const toggleSelect = (id: string) => {
    setSelectedIds(prev => {
        const next = new Set(prev);
        next.has(id) ? next.delete(id) : next.add(id);
        return next;
    });
};
```

- [ ] **Step 3: Add checkbox to each card**

On each draggable card, add a small checkbox:
```tsx
<div className="flex items-start gap-2">
    <Checkbox
        checked={selectedIds.has(chunk.id)}
        onCheckedChange={() => toggleSelect(chunk.id)}
        onClick={(e: React.MouseEvent) => e.stopPropagation()}
        className="mt-0.5"
    />
    <div>
        <p className="text-sm font-medium truncate">{chunk.title}</p>
        ...
    </div>
</div>
```

- [ ] **Step 4: Add bulk action bar**

When items are selected, show a floating bar (similar to the one in list view):
```tsx
{selectedIds.size > 0 && (
    <div className="fixed bottom-4 left-1/2 -translate-x-1/2 z-20 bg-background border rounded-lg shadow-lg px-4 py-2 flex items-center gap-3">
        <span className="text-sm font-medium">{selectedIds.size} selected</span>
        <Button size="sm" variant="outline" onClick={() => bulkArchive(selectedIds)}>Archive</Button>
        <Button size="sm" variant="destructive" onClick={() => bulkDelete(selectedIds)}>Delete</Button>
        <Button size="sm" variant="ghost" onClick={() => setSelectedIds(new Set())}>Cancel</Button>
    </div>
)}
```

**Important:** The kanban component currently only accepts `{ chunks: Chunk[] }` as props — no mutation callbacks are passed in. You must:
1. Extend the kanban component's props to accept `onBulkDelete` and `onBulkArchive` callbacks
2. Pass these from the parent (`chunks.index.tsx`) using the existing bulk mutations there
3. Read both files to understand the current prop contract before implementing

- [ ] **Step 5: Commit**

```bash
git add apps/web/src/features/chunks/kanban-view.tsx
git commit -m "feat(web): add multi-select and bulk actions to kanban view"
```

---

## Task 5: Connection Quick-Create in Graph

Simplify edge creation in the graph to reduce modal friction.

**Files:**
- Modify: `apps/web/src/features/graph/graph-view.tsx`

- [ ] **Step 1: Read the edge creation flow**

Find the `pendingConnection` state and how the relation picker dialog works when drawing edges between nodes.

- [ ] **Step 2: Add quick-create with default relation**

When a user drags to create an edge, instead of showing a modal asking for relation type:
1. Create the connection immediately with `related_to` as default
2. Show a toast: "Connected A → B (related_to)" with an "Edit" action button
3. Clicking "Edit" opens the relation type picker to change it

```tsx
// On edge drop:
await createConnection({ sourceId, targetId, relation: "related_to" });
toast("Connection created", {
    action: {
        label: "Change type",
        onClick: () => openRelationPicker(connectionId),
    },
});
```

- [ ] **Step 3: Commit**

```bash
git add apps/web/src/features/graph/graph-view.tsx
git commit -m "feat(web): quick-create graph connections with default relation type"
```
