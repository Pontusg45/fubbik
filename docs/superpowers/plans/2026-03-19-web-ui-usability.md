# Web UI Usability Improvements Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Improve web UI usability with better feedback, navigation, keyboard shortcuts, and polish — no AI features.

**Architecture:** All changes are in `apps/web/src/`. Most are isolated component changes with no backend modifications.

**Tech Stack:** React, TanStack Router, TanStack Query, shadcn-ui (base-ui based), sonner (toasts), Tailwind CSS, localStorage

**Important codebase notes:**
- `ConfirmDialog` already exists at `apps/web/src/components/confirm-dialog.tsx` using `Dialog`/`DialogPopup` from base-ui (NOT AlertDialog). Uses `onOpenChange` (not `onCancel`), `confirmVariant` (not `variant`).
- `useRecentChunks` already exists at `apps/web/src/features/chunks/use-recent-chunks.ts` — tracks recently viewed chunks by ID in localStorage.
- `chunks.$chunkId.tsx` already uses `ConfirmDialog` for delete — no browser `confirm()` there.
- `chunks.index.tsx` already has URL-driven filters via `validateSearch` and `Route.useSearch()`.
- `skeleton.tsx` already exists in `apps/web/src/components/ui/`.
- The Dialog system uses `DialogPopup` (aliased as `DialogContent`), `DialogBackdrop`, `DialogClose` — NOT `AlertDialogAction`/`AlertDialogCancel`.

---

## File Structure

### New files to create:
- `apps/web/src/components/prompt-dialog.tsx` — Reusable prompt dialog (replaces browser `prompt()`)
- `apps/web/src/components/ui/skeleton-list.tsx` — Skeleton loader for list pages
- `apps/web/src/components/ui/skeleton-card.tsx` — Skeleton loader for stat cards
- `apps/web/src/features/chunks/use-autosave.ts` — Form draft persistence hook
- `apps/web/src/features/chunks/inline-tag-editor.tsx` — Click-to-edit tag bar component
- `apps/web/src/components/ui/undo-toast.tsx` — Toast with undo action button

### Files to modify:
- `apps/web/src/routes/chunks.index.tsx` — Replace `confirm()`/`prompt()`, skeleton loaders, `/` shortcut
- `apps/web/src/routes/codebases.tsx` — Replace `confirm()`
- `apps/web/src/routes/templates.tsx` — Replace `confirm()`
- `apps/web/src/routes/tags.tsx` — Replace `confirm()`
- `apps/web/src/routes/vocabulary.tsx` — Replace `confirm()`
- `apps/web/src/features/graph/graph-view.tsx` — Replace `confirm()`
- `apps/web/src/routes/chunks.$chunkId.tsx` — Undo delete, inline tag editor, skeleton
- `apps/web/src/routes/chunks.new.tsx` — Autosave hook integration
- `apps/web/src/routes/chunks.$chunkId_.edit.tsx` — Autosave hook integration
- `apps/web/src/routes/dashboard.tsx` — Recently viewed section (using existing useRecentChunks), skeleton loaders
- `apps/web/src/routes/__root.tsx` — Breadcrumb integration (import + render)
- `apps/web/src/features/command-palette/command-palette.tsx` — Tag/type search, quick actions
- `apps/web/src/features/nav/keyboard-shortcuts.tsx` — `/` shortcut for search focus

---

## Task 1: Replace All `confirm()` / `prompt()` Calls with Styled Dialogs

The existing `ConfirmDialog` at `apps/web/src/components/confirm-dialog.tsx` is already used in `chunks.$chunkId.tsx`. Reuse it in 6 more files that still use browser `confirm()`. Also create a `PromptDialog` for the one `prompt()` call.

**Files with `confirm()` to replace:**
- `apps/web/src/routes/chunks.index.tsx:347,352` — bulk delete, bulk archive
- `apps/web/src/routes/codebases.tsx:75` — delete codebase
- `apps/web/src/routes/templates.tsx:129` — delete template
- `apps/web/src/routes/tags.tsx:326` — delete tag type
- `apps/web/src/routes/vocabulary.tsx:205` — delete vocabulary entry
- `apps/web/src/features/graph/graph-view.tsx:1666` — delete from graph

**Files with `prompt()` to replace:**
- `apps/web/src/routes/chunks.index.tsx:659` — save filter name

- [ ] **Step 1: Create PromptDialog component**

Create at `apps/web/src/components/prompt-dialog.tsx`, following the same pattern as the existing `ConfirmDialog` (using `Dialog`, `DialogPopup`, `DialogBackdrop`, `DialogClose`):

