# Web UI Polish Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Polish remaining web UI rough edges — remaining skeleton loaders, kanban responsiveness, inline list actions, draft indicator, graph onboarding, and form validation.

**Architecture:** All changes in `apps/web/src/`. Isolated component changes with no backend modifications. Each task is independent.

**Tech Stack:** React, TanStack Router, TanStack Query, shadcn-ui (base-ui), Tailwind CSS

**Codebase notes:**
- `SkeletonList` exists at `apps/web/src/components/ui/skeleton-list.tsx`
- `SkeletonCard` exists at `apps/web/src/components/ui/skeleton-card.tsx`
- Loading states use `isLoading` from TanStack Query
- Chunk list items are rendered inline in `chunks.index.tsx` (lines 1082-1155) — no separate component
- Graph help overlay exists at `graph-view.tsx` lines 1360-1392, toggled by `?` key
- Kanban view uses hardcoded `grid-cols-5` at `kanban-view.tsx` line 35
- Autosave hook returns `{ clearDraft }` — no visual indicator of draft state

---

## File Structure

### New files to create:
- `apps/web/src/features/chunks/chunk-row-actions.tsx` — Dropdown menu for inline list actions
- `apps/web/src/features/chunks/draft-indicator.tsx` — Small "Draft saved" badge component
- `apps/web/src/features/graph/graph-welcome.tsx` — First-visit onboarding overlay for graph

### Files to modify:
- `apps/web/src/routes/templates.tsx` — Replace "Loading..." with skeleton
- `apps/web/src/routes/tags.tsx` — Replace "Loading..." with skeleton
- `apps/web/src/routes/codebases.tsx` — Replace "Loading..." with skeleton
- `apps/web/src/routes/vocabulary.tsx` — Replace "Loading..." with skeleton
- `apps/web/src/routes/knowledge-health.tsx` — Replace "Loading..." with skeleton
- `apps/web/src/routes/requirements.tsx` — Replace "Loading..." with skeleton
- `apps/web/src/features/chunks/kanban-view.tsx` — Responsive columns
- `apps/web/src/routes/chunks.index.tsx` — Add row action menu
- `apps/web/src/routes/chunks.new.tsx` — Add draft indicator
- `apps/web/src/routes/chunks.$chunkId_.edit.tsx` — Add draft indicator
- `apps/web/src/features/graph/graph-view.tsx` — Add welcome overlay

---

## Task 1: Remaining Skeleton Loaders

Replace "Loading..." text in 6 pages with `SkeletonList`.

**Files to modify:**
- `apps/web/src/routes/templates.tsx:249-250`
- `apps/web/src/routes/tags.tsx:195-196,309-310`
- `apps/web/src/routes/codebases.tsx:120-121`
- `apps/web/src/routes/vocabulary.tsx:449-450`
- `apps/web/src/routes/knowledge-health.tsx:64-67`
- `apps/web/src/routes/requirements.tsx:231-236`

- [ ] **Step 1: Replace loading state in templates.tsx**

Find:
```tsx
{templatesQuery.isLoading ? (
    <p className="text-muted-foreground text-sm">Loading...</p>
```
Replace with:
```tsx
{templatesQuery.isLoading ? (
    <SkeletonList count={4} />
```
Import `SkeletonList` from `@/components/ui/skeleton-list`.

- [ ] **Step 2: Replace loading states in tags.tsx**

Two loading states: one for tags list (~line 195) and one for tag types (~line 309). Replace both with `<SkeletonList count={5} />` and `<SkeletonList count={3} />` respectively.

- [ ] **Step 3: Replace loading state in codebases.tsx**

Replace `<p>Loading...</p>` with `<SkeletonList count={3} />`.

- [ ] **Step 4: Replace loading state in vocabulary.tsx**

Replace `<p>Loading...</p>` with `<SkeletonList count={6} />`.

- [ ] **Step 5: Replace loading state in knowledge-health.tsx**

Replace `<p>Loading...</p>` with `<SkeletonList count={4} />`.

- [ ] **Step 6: Replace loading state in requirements.tsx**

Replace the `<p>Loading...</p>` inside the existing `<Card><CardPanel>` wrapper with `<SkeletonList count={5} />`. Keep the Card/CardPanel wrapper for visual consistency with the rest of the page.