```tsx
// apps/web/src/components/prompt-dialog.tsx
import { useState } from "react";
import { Button } from "@/components/ui/button";
import {
    Dialog,
    DialogPopup,
    DialogHeader,
    DialogTitle,
    DialogDescription,
    DialogFooter,
    DialogClose,
} from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";

interface PromptDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  onSubmit: (value: string) => void;
  title: string;
  description?: string;
  placeholder?: string;
  defaultValue?: string;
  submitLabel?: string;
}

export function PromptDialog({
  open, onOpenChange, onSubmit, title, description, placeholder, defaultValue = "", submitLabel = "Save",
}: PromptDialogProps) {
  const [value, setValue] = useState(defaultValue);
  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogPopup showCloseButton={false}>
        <DialogHeader>
          <DialogTitle>{title}</DialogTitle>
          {description && <DialogDescription>{description}</DialogDescription>}
        </DialogHeader>
        <Input value={value} onChange={(e) => setValue(e.target.value)} placeholder={placeholder} autoFocus />
        <DialogFooter variant="bare">
          <DialogClose render={<Button variant="outline" />}>Cancel</DialogClose>
          <Button onClick={() => { onSubmit(value); onOpenChange(false); }} disabled={!value.trim()}>{submitLabel}</Button>
        </DialogFooter>
      </DialogPopup>
    </Dialog>
  );
}
```

- [ ] **Step 2: Run tests**

Run: `cd apps/web && pnpm vitest run`
Expected: PASS (no test for PromptDialog yet, but existing tests still pass)

- [ ] **Step 3: Commit PromptDialog**

```bash
git add apps/web/src/components/prompt-dialog.tsx
git commit -m "feat(web): add PromptDialog component for styled text input prompts"
```

- [ ] **Step 4: Replace confirm() in chunks.index.tsx**

Import `ConfirmDialog` from `@/components/confirm-dialog`. Add state:
```tsx
const [confirmAction, setConfirmAction] = useState<{ type: "delete" | "archive" } | null>(null);
```

Replace `if (!confirm(...)) return;` at lines 347 and 352 with `setConfirmAction({ type: "delete" })` / `setConfirmAction({ type: "archive" })`.

Add `<ConfirmDialog>` at bottom of component:
```tsx
<ConfirmDialog
  open={!!confirmAction}
  onOpenChange={(open) => !open && setConfirmAction(null)}
  title={confirmAction?.type === "delete" ? `Delete ${selectedIds.size} chunks permanently?` : `Archive ${selectedIds.size} chunks?`}
  description="This action cannot be undone."
  confirmLabel={confirmAction?.type === "delete" ? "Delete" : "Archive"}
  confirmVariant="destructive"
  onConfirm={() => {
    if (confirmAction?.type === "delete") { /* existing delete mutation */ }
    if (confirmAction?.type === "archive") { /* existing archive mutation */ }
    setConfirmAction(null);
  }}
/>
```

- [ ] **Step 5: Replace prompt() for save filter**

Import `PromptDialog`. Replace the `prompt("Filter name:")` at line 659 with state-driven PromptDialog.

- [ ] **Step 6: Replace confirm() in remaining 5 files**

Apply the same ConfirmDialog pattern to:
- `codebases.tsx:75` — delete codebase
- `templates.tsx:129` — delete template
- `tags.tsx:326` — delete tag type
- `vocabulary.tsx:205` — delete vocabulary entry
- `graph-view.tsx:1666` — delete from graph (bulk)

Each file: import ConfirmDialog, add state, replace `confirm()`, add `<ConfirmDialog>`.

- [ ] **Step 7: Verify all confirm()/prompt() calls are gone**

Run: `grep -rn "confirm(" apps/web/src/routes/ apps/web/src/features/ --include="*.tsx" | grep -v "onConfirm\|confirmAction\|confirmLabel\|confirm-dialog\|ConfirmDialog"`
Expected: No results

Run: `grep -rn "[^.]prompt(" apps/web/src/routes/ --include="*.tsx" | grep -v "PromptDialog\|promptDialog"`
Expected: No results

- [ ] **Step 8: Commit**

```bash
git add apps/web/src/routes/ apps/web/src/features/graph/graph-view.tsx
git commit -m "feat(web): replace all browser confirm/prompt with styled dialogs"
```

---

## Task 2: Undo Toast for Destructive Actions