- [ ] **Step 7: Commit**

```bash
git add apps/web/src/routes/templates.tsx apps/web/src/routes/tags.tsx apps/web/src/routes/codebases.tsx apps/web/src/routes/vocabulary.tsx apps/web/src/routes/knowledge-health.tsx apps/web/src/routes/requirements.tsx
git commit -m "feat(web): add skeleton loaders to all remaining list pages"
```

---

## Task 2: Kanban View Responsiveness

Make the kanban columns responsive and scrollable on small screens.

**Files:**
- Modify: `apps/web/src/features/chunks/kanban-view.tsx:35`

- [ ] **Step 1: Read kanban-view.tsx**

Understand the current `grid-cols-5 gap-3` layout and how columns are rendered.

- [ ] **Step 2: Make columns responsive**

Replace the hardcoded grid (~line 41) with responsive flex + horizontal scroll:
```tsx
// Replace: className="grid grid-cols-5 gap-3"
// With:
className="flex gap-3 overflow-x-auto pb-2"
```

On each column div (~line 45), replace the existing className to add min-width and flex:
```tsx
// Replace: className="bg-muted/30 rounded-lg border p-2"
// With:
className="bg-muted/30 min-w-[200px] flex-1 rounded-lg border p-2"
```

This gives flexible columns on wide screens and horizontal scrolling on narrow ones.

- [ ] **Step 3: Test at different viewport widths**

Verify: at 1200px+ all 5 columns visible. At 768px, columns scroll horizontally. At 375px (mobile), same scroll behavior.

- [ ] **Step 4: Commit**

```bash
git add apps/web/src/features/chunks/kanban-view.tsx
git commit -m "feat(web): make kanban view responsive with horizontal scroll"
```

---

## Task 3: Inline List Actions (Row Action Menu)

Add a "..." dropdown menu on each chunk row in the list view with quick actions.

**Files:**
- Create: `apps/web/src/features/chunks/chunk-row-actions.tsx`
- Modify: `apps/web/src/routes/chunks.index.tsx` (~lines 1082-1155, chunk list item rendering)

- [ ] **Step 1: Create ChunkRowActions component**

```tsx
// apps/web/src/features/chunks/chunk-row-actions.tsx
import { MoreHorizontal, Pencil, Archive, Trash2, Star, ExternalLink } from "lucide-react";
import { Button } from "@/components/ui/button";
import {
    DropdownMenu,
    DropdownMenuTrigger,
    DropdownMenuContent,
    DropdownMenuItem,
    DropdownMenuSeparator,
} from "@/components/ui/dropdown-menu";
import { Link } from "@tanstack/react-router";

// NOTE: The chunk list uses `isPinned`/`togglePin` (not isFavorite/toggleFavorite).
// There is no per-row archiveMutation — only bulk. You'll need to create one or adapt.
// The `setConfirmAction` shape is { title, description, action: () => void }.
// Read chunks.index.tsx FIRST to understand existing state before wiring.
interface ChunkRowActionsProps {
    chunkId: string;
    isPinned: boolean;
    onTogglePin: () => void;
    onDelete: () => void;
}

export function ChunkRowActions({ chunkId, isPinned, onTogglePin, onDelete }: ChunkRowActionsProps) {
    return (
        <DropdownMenu>
            <DropdownMenuTrigger render={<Button variant="ghost" size="sm" className="size-7 p-0" />}>
                <MoreHorizontal className="size-4" />
            </DropdownMenuTrigger>
            <DropdownMenuContent align="end">
                <DropdownMenuItem render={<Link to="/chunks/$chunkId/edit" params={{ chunkId }} />}>
                    <Pencil className="mr-2 size-3.5" /> Edit
                </DropdownMenuItem>
                <DropdownMenuItem onClick={onTogglePin}>
                    <Pin className={`mr-2 size-3.5 ${isPinned ? "fill-current" : ""}`} />
                    {isPinned ? "Unpin" : "Pin"}
                </DropdownMenuItem>
                <DropdownMenuSeparator />
                <DropdownMenuItem onClick={onDelete} className="text-destructive">
                    <Trash2 className="mr-2 size-3.5" /> Delete
                </DropdownMenuItem>
            </DropdownMenuContent>
        </DropdownMenu>
    );
}
```