Add an undo toast with a 5-second window for chunk deletion. Uses archive as the soft-delete mechanism, with restore as undo.

**Prerequisites:** Verify that POST `/api/chunks/:id/archive` and POST `/api/chunks/:id/restore` endpoints both exist.

**Files:**
- Create: `apps/web/src/components/ui/undo-toast.tsx`
- Modify: `apps/web/src/routes/chunks.$chunkId.tsx` — replace delete with archive+undo flow

- [ ] **Step 1: Verify restore endpoint exists**

Run: `grep -rn "restore" packages/api/src/chunks/routes.ts`
If restore endpoint exists, proceed. If not, this task should be deferred until the endpoint is added.

- [ ] **Step 2: Write undo toast utility**

```tsx
// apps/web/src/components/ui/undo-toast.tsx
import { toast } from "sonner";

export function undoableAction({
  action,
  undoAction,
  message,
  duration = 5000,
}: {
  action: () => Promise<void>;
  undoAction: () => Promise<void>;
  message: string;
  duration?: number;
}) {
  action();
  toast(message, {
    duration,
    action: {
      label: "Undo",
      onClick: () => undoAction(),
    },
  });
}
```

- [ ] **Step 3: Wire into chunk detail delete**

In `chunks.$chunkId.tsx`, modify the delete flow:
- Archive the chunk first (soft-delete)
- Show undo toast with restore as the undo action
- Navigate away (to chunks list)
- If undo clicked within 5s, restore the chunk

- [ ] **Step 4: Commit**

```bash
git add apps/web/src/components/ui/undo-toast.tsx apps/web/src/routes/chunks.\$chunkId.tsx
git commit -m "feat(web): add undo toast for chunk deletion"
```

---

## Task 3: Skeleton Loaders

Replace "Loading..." text with skeleton placeholders.

**Files:**
- Create: `apps/web/src/components/ui/skeleton-list.tsx`
- Create: `apps/web/src/components/ui/skeleton-card.tsx`
- Modify: `apps/web/src/routes/chunks.index.tsx`
- Modify: `apps/web/src/routes/chunks.$chunkId.tsx`
- Modify: `apps/web/src/routes/dashboard.tsx`

- [ ] **Step 1: Create SkeletonList component**

```tsx
// apps/web/src/components/ui/skeleton-list.tsx
import { Skeleton } from "@/components/ui/skeleton";

export function SkeletonList({ count = 5 }: { count?: number }) {
  return (
    <div className="space-y-3">
      {Array.from({ length: count }).map((_, i) => (
        <div key={i} className="flex items-center gap-3 p-3 rounded-lg border">
          <Skeleton className="h-5 w-16" />
          <div className="flex-1 space-y-2">
            <Skeleton className="h-4 w-3/4" />
            <Skeleton className="h-3 w-1/2" />
          </div>
        </div>
      ))}
    </div>
  );
}
```

- [ ] **Step 2: Create SkeletonCard component**

```tsx
// apps/web/src/components/ui/skeleton-card.tsx
import { Skeleton } from "@/components/ui/skeleton";

export function SkeletonCard() {
  return (
    <div className="rounded-lg border p-4 space-y-2">
      <Skeleton className="h-3 w-20" />
      <Skeleton className="h-8 w-16" />
    </div>
  );
}
```

- [ ] **Step 3: Replace loading states in chunks.index.tsx**

Find the loading/empty state rendering. Replace `"Loading..."` with `<SkeletonList count={10} />`.

- [ ] **Step 4: Replace loading states in dashboard.tsx**

Replace stat card loading with `<SkeletonCard />` (4 in the stats grid).

- [ ] **Step 5: Replace loading state in chunks.$chunkId.tsx**

Add skeleton for chunk detail: title skeleton, content block skeleton, metadata line skeleton.

- [ ] **Step 6: Commit**

```bash
git add apps/web/src/components/ui/skeleton-list.tsx apps/web/src/components/ui/skeleton-card.tsx apps/web/src/routes/
git commit -m "feat(web): add skeleton loaders for chunk list, detail, and dashboard"
```

---

## Task 4: Form Autosave / Draft Persistence

Persist chunk create/edit form state to localStorage.

**Files:**
- Create: `apps/web/src/features/chunks/use-autosave.ts`
- Test: `apps/web/src/__tests__/use-autosave.test.ts`
- Modify: `apps/web/src/routes/chunks.new.tsx`
- Modify: `apps/web/src/routes/chunks.$chunkId_.edit.tsx`

- [ ] **Step 1: Write the test**