**Note:** Read the existing `DropdownMenu` component first — the codebase uses base-ui, so the actual API may differ from standard shadcn. Match the pattern used elsewhere (check `dashboard.tsx` export dropdown or `__root.tsx` manage dropdown).

- [ ] **Step 2: Wire into chunk list items**

In `chunks.index.tsx`, inside each chunk's `CardPanel` (~line 1082-1155), add `<ChunkRowActions>` after the date display, before the closing `</CardPanel>`:

```tsx
<ChunkRowActions
    chunkId={chunk.id}
    isPinned={isPinned(chunk.id)}
    onTogglePin={() => togglePin(chunk.id)}
    onDelete={() => setConfirmAction({
        title: `Delete "${chunk.title}"?`,
        description: "This action cannot be undone.",
        action: () => deleteMutation.mutate(chunk.id),
    })}
/>
```

**Note:** `isPinned` and `togglePin` already exist in the file. `setConfirmAction` takes `{ title, description, action }` shape (read the existing ConfirmDialog usage in the file). You may need to create a `deleteMutation` for single-chunk deletion if only bulk delete exists.

- [ ] **Step 3: Add click stopPropagation**

The dropdown must stop click propagation to prevent navigating to chunk detail when clicking the menu:
```tsx
<div onClick={(e) => e.stopPropagation()}>
    <ChunkRowActions ... />
</div>
```

- [ ] **Step 4: Commit**

```bash
git add apps/web/src/features/chunks/chunk-row-actions.tsx apps/web/src/routes/chunks.index.tsx
git commit -m "feat(web): add inline action menu on chunk list rows"
```

---

## Task 4: Draft Saved Indicator

Show a subtle "Draft saved" indicator on chunk create/edit forms when autosave fires.

**Files:**
- Create: `apps/web/src/features/chunks/draft-indicator.tsx`
- Modify: `apps/web/src/features/chunks/use-autosave.ts` — return `lastSaved` timestamp
- Modify: `apps/web/src/routes/chunks.new.tsx` — render indicator
- Modify: `apps/web/src/routes/chunks.$chunkId_.edit.tsx` — render indicator

- [ ] **Step 1: Extend useAutosave to track save time**

In `use-autosave.ts`, add a `lastSaved` state:
```tsx
const [lastSaved, setLastSaved] = useState<Date | null>(null);

useEffect(() => {
    if (!enabled) return;
    const timer = setTimeout(() => {
        localStorage.setItem(key, JSON.stringify(data));
        setLastSaved(new Date());
    }, DEBOUNCE_MS);
    return () => clearTimeout(timer);
}, [key, data, enabled]);

return { clearDraft, lastSaved };
```

- [ ] **Step 2: Create DraftIndicator component**

```tsx
// apps/web/src/features/chunks/draft-indicator.tsx
import { CheckCircle } from "lucide-react";

export function DraftIndicator({ lastSaved }: { lastSaved: Date | null }) {
    if (!lastSaved) return null;
    return (
        <span className="text-muted-foreground flex items-center gap-1 text-xs">
            <CheckCircle className="size-3" />
            Draft saved
        </span>
    );
}
```

- [ ] **Step 3: Add to chunks.new.tsx**

Render `<DraftIndicator lastSaved={lastSaved} />` near the submit button area.

- [ ] **Step 4: Add to chunks.$chunkId_.edit.tsx**

Same placement near the submit button.

- [ ] **Step 5: Commit**

```bash
git add apps/web/src/features/chunks/draft-indicator.tsx apps/web/src/features/chunks/use-autosave.ts apps/web/src/routes/chunks.new.tsx apps/web/src/routes/chunks.\$chunkId_.edit.tsx
git commit -m "feat(web): show draft saved indicator on chunk forms"
```

---

## Task 5: Graph Onboarding Overlay

Show a welcome overlay on first graph visit explaining key interactions.

**Files:**
- Create: `apps/web/src/features/graph/graph-welcome.tsx`
- Modify: `apps/web/src/features/graph/graph-view.tsx`

- [ ] **Step 1: Create GraphWelcome component**