```ts
// apps/web/src/__tests__/use-autosave.test.ts
import { describe, it, expect, beforeEach } from "vitest";

describe("autosave", () => {
  beforeEach(() => localStorage.clear());

  it("saves form state to localStorage", () => {
    const key = "chunk-draft-new";
    const data = { title: "Test", content: "Hello" };
    localStorage.setItem(key, JSON.stringify(data));
    expect(JSON.parse(localStorage.getItem(key)!)).toEqual(data);
  });

  it("clears draft on explicit clear", () => {
    const key = "chunk-draft-new";
    localStorage.setItem(key, JSON.stringify({ title: "Test" }));
    localStorage.removeItem(key);
    expect(localStorage.getItem(key)).toBeNull();
  });
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cd apps/web && pnpm vitest run src/__tests__/use-autosave.test.ts`

- [ ] **Step 3: Implement useAutosave hook**

```ts
// apps/web/src/features/chunks/use-autosave.ts
import { useEffect, useCallback } from "react";

const DEBOUNCE_MS = 1000;

export function useAutosave<T>(key: string, data: T, enabled = true) {
  useEffect(() => {
    if (!enabled) return;
    const timer = setTimeout(() => {
      localStorage.setItem(key, JSON.stringify(data));
    }, DEBOUNCE_MS);
    return () => clearTimeout(timer);
  }, [key, data, enabled]);

  const clearDraft = useCallback(() => {
    localStorage.removeItem(key);
  }, [key]);

  return { clearDraft };
}

export function loadDraft<T>(key: string): T | null {
  try {
    const raw = localStorage.getItem(key);
    return raw ? JSON.parse(raw) : null;
  } catch {
    return null;
  }
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cd apps/web && pnpm vitest run src/__tests__/use-autosave.test.ts`
Expected: PASS

- [ ] **Step 5: Integrate into chunks.new.tsx**

- Import `useAutosave` and `loadDraft`
- On mount, load draft: `const draft = loadDraft<FormState>("chunk-draft-new")`
- If draft exists, show toast: "Restored unsaved draft"
- Initialize form state from draft if present
- Call `useAutosave("chunk-draft-new", formState)`
- On successful submit, call `clearDraft()`

- [ ] **Step 6: Integrate into chunks.$chunkId_.edit.tsx**

Same pattern with key `chunk-draft-edit-${chunkId}`. Clear on successful update.

- [ ] **Step 7: Commit**

```bash
git add apps/web/src/features/chunks/use-autosave.ts apps/web/src/__tests__/use-autosave.test.ts apps/web/src/routes/chunks.new.tsx apps/web/src/routes/chunks.\$chunkId_.edit.tsx
git commit -m "feat(web): persist form drafts to localStorage with autosave"
```

---

## Task 5: Command Palette Enhancements

Add tag search, type filtering, and quick actions.

**Files:**
- Modify: `apps/web/src/features/command-palette/command-palette.tsx`

**Important:** The command palette uses TanStack Router's `navigate()` which requires typed paths. Quick actions with search params must use `{ to: "/chunks/new", search: { type: "note" } }`, NOT embedded query strings like `"/chunks/new?type=note"`. Use the Eden treaty client for API calls (not raw fetch).

- [ ] **Step 1: Add tag search**

When query starts with `#`, search tags via the Eden treaty client and show results as navigable items. Clicking a tag navigates to `/chunks` with the tag filter applied:
```tsx
navigate({ to: "/chunks", search: { tags: tagName } });
```

- [ ] **Step 2: Add quick actions**

Extend `ACTION_ITEMS` (~line 44-47). Each action needs a `to` path and optional `search` object:
```tsx
const ACTION_ITEMS = [
  { label: "New Chunk", to: "/chunks/new", icon: Plus },
  { label: "New Note", to: "/chunks/new", search: { type: "note" }, icon: FileText },
  { label: "New Document", to: "/chunks/new", search: { type: "document" }, icon: File },
  { label: "New Requirement", to: "/requirements/new", icon: ListChecks },
  { label: "View Health", to: "/knowledge-health", icon: HeartPulse },
];
```

Update the `navigate` call for actions to pass `search` when present.

- [ ] **Step 3: Add recent items section**

Use the existing `useRecentChunks` hook from `@/features/chunks/use-recent-chunks` to show recently viewed chunk IDs. Fetch their titles via API and show as a "Recent" group when query is empty.

- [ ] **Step 4: Test manually**