```tsx
// apps/web/src/features/graph/graph-welcome.tsx
import { MousePointer, Move, Link2, Search } from "lucide-react";
import { Button } from "@/components/ui/button";

interface GraphWelcomeProps {
    onDismiss: () => void;
}

export function GraphWelcome({ onDismiss }: GraphWelcomeProps) {
    return (
        <div className="bg-background/80 absolute inset-0 z-30 flex items-center justify-center backdrop-blur-sm">
            <div className="bg-background max-w-md rounded-lg border p-6 shadow-lg">
                <h3 className="mb-1 text-base font-semibold">Knowledge Graph</h3>
                <p className="text-muted-foreground mb-4 text-sm">Navigate your knowledge visually.</p>
                <div className="space-y-3 text-sm">
                    <div className="flex items-center gap-3">
                        <MousePointer className="text-muted-foreground size-4 shrink-0" />
                        <span><strong>Click</strong> a node to see details. <strong>Double-click</strong> to open.</span>
                    </div>
                    <div className="flex items-center gap-3">
                        <Move className="text-muted-foreground size-4 shrink-0" />
                        <span><strong>Drag</strong> nodes to rearrange. Scroll to zoom.</span>
                    </div>
                    <div className="flex items-center gap-3">
                        <Link2 className="text-muted-foreground size-4 shrink-0" />
                        <span><strong>Shift+click</strong> two nodes to find the path between them.</span>
                    </div>
                    <div className="flex items-center gap-3">
                        <Search className="text-muted-foreground size-4 shrink-0" />
                        <span>Press <kbd className="bg-muted rounded border px-1 text-xs">?</kbd> for all shortcuts.</span>
                    </div>
                </div>
                <Button onClick={onDismiss} className="mt-5 w-full" size="sm">
                    Got it
                </Button>
            </div>
        </div>
    );
}
```

- [ ] **Step 2: Wire into graph-view.tsx**

Add state that checks localStorage for first visit. **Important:** TanStack Start does SSR, so guard against `localStorage` not being available on the server:
```tsx
const [showWelcome, setShowWelcome] = useState(false);

useEffect(() => {
    if (typeof window !== "undefined" && !localStorage.getItem("fubbik-graph-welcomed")) {
        setShowWelcome(true);
    }
}, []);

const dismissWelcome = () => {
    setShowWelcome(false);
    localStorage.setItem("fubbik-graph-welcomed", "true");
};
```

Render `<GraphWelcome onDismiss={dismissWelcome} />` when `showWelcome` is true (alongside the existing `showHelp` overlay).

- [ ] **Step 3: Commit**

```bash
git add apps/web/src/features/graph/graph-welcome.tsx apps/web/src/features/graph/graph-view.tsx
git commit -m "feat(web): add first-visit onboarding overlay to graph page"
```

---

## Task 6: Glob Pattern Validation

Validate applies-to patterns on chunk forms and show inline warnings.

**Files:**
- Modify: `apps/web/src/routes/chunks.new.tsx` (~line 454-461, applies-to input)
- Modify: `apps/web/src/routes/chunks.$chunkId_.edit.tsx` (same section)

- [ ] **Step 1: Add validation helper**

Create a simple inline function (no new file needed):
```tsx
function isValidGlob(pattern: string): boolean {
    if (!pattern.trim()) return true; // empty is fine
    try {
        // Basic validation: no unmatched brackets, no empty groups
        const unmatched = (pattern.match(/\[/g) || []).length !== (pattern.match(/\]/g) || []).length;
        const emptyBraces = /\{\s*\}/.test(pattern);
        return !unmatched && !emptyBraces;
    } catch {
        return false;
    }
}
```

- [ ] **Step 2: Show warning on invalid patterns**

After each applies-to pattern input, add:
```tsx
{pattern && !isValidGlob(pattern) && (
    <p className="text-destructive text-xs mt-1">Invalid glob pattern</p>
)}
```

- [ ] **Step 3: Apply same to edit form**

Copy the validation to the edit form's applies-to section.

- [ ] **Step 4: Commit**

```bash
git add apps/web/src/routes/chunks.new.tsx apps/web/src/routes/chunks.\$chunkId_.edit.tsx
git commit -m "feat(web): add glob pattern validation on chunk forms"
```