- Cmd+K, type `#api` → tags matching "api" shown
- Empty query → recent items and actions visible
- Click "New Note" → navigates to `/chunks/new` with type pre-set

- [ ] **Step 5: Commit**

```bash
git add apps/web/src/features/command-palette/command-palette.tsx
git commit -m "feat(web): enhance command palette with tag search, types, and quick actions"
```

---

## Task 6: Search Focus Shortcut (`/`)

**Files:**
- Modify: `apps/web/src/routes/chunks.index.tsx` (keyboard handler ~line 215-243)
- Modify: `apps/web/src/features/nav/keyboard-shortcuts.tsx`

- [ ] **Step 1: Add ref to search input and `/` handler**

In `chunks.index.tsx`:
1. Add `const searchInputRef = useRef<HTMLInputElement>(null)` and attach it to the search input element
2. In the keyboard handler (which is a `switch (e.key)` block), add a case for `/`:
```tsx
case "/":
  e.preventDefault();
  searchInputRef.current?.focus();
  break;
```

The existing guard `if (e.target instanceof HTMLInputElement || e.target instanceof HTMLTextAreaElement) return;` will correctly prevent this from firing when already typing in an input.

- [ ] **Step 2: Add to keyboard shortcuts help**

In `keyboard-shortcuts.tsx`, add `/` → "Focus search" to the shortcuts list.

- [ ] **Step 3: Commit**

```bash
git add apps/web/src/routes/chunks.index.tsx apps/web/src/features/nav/keyboard-shortcuts.tsx
git commit -m "feat(web): add / shortcut to focus search on chunk list"
```

---

## Task 7: Recently Viewed on Dashboard

The `useRecentChunks` hook already tracks viewed chunk IDs. Surface them on the dashboard.

**Files:**
- Modify: `apps/web/src/routes/dashboard.tsx`

- [ ] **Step 1: Add recently viewed section**

Import `useRecentChunks` from `@/features/chunks/use-recent-chunks`. Use the `recentIds` array to fetch chunk details (batch or individually) and render a "Recently Viewed" section between Favorites and Recent Chunks on the dashboard.

```tsx
const { recentIds } = useRecentChunks();
// Fetch chunk details for recentIds from the chunks query data or a separate query
```

- [ ] **Step 2: Commit**

```bash
git add apps/web/src/routes/dashboard.tsx
git commit -m "feat(web): add recently viewed section to dashboard"
```

---

## Task 8: Inline Tag Editor on Chunk Detail

**Files:**
- Create: `apps/web/src/features/chunks/inline-tag-editor.tsx`
- Modify: `apps/web/src/routes/chunks.$chunkId.tsx`

- [ ] **Step 1: Build InlineTagEditor component**

```tsx
// apps/web/src/features/chunks/inline-tag-editor.tsx
import { useState } from "react";
import { Badge } from "@/components/ui/badge";
import { Input } from "@/components/ui/input";
import { X, Plus } from "lucide-react";

interface InlineTagEditorProps {
  tags: string[];
  onUpdate: (tags: string[]) => void;
  loading?: boolean;
}

export function InlineTagEditor({ tags, onUpdate, loading }: InlineTagEditorProps) {
  const [editing, setEditing] = useState(false);
  const [input, setInput] = useState("");

  const addTag = () => {
    const tag = input.trim().toLowerCase();
    if (tag && !tags.includes(tag)) {
      onUpdate([...tags, tag]);
    }
    setInput("");
  };

  const removeTag = (tag: string) => {
    onUpdate(tags.filter((t) => t !== tag));
  };

  return (
    <div className="flex flex-wrap items-center gap-1.5">
      {tags.map((tag) => (
        <Badge key={tag} variant="secondary" className="gap-1">
          {tag}
          <button onClick={() => removeTag(tag)} className="hover:text-destructive" disabled={loading}>
            <X className="h-3 w-3" />
          </button>
        </Badge>
      ))}
      {editing ? (
        <Input
          value={input}
          onChange={(e) => setInput(e.target.value)}
          onKeyDown={(e) => { if (e.key === "Enter") { e.preventDefault(); addTag(); } if (e.key === "Escape") setEditing(false); }}
          onBlur={() => { if (input.trim()) addTag(); setEditing(false); }}
          placeholder="Add tag..."
          className="h-6 w-24 text-xs"
          autoFocus
        />
      ) : (
        <button onClick={() => setEditing(true)} className="text-muted-foreground hover:text-foreground">
          <Plus className="h-3.5 w-3.5" />
        </button>
      )}
    </div>
  );
}
```

- [ ] **Step 2: Wire into chunk detail page**

In `chunks.$chunkId.tsx`, replace the static tag display with `<InlineTagEditor>`. The `onUpdate` callback calls the chunk update mutation with the new tags array.

- [ ] **Step 3: Commit**

```bash
git add apps/web/src/features/chunks/inline-tag-editor.tsx apps/web/src/routes/chunks.\$chunkId.tsx
git commit -m "feat(web): inline tag editing on chunk detail page"
```

---

## Task 9: Breadcrumb Integration

**Files:**
- Modify: `apps/web/src/routes/__root.tsx` — import and render `<Breadcrumbs />`
- Modify: `apps/web/src/features/nav/breadcrumbs.tsx` — verify dynamic label resolution

- [ ] **Step 1: Add Breadcrumbs to root layout**

In `__root.tsx`, import `Breadcrumbs` from `@/features/nav/breadcrumbs` and render it below the header nav, above the main content outlet.

- [ ] **Step 2: Verify dynamic labels**

Ensure chunk titles resolve properly for routes like `/chunks/:id/edit` → `Chunks > My Chunk Title > Edit`. Test by navigating to a chunk edit page.

- [ ] **Step 3: Commit**

```bash
git add apps/web/src/routes/__root.tsx apps/web/src/features/nav/breadcrumbs.tsx
git commit -m "feat(web): integrate breadcrumb navigation in root layout"
```

---

## Task 10: Standardized Empty States

**Files:**
- Modify: `apps/web/src/components/ui/empty.tsx` — add `EmptyAction` subcomponent
- Modify: `apps/web/src/routes/tags.tsx`
- Modify: `apps/web/src/routes/codebases.tsx`
- Modify: `apps/web/src/routes/templates.tsx`
- Modify: `apps/web/src/routes/vocabulary.tsx`
- Modify: `apps/web/src/routes/chunks.archived.tsx` (if exists)

- [ ] **Step 1: Add EmptyAction to empty.tsx**

`EmptyAction` does not exist yet. Add it:
```tsx
export function EmptyAction({ children }: { children: React.ReactNode }) {
  return <div className="mt-4">{children}</div>;
}
```

Or alternatively, use the existing `EmptyContent` component for wrapping action buttons.

- [ ] **Step 2: Add empty states to list pages**

For each page, wrap the "no items" case:

| Page | Icon (lucide) | Title | Description | Action |
|------|------|-------|-------------|--------|
| Tags | `Tag` | No tags yet | Tags help categorize and filter chunks. | Create Tag |
| Codebases | `GitBranch` | No codebases | Add a codebase to scope chunks to specific projects. | Add Codebase |
| Templates | `LayoutTemplate` | No custom templates | Templates pre-fill chunk forms for common patterns. | Create Template |
| Vocabulary | `BookOpen` | No vocabulary | Define domain terms to standardize your knowledge base. | Add Term |

```tsx
<Empty>
  <EmptyMedia variant="icon"><Tag className="h-10 w-10" /></EmptyMedia>
  <EmptyTitle>No tags yet</EmptyTitle>
  <EmptyDescription>Tags help categorize and filter chunks.</EmptyDescription>
  <EmptyAction><Button asChild><Link to="/tags">Create Tag</Link></Button></EmptyAction>
</Empty>
```

- [ ] **Step 3: Commit**

```bash
git add apps/web/src/components/ui/empty.tsx apps/web/src/routes/
git commit -m "feat(web): standardize empty states across all list pages"
```

---

## Task 11: Bulk Connect

Allow connecting multiple selected chunks to a target chunk from the list view.

**Files:**
- Modify: `apps/web/src/routes/chunks.index.tsx`

- [ ] **Step 1: Add "Connect to..." action in bulk bar**

In the bulk actions bar (~line 1123-1222), add a button that opens a dialog:
- Dialog has a chunk search input (debounced, queries API)
- User picks a target chunk and a relation type dropdown
- On submit, creates connections from all selected chunks to the target

- [ ] **Step 2: Implement connection creation**

Use the Eden treaty client (not raw fetch) to POST `/api/connections` for each selected chunk:
```tsx
await Promise.all(
  Array.from(selectedIds).map((sourceId) =>
    api.api.connections.post({ sourceId, targetId, relation })
  )
);
```

- [ ] **Step 3: Commit**

```bash
git add apps/web/src/routes/chunks.index.tsx
git commit -m "feat(web): add bulk connect action for selected chunks"
```
