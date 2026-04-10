# Chunk Browsing & Navigation Improvements Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make browsing and navigating chunks easier for humans through preview tooltips, inline wiki-style links, reading trails, focus mode, keyboard shortcuts, card grid view, and more.

**Architecture:** Mostly frontend work. Several small hooks (`useReadingTrail`, `useFocusMode`), a few new components (`ChunkPreviewTooltip`, `ChunkCardGrid`, `FocusMode`, `ChunkLinkRenderer`), keyboard handlers, and minor backend additions (cluster computation, learning path entities — in later phases).

**Tech Stack:** React, TanStack Router, shadcn-ui, Tailwind, lucide-react, existing API + AGE queries

---

## Phases

Organized as 5 phases, each independently shippable:

- **Phase 1: Quick wins (UX polish)** — preview tooltips, inline links, reading trail, focus mode, card grid
- **Phase 2: Keyboard-first navigation** — vim-style bindings, numbered jumps, quick open
- **Phase 3: Browse entry points** — alphabetical index, tag cloud, codebase dashboards, featured chunks
- **Phase 4: Reading experience** — ToC in chunk detail, reading time, scroll progress, chunk type icons, content thumbnails, adjustable reader settings
- **Phase 5: Discovery & clustering** — auto-clusters, similar-to, smart collections, learning paths, "you might have missed this"

Items within a phase can be parallelized if they don't share files. Tasks are numbered sequentially across phases.

---

## File Structure

| File | Phase | Action | Responsibility |
|------|-------|--------|---------------|
| `apps/web/src/features/chunks/chunk-preview-tooltip.tsx` | 1 | Create | Hover preview component |
| `apps/web/src/features/chunks/use-chunk-preview.ts` | 1 | Create | Prefetch + cache for hover previews |
| `apps/web/src/features/chunks/chunk-link-renderer.tsx` | 1 | Create | Auto-link chunk names in content |
| `apps/web/src/hooks/use-reading-trail.ts` | 1 | Create | Session-scoped chunk visit history |
| `apps/web/src/features/nav/reading-trail-sidebar.tsx` | 1 | Create | Sidebar showing recent visits |
| `apps/web/src/features/chunks/focus-mode.tsx` | 1 | Create | Focus mode wrapper for chunk detail |
| `apps/web/src/features/chunks/chunk-card-grid.tsx` | 1 | Create | Masonry card grid view |
| `apps/web/src/hooks/use-focus-mode.ts` | 1 | Create | Focus mode state + persistence |
| `apps/web/src/routes/chunks.$chunkId.tsx` | 1, 4 | Modify | Integrate preview, focus mode, ToC, progress |
| `apps/web/src/routes/chunks.index.tsx` | 1 | Modify | Add card grid view toggle |
| `apps/web/src/routes/__root.tsx` | 1, 2 | Modify | Mount reading trail + keyboard handler |
| `apps/web/src/features/nav/keyboard-shortcuts.tsx` | 2 | Create | Global vim-style keyboard handler |
| `apps/web/src/features/nav/quick-open.tsx` | 2 | Create | Ctrl+O chunk picker |
| `apps/web/src/routes/browse.tsx` | 3 | Create | Alphabetical index + tag cloud entry point |
| `apps/web/src/features/browse/alphabetical-index.tsx` | 3 | Create | A-Z index component |
| `apps/web/src/features/browse/tag-cloud.tsx` | 3 | Create | Weighted tag cloud |
| `apps/web/src/routes/codebases.$codebaseId.tsx` | 3 | Create | Codebase dashboard |
| `apps/web/src/features/chunks/featured-chunk-widget.tsx` | 3 | Create | Chunk of the day widget |
| `apps/web/src/features/chunks/reading-time.ts` | 4 | Create | Word count + time estimator |
| `apps/web/src/features/chunks/chunk-type-icon.tsx` | 4 | Create | Per-type icon component |
| `apps/web/src/features/chunks/content-thumbnail.tsx` | 4 | Create | Text-shape preview SVG |
| `apps/web/src/features/chunks/reader-settings.tsx` | 4 | Create | Font size/theme popover |
| `apps/web/src/hooks/use-reader-settings.ts` | 4 | Create | Reader preferences persistence |
| `packages/api/src/chunks/clusters.ts` | 5 | Create | Embedding-based clustering service |
| `apps/web/src/routes/browse.clusters.tsx` | 5 | Create | Cluster browsing view |
| `apps/web/src/features/chunks/similar-button.tsx` | 5 | Create | "Show similar" action |
| `packages/db/src/schema/learning-path.ts` | 5 | Create | Learning path table |
| `apps/web/src/routes/learn.tsx` | 5 | Create | Learning paths index |
| `apps/web/src/features/dashboard/missed-chunks-widget.tsx` | 5 | Create | "You might have missed this" widget |

---

# PHASE 1: QUICK WINS

Five small-to-medium features that dramatically improve browsing UX with minimal backend changes.

---

### Task 1: Chunk Preview Tooltip

**Files:**
- Create: `apps/web/src/features/chunks/chunk-preview-tooltip.tsx`
- Create: `apps/web/src/features/chunks/use-chunk-preview.ts`

**Context:** Hovering over a chunk link in any list shows a floating preview card with title, summary, content excerpt, tags, and type. Uses the existing tooltip/popover primitives and prefetches chunk detail on hover.

- [ ] **Step 1: Create the preview hook**

```typescript
// apps/web/src/features/chunks/use-chunk-preview.ts
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { api } from "@/utils/api";
import { unwrapEden } from "@/utils/eden";

export function useChunkPreview(chunkId: string, enabled: boolean) {
    return useQuery({
        queryKey: ["chunk-preview", chunkId],
        queryFn: async () => unwrapEden(await api.api.chunks({ id: chunkId }).get()),
        enabled,
        staleTime: 60_000,
    });
}

export function usePrefetchChunkPreview() {
    const queryClient = useQueryClient();
    return (chunkId: string) => {
        queryClient.prefetchQuery({
            queryKey: ["chunk-preview", chunkId],
            queryFn: async () => unwrapEden(await api.api.chunks({ id: chunkId }).get()),
            staleTime: 60_000,
        });
    };
}
```

- [ ] **Step 2: Create the preview tooltip component**

```typescript
// apps/web/src/features/chunks/chunk-preview-tooltip.tsx
import { Tooltip, TooltipProvider, TooltipTrigger, TooltipPopup } from "@/components/ui/tooltip";
import { Badge } from "@/components/ui/badge";
import { useChunkPreview, usePrefetchChunkPreview } from "./use-chunk-preview";
import type { ReactNode } from "react";
import { useState } from "react";

export function ChunkPreviewTooltip({ chunkId, children }: { chunkId: string; children: ReactNode }) {
    const [open, setOpen] = useState(false);
    const prefetch = usePrefetchChunkPreview();
    const { data } = useChunkPreview(chunkId, open);

    return (
        <TooltipProvider>
            <Tooltip open={open} onOpenChange={setOpen}>
                <TooltipTrigger
                    onMouseEnter={() => prefetch(chunkId)}
                    asChild
                >
                    {children as any}
                </TooltipTrigger>
                <TooltipPopup side="right" align="start" className="max-w-[360px] p-0">
                    {data ? (
                        <div className="p-3">
                            <div className="mb-2 flex items-center gap-2">
                                <span className="text-sm font-semibold">{(data as any).title}</span>
                                <Badge variant="secondary" size="sm" className="font-mono text-[9px]">
                                    {(data as any).type}
                                </Badge>
                            </div>
                            {(data as any).summary && (
                                <p className="mb-2 text-xs italic text-muted-foreground">
                                    {(data as any).summary}
                                </p>
                            )}
                            {(data as any).content && (
                                <p className="line-clamp-4 text-xs text-muted-foreground">
                                    {(data as any).content.slice(0, 300)}
                                </p>
                            )}
                            {Array.isArray((data as any).tags) && (data as any).tags.length > 0 && (
                                <div className="mt-2 flex flex-wrap gap-1">
                                    {(data as any).tags.slice(0, 5).map((t: any, i: number) => (
                                        <span
                                            key={`${t.name ?? t}-${i}`}
                                            className="rounded bg-muted px-1.5 py-0.5 text-[9px] text-muted-foreground"
                                        >
                                            {t.name ?? t}
                                        </span>
                                    ))}
                                </div>
                            )}
                        </div>
                    ) : (
                        <div className="p-3 text-xs text-muted-foreground">Loading…</div>
                    )}
                </TooltipPopup>
            </Tooltip>
        </TooltipProvider>
    );
}
```

- [ ] **Step 3: Use it in chunk lists**

Find places in the codebase where chunk titles appear as links (chunks list, search results, dashboard widgets). Wrap each Link with `<ChunkPreviewTooltip chunkId={chunk.id}>...</ChunkPreviewTooltip>`.

Start with `apps/web/src/routes/chunks.index.tsx` — wrap the main list items.

- [ ] **Step 4: Verify and commit**

```bash
pnpm --filter web run check-types 2>&1 | grep -E "preview|tooltip"
git add apps/web/src/features/chunks/chunk-preview-tooltip.tsx apps/web/src/features/chunks/use-chunk-preview.ts apps/web/src/routes/chunks.index.tsx
git commit -m "feat(chunks): add hover preview tooltip for chunk links"
```

---

### Task 2: Inline Wiki-Style Links

**Files:**
- Create: `apps/web/src/features/chunks/chunk-link-renderer.tsx`
- Modify: `apps/web/src/routes/chunks.$chunkId.tsx`

**Context:** Auto-detect chunk titles in chunk content and turn them into clickable links. Uses a map of chunk titles → IDs loaded once per page.

- [ ] **Step 1: Create the link renderer component**

```typescript
// apps/web/src/features/chunks/chunk-link-renderer.tsx
import { useQuery } from "@tanstack/react-query";
import { Link } from "@tanstack/react-router";
import { useMemo } from "react";
import { api } from "@/utils/api";
import { unwrapEden } from "@/utils/eden";
import { MarkdownRenderer } from "@/components/markdown-renderer";

/**
 * Renders chunk content as markdown with auto-linked chunk titles.
 * Matches titles (case-insensitive) and wraps them in links.
 */
export function ChunkLinkRenderer({ content, currentChunkId }: { content: string; currentChunkId?: string }) {
    const { data: allChunks } = useQuery({
        queryKey: ["chunks-title-index"],
        queryFn: async () => unwrapEden(await api.api.chunks.get({ query: { limit: "1000" } as any })),
        staleTime: 300_000, // 5 min
    });

    const titleMap = useMemo(() => {
        if (!allChunks) return new Map<string, string>();
        const map = new Map<string, string>();
        const chunks = (allChunks as any)?.chunks ?? [];
        for (const c of chunks) {
            if (c.id !== currentChunkId && c.title && c.title.length >= 4) {
                map.set(c.title.toLowerCase(), c.id);
            }
        }
        return map;
    }, [allChunks, currentChunkId]);

    const processed = useMemo(() => {
        if (titleMap.size === 0) return content;
        // Build a regex of all titles, sorted by length desc to match longer titles first
        const titles = Array.from(titleMap.keys()).sort((a, b) => b.length - a.length);
        if (titles.length === 0) return content;
        // Escape regex special chars
        const escaped = titles.map(t => t.replace(/[.*+?^${}()|[\]\\]/g, "\\$&"));
        const pattern = new RegExp(`\\b(${escaped.join("|")})\\b`, "gi");
        return content.replace(pattern, (match) => {
            const id = titleMap.get(match.toLowerCase());
            return id ? `[${match}](/chunks/${id})` : match;
        });
    }, [content, titleMap]);

    return <MarkdownRenderer>{processed}</MarkdownRenderer>;
}
```

- [ ] **Step 2: Use it in chunk detail page**

In `apps/web/src/routes/chunks.$chunkId.tsx`, find where chunk content is rendered (likely via `MarkdownRenderer`). Replace with `ChunkLinkRenderer`:

```tsx
<ChunkLinkRenderer content={chunk.content} currentChunkId={chunk.id} />
```

- [ ] **Step 3: Verify and commit**

```bash
pnpm --filter web run check-types 2>&1 | grep -E "link-renderer"
git add apps/web/src/features/chunks/chunk-link-renderer.tsx apps/web/src/routes/chunks.$chunkId.tsx
git commit -m "feat(chunks): auto-link chunk titles in content (wiki-style)"
```

---

### Task 3: Reading Trail Sidebar

**Files:**
- Create: `apps/web/src/hooks/use-reading-trail.ts`
- Create: `apps/web/src/features/nav/reading-trail-sidebar.tsx`
- Modify: `apps/web/src/routes/chunks.$chunkId.tsx` (record visit)
- Modify: `apps/web/src/routes/__root.tsx` (mount sidebar)

**Context:** Session-scoped trail of chunks visited. Small persistent sidebar (collapsible) showing the last 10 visited chunks with quick-jump.

- [ ] **Step 1: Create the hook**

```typescript
// apps/web/src/hooks/use-reading-trail.ts
import { useCallback, useEffect, useState } from "react";

const STORAGE_KEY = "fubbik-reading-trail";
const MAX = 15;

export interface TrailItem {
    id: string;
    title: string;
    type: string;
    visitedAt: string;
}

export function useReadingTrail() {
    const [items, setItems] = useState<TrailItem[]>([]);

    useEffect(() => {
        try {
            const stored = sessionStorage.getItem(STORAGE_KEY);
            if (stored) setItems(JSON.parse(stored));
        } catch {
            // ignore
        }
    }, []);

    const addVisit = useCallback((item: Omit<TrailItem, "visitedAt">) => {
        setItems(prev => {
            const filtered = prev.filter(i => i.id !== item.id);
            const next = [{ ...item, visitedAt: new Date().toISOString() }, ...filtered].slice(0, MAX);
            try {
                sessionStorage.setItem(STORAGE_KEY, JSON.stringify(next));
            } catch {
                // ignore
            }
            return next;
        });
    }, []);

    const clear = useCallback(() => {
        setItems([]);
        try {
            sessionStorage.removeItem(STORAGE_KEY);
        } catch {
            // ignore
        }
    }, []);

    return { items, addVisit, clear };
}
```

- [ ] **Step 2: Create the sidebar component**

```typescript
// apps/web/src/features/nav/reading-trail-sidebar.tsx
import { Link } from "@tanstack/react-router";
import { ChevronRight, Footprints, X } from "lucide-react";
import { useState } from "react";
import { useReadingTrail } from "@/hooks/use-reading-trail";
import { Badge } from "@/components/ui/badge";

export function ReadingTrailSidebar() {
    const [collapsed, setCollapsed] = useState(true);
    const { items, clear } = useReadingTrail();

    if (items.length === 0) return null;

    return (
        <div className="fixed bottom-4 right-4 z-40 print:hidden">
            {collapsed ? (
                <button
                    type="button"
                    onClick={() => setCollapsed(false)}
                    className="flex items-center gap-2 rounded-full border bg-card shadow-lg hover:bg-muted/60 transition-colors px-3 py-2 text-xs"
                    aria-label="Show reading trail"
                >
                    <Footprints className="size-3.5" />
                    <span>Trail ({items.length})</span>
                </button>
            ) : (
                <div className="w-64 rounded-lg border bg-card shadow-xl">
                    <div className="flex items-center justify-between border-b px-3 py-2">
                        <div className="flex items-center gap-1.5 text-xs font-semibold">
                            <Footprints className="size-3.5" />
                            Reading trail
                        </div>
                        <div className="flex items-center gap-1">
                            <button
                                type="button"
                                onClick={clear}
                                className="text-[10px] text-muted-foreground hover:text-foreground"
                            >
                                Clear
                            </button>
                            <button
                                type="button"
                                onClick={() => setCollapsed(true)}
                                className="text-muted-foreground hover:text-foreground"
                                aria-label="Collapse trail"
                            >
                                <X className="size-3" />
                            </button>
                        </div>
                    </div>
                    <div className="max-h-64 overflow-y-auto p-1">
                        {items.map((item, i) => (
                            <Link
                                key={`${item.id}-${i}`}
                                to="/chunks/$chunkId"
                                params={{ chunkId: item.id }}
                                className="flex items-center gap-2 rounded-md px-2 py-1.5 hover:bg-muted transition-colors"
                            >
                                <ChevronRight className="size-3 shrink-0 text-muted-foreground/40" />
                                <span className="truncate text-xs">{item.title}</span>
                                <Badge variant="secondary" size="sm" className="ml-auto shrink-0 font-mono text-[8px]">
                                    {item.type}
                                </Badge>
                            </Link>
                        ))}
                    </div>
                </div>
            )}
        </div>
    );
}
```

- [ ] **Step 3: Record visits in chunk detail**

In `apps/web/src/routes/chunks.$chunkId.tsx`, after the chunk data is loaded, add:

```typescript
import { useReadingTrail } from "@/hooks/use-reading-trail";

// In the component:
const { addVisit } = useReadingTrail();

useEffect(() => {
    if (chunk) {
        addVisit({ id: chunk.id, title: chunk.title, type: chunk.type });
    }
}, [chunk, addVisit]);
```

- [ ] **Step 4: Mount sidebar in root layout**

In `apps/web/src/routes/__root.tsx`, add the sidebar component near where `CommandPalette` is mounted:

```tsx
import { ReadingTrailSidebar } from "@/features/nav/reading-trail-sidebar";

// In the JSX, outside isLanding branch:
<ReadingTrailSidebar />
```

- [ ] **Step 5: Verify and commit**

```bash
pnpm --filter web run check-types 2>&1 | grep -E "reading-trail"
git add apps/web/src/hooks/use-reading-trail.ts apps/web/src/features/nav/reading-trail-sidebar.tsx apps/web/src/routes/chunks.$chunkId.tsx apps/web/src/routes/__root.tsx
git commit -m "feat(nav): add session reading trail sidebar"
```

---

### Task 4: Focus Mode

**Files:**
- Create: `apps/web/src/hooks/use-focus-mode.ts`
- Modify: `apps/web/src/routes/chunks.$chunkId.tsx`

**Context:** A toggle on chunk detail that hides the nav, sidebars, and widens the content column for distraction-free reading. Uses a simple state + CSS toggle.

- [ ] **Step 1: Create the hook**

```typescript
// apps/web/src/hooks/use-focus-mode.ts
import { useCallback, useEffect, useState } from "react";

const STORAGE_KEY = "fubbik-focus-mode";

export function useFocusMode() {
    const [enabled, setEnabled] = useState(false);

    useEffect(() => {
        try {
            setEnabled(localStorage.getItem(STORAGE_KEY) === "true");
        } catch {
            // ignore
        }
    }, []);

    const toggle = useCallback(() => {
        setEnabled(prev => {
            const next = !prev;
            try {
                localStorage.setItem(STORAGE_KEY, next ? "true" : "false");
            } catch {
                // ignore
            }
            return next;
        });
    }, []);

    // Apply class to html element so global styles can respond
    useEffect(() => {
        if (enabled) {
            document.documentElement.classList.add("focus-mode");
        } else {
            document.documentElement.classList.remove("focus-mode");
        }
        return () => document.documentElement.classList.remove("focus-mode");
    }, [enabled]);

    return { enabled, toggle };
}
```

- [ ] **Step 2: Add focus-mode CSS**

In `apps/web/src/styles/globals.css` (or wherever global styles live), add:

```css
html.focus-mode header,
html.focus-mode nav,
html.focus-mode aside,
html.focus-mode [data-focus-hide="true"] {
    display: none !important;
}

html.focus-mode main,
html.focus-mode [data-focus-main="true"] {
    max-width: 48rem !important;
    margin: 0 auto !important;
    font-size: 1.05rem;
    line-height: 1.7;
}
```

- [ ] **Step 3: Add toggle to chunk detail**

In `apps/web/src/routes/chunks.$chunkId.tsx`, import and use the hook:

```typescript
import { Focus } from "lucide-react";
import { useFocusMode } from "@/hooks/use-focus-mode";

// In the component:
const { enabled: focusMode, toggle: toggleFocus } = useFocusMode();
```

Add a button in the chunk detail header:

```tsx
<Button variant="ghost" size="sm" onClick={toggleFocus} className="gap-1.5" title="Focus mode (press f)">
    <Focus className="size-3.5" />
    {focusMode ? "Exit focus" : "Focus"}
</Button>
```

Also add `data-focus-main="true"` to the main content container so the CSS applies.

- [ ] **Step 4: Verify and commit**

```bash
pnpm --filter web run check-types 2>&1 | grep -E "focus-mode"
git add apps/web/src/hooks/use-focus-mode.ts apps/web/src/routes/chunks.$chunkId.tsx apps/web/src/styles/globals.css
git commit -m "feat(chunks): add focus mode for distraction-free reading"
```

---

### Task 5: Card Grid View for Chunks List

**Files:**
- Create: `apps/web/src/features/chunks/chunk-card-grid.tsx`
- Modify: `apps/web/src/routes/chunks.index.tsx`

**Context:** Alternative view for the `/chunks` page showing chunks as a masonry/card grid instead of a list. Each card shows title, summary, type badge, tags, and a content excerpt.

- [ ] **Step 1: Create the grid component**

```typescript
// apps/web/src/features/chunks/chunk-card-grid.tsx
import { Link } from "@tanstack/react-router";
import { Badge } from "@/components/ui/badge";

interface ChunkCardData {
    id: string;
    title: string;
    type: string;
    summary?: string | null;
    content?: string | null;
    tags?: Array<{ name: string } | string>;
}

export function ChunkCardGrid({ chunks }: { chunks: ChunkCardData[] }) {
    return (
        <div className="grid gap-3 sm:grid-cols-2 lg:grid-cols-3 xl:grid-cols-4">
            {chunks.map(chunk => (
                <Link
                    key={chunk.id}
                    to="/chunks/$chunkId"
                    params={{ chunkId: chunk.id }}
                    className="group flex h-full flex-col rounded-lg border bg-card p-4 hover:bg-muted/40 hover:border-foreground/20 transition-all"
                >
                    <div className="mb-2 flex items-start justify-between gap-2">
                        <span className="text-sm font-semibold leading-tight text-foreground group-hover:text-primary transition-colors line-clamp-2">
                            {chunk.title}
                        </span>
                        <Badge variant="secondary" size="sm" className="shrink-0 font-mono text-[9px]">
                            {chunk.type}
                        </Badge>
                    </div>
                    {chunk.summary && (
                        <p className="mb-2 line-clamp-2 text-xs italic text-muted-foreground">
                            {chunk.summary}
                        </p>
                    )}
                    {chunk.content && (
                        <p className="line-clamp-4 text-xs text-muted-foreground/80 flex-1">
                            {chunk.content.slice(0, 200)}
                        </p>
                    )}
                    {chunk.tags && chunk.tags.length > 0 && (
                        <div className="mt-3 flex flex-wrap gap-1">
                            {chunk.tags.slice(0, 3).map((t, i) => {
                                const name = typeof t === "string" ? t : t.name;
                                return (
                                    <span
                                        key={`${name}-${i}`}
                                        className="rounded bg-muted px-1.5 py-0.5 text-[9px] text-muted-foreground"
                                    >
                                        {name}
                                    </span>
                                );
                            })}
                            {chunk.tags.length > 3 && (
                                <span className="text-[9px] text-muted-foreground">
                                    +{chunk.tags.length - 3}
                                </span>
                            )}
                        </div>
                    )}
                </Link>
            ))}
        </div>
    );
}
```

- [ ] **Step 2: Add grid view toggle to chunks page**

In `apps/web/src/routes/chunks.index.tsx`, the page already has a `view` URL param for list/kanban. Add `"grid"` as a third option.

Find the view toggle buttons and add a grid option. Import `LayoutGrid` from lucide-react.

In the render logic, add:
```tsx
{view === "grid" && (
    <ChunkCardGrid chunks={processedChunks} />
)}
```

Alongside the existing list and kanban renderings.

- [ ] **Step 3: Verify and commit**

```bash
pnpm --filter web run check-types 2>&1 | grep -E "card-grid"
git add apps/web/src/features/chunks/chunk-card-grid.tsx apps/web/src/routes/chunks.index.tsx
git commit -m "feat(chunks): add card grid view option"
```

---

# PHASE 2: KEYBOARD-FIRST NAVIGATION

Three tasks that add power-user keyboard shortcuts.

---

### Task 6: Vim-Style Global Keyboard Shortcuts

**Files:**
- Create: `apps/web/src/features/nav/keyboard-shortcuts.tsx`
- Modify: `apps/web/src/routes/__root.tsx`

**Context:** Global keyboard handler providing `j/k` (scroll), `g/G` (top/bottom), `/` (focus search — already done), `e` (edit current chunk), `f` (focus mode), `?` (show shortcuts help).

- [ ] **Step 1: Create the keyboard handler component**

```typescript
// apps/web/src/features/nav/keyboard-shortcuts.tsx
import { useNavigate, useRouterState } from "@tanstack/react-router";
import { useEffect, useState } from "react";

const SCROLL_AMOUNT = 100;

function isTypingInInput(): boolean {
    const tag = document.activeElement?.tagName;
    return tag === "INPUT" || tag === "TEXTAREA" || tag === "SELECT" || !!(document.activeElement as HTMLElement)?.isContentEditable;
}

export function KeyboardShortcuts() {
    const navigate = useNavigate();
    const router = useRouterState();
    const [helpOpen, setHelpOpen] = useState(false);

    useEffect(() => {
        function handleKey(e: KeyboardEvent) {
            if (isTypingInInput()) return;
            if (e.metaKey || e.ctrlKey || e.altKey) return;

            switch (e.key) {
                case "j":
                    e.preventDefault();
                    window.scrollBy({ top: SCROLL_AMOUNT, behavior: "smooth" });
                    break;
                case "k":
                    e.preventDefault();
                    window.scrollBy({ top: -SCROLL_AMOUNT, behavior: "smooth" });
                    break;
                case "g":
                    e.preventDefault();
                    window.scrollTo({ top: 0, behavior: "smooth" });
                    break;
                case "G":
                    e.preventDefault();
                    window.scrollTo({ top: document.documentElement.scrollHeight, behavior: "smooth" });
                    break;
                case "e": {
                    // Edit current chunk if we're on a chunk detail page
                    const match = router.location.pathname.match(/^\/chunks\/([^/]+)$/);
                    if (match) {
                        e.preventDefault();
                        void navigate({ to: "/chunks/$chunkId/edit", params: { chunkId: match[1]! } as any });
                    }
                    break;
                }
                case "f":
                    e.preventDefault();
                    document.documentElement.classList.toggle("focus-mode");
                    try {
                        localStorage.setItem(
                            "fubbik-focus-mode",
                            document.documentElement.classList.contains("focus-mode") ? "true" : "false",
                        );
                    } catch {
                        // ignore
                    }
                    break;
                case "?":
                    e.preventDefault();
                    setHelpOpen(prev => !prev);
                    break;
                case "Escape":
                    if (helpOpen) {
                        e.preventDefault();
                        setHelpOpen(false);
                    }
                    break;
            }
        }

        window.addEventListener("keydown", handleKey);
        return () => window.removeEventListener("keydown", handleKey);
    }, [navigate, router.location.pathname, helpOpen]);

    if (!helpOpen) return null;

    return (
        <div className="fixed inset-0 z-[100] flex items-center justify-center bg-background/80 backdrop-blur-sm" onClick={() => setHelpOpen(false)}>
            <div className="w-96 rounded-lg border bg-card p-6 shadow-xl" onClick={e => e.stopPropagation()}>
                <h2 className="mb-4 text-lg font-semibold">Keyboard shortcuts</h2>
                <dl className="space-y-2 text-sm">
                    <ShortcutRow keys="j / k" description="Scroll down / up" />
                    <ShortcutRow keys="g / G" description="Jump to top / bottom" />
                    <ShortcutRow keys="/" description="Focus search" />
                    <ShortcutRow keys="⌘K / Ctrl+K" description="Command palette" />
                    <ShortcutRow keys="e" description="Edit current chunk" />
                    <ShortcutRow keys="f" description="Toggle focus mode" />
                    <ShortcutRow keys="?" description="Show this help" />
                    <ShortcutRow keys="Esc" description="Close popups" />
                </dl>
                <p className="mt-4 text-xs text-muted-foreground">Press Esc or click outside to close.</p>
            </div>
        </div>
    );
}

function ShortcutRow({ keys, description }: { keys: string; description: string }) {
    return (
        <div className="flex items-center justify-between">
            <dt className="text-muted-foreground">{description}</dt>
            <dd className="font-mono text-xs">
                {keys.split(" / ").map((k, i) => (
                    <span key={k}>
                        {i > 0 && <span className="text-muted-foreground"> / </span>}
                        <kbd className="rounded border bg-muted px-1.5 py-0.5">{k}</kbd>
                    </span>
                ))}
            </dd>
        </div>
    );
}
```

- [ ] **Step 2: Mount in root layout**

In `apps/web/src/routes/__root.tsx`:

```tsx
import { KeyboardShortcuts } from "@/features/nav/keyboard-shortcuts";

// Near CommandPalette mount:
<KeyboardShortcuts />
```

- [ ] **Step 3: Verify and commit**

```bash
pnpm --filter web run check-types 2>&1 | grep -E "keyboard-shortcuts"
git add apps/web/src/features/nav/keyboard-shortcuts.tsx apps/web/src/routes/__root.tsx
git commit -m "feat(nav): add vim-style global keyboard shortcuts and help modal"
```

---

### Task 7: Numbered List Jumps

**Files:**
- Modify: `apps/web/src/routes/chunks.index.tsx`

**Context:** Pressing `1`-`9` in any list view jumps to (or opens) the Nth visible item. Small enhancement to existing keyboard handler.

- [ ] **Step 1: Add a list keyboard handler**

In `apps/web/src/routes/chunks.index.tsx`, add a `useEffect`:

```typescript
useEffect(() => {
    function handleKey(e: KeyboardEvent) {
        const tag = document.activeElement?.tagName;
        if (tag === "INPUT" || tag === "TEXTAREA" || tag === "SELECT") return;
        if (e.metaKey || e.ctrlKey || e.altKey) return;

        const key = e.key;
        if (!/^[1-9]$/.test(key)) return;

        const n = Number(key) - 1;
        if (n >= chunks.length) return;

        e.preventDefault();
        const chunk = chunks[n];
        if (chunk) {
            void navigate({ to: "/chunks/$chunkId", params: { chunkId: chunk.id } });
        }
    }
    window.addEventListener("keydown", handleKey);
    return () => window.removeEventListener("keydown", handleKey);
}, [chunks, navigate]);
```

- [ ] **Step 2: Commit**

```bash
git add apps/web/src/routes/chunks.index.tsx
git commit -m "feat(chunks): add numbered 1-9 jumps for quick navigation"
```

---

### Task 8: Quick Open (Fuzzy File-Picker for Chunks)

**Files:**
- Create: `apps/web/src/features/nav/quick-open.tsx`
- Modify: `apps/web/src/routes/__root.tsx`

**Context:** `Ctrl+O` (or `Cmd+O`) opens a fuzzy picker to jump to any chunk by title. Similar to VS Code's file picker. Could reuse the existing command palette infrastructure if it supports ad-hoc queries.

- [ ] **Step 1: Check if command palette already covers this**

Read `apps/web/src/features/command-palette/command-palette.tsx`. If it already has chunk title search (mentioned in previous sessions), this task may be a no-op — just verify `Ctrl+O` is bound.

If not covered, continue with the next steps.

- [ ] **Step 2: Create the quick-open modal**

```typescript
// apps/web/src/features/nav/quick-open.tsx
import { useQuery } from "@tanstack/react-query";
import { useNavigate } from "@tanstack/react-router";
import { FileText, Search } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { api } from "@/utils/api";
import { unwrapEden } from "@/utils/eden";

export function QuickOpen() {
    const [open, setOpen] = useState(false);
    const [query, setQuery] = useState("");
    const [selectedIdx, setSelectedIdx] = useState(0);
    const inputRef = useRef<HTMLInputElement>(null);
    const navigate = useNavigate();

    // Open/close on Cmd+O / Ctrl+O
    useEffect(() => {
        function handleKey(e: KeyboardEvent) {
            if ((e.metaKey || e.ctrlKey) && e.key === "o") {
                e.preventDefault();
                setOpen(prev => !prev);
            }
            if (open && e.key === "Escape") {
                setOpen(false);
            }
        }
        window.addEventListener("keydown", handleKey);
        return () => window.removeEventListener("keydown", handleKey);
    }, [open]);

    // Focus input when opened
    useEffect(() => {
        if (open) {
            setTimeout(() => inputRef.current?.focus(), 0);
            setQuery("");
            setSelectedIdx(0);
        }
    }, [open]);

    const { data } = useQuery({
        queryKey: ["chunks-title-list"],
        queryFn: async () => unwrapEden(await api.api.chunks.get({ query: { limit: "500" } as any })),
        staleTime: 300_000,
        enabled: open,
    });

    const allChunks = ((data as any)?.chunks ?? []) as Array<{ id: string; title: string; type: string }>;

    // Simple fuzzy filter — matches if all chars in query appear in order in title
    const filtered = allChunks
        .filter(c => {
            if (!query) return true;
            const title = c.title.toLowerCase();
            const q = query.toLowerCase();
            let qi = 0;
            for (let i = 0; i < title.length && qi < q.length; i++) {
                if (title[i] === q[qi]) qi++;
            }
            return qi === q.length;
        })
        .slice(0, 20);

    function handleKeyInInput(e: React.KeyboardEvent) {
        if (e.key === "ArrowDown") {
            e.preventDefault();
            setSelectedIdx(i => Math.min(filtered.length - 1, i + 1));
        } else if (e.key === "ArrowUp") {
            e.preventDefault();
            setSelectedIdx(i => Math.max(0, i - 1));
        } else if (e.key === "Enter") {
            e.preventDefault();
            const chunk = filtered[selectedIdx];
            if (chunk) {
                void navigate({ to: "/chunks/$chunkId", params: { chunkId: chunk.id } });
                setOpen(false);
            }
        }
    }

    if (!open) return null;

    return (
        <div className="fixed inset-0 z-[100] flex items-start justify-center bg-background/80 backdrop-blur-sm pt-24" onClick={() => setOpen(false)}>
            <div className="w-[500px] max-w-[90vw] rounded-lg border bg-card shadow-xl" onClick={e => e.stopPropagation()}>
                <div className="flex items-center gap-2 border-b px-3 py-2">
                    <Search className="size-4 text-muted-foreground" />
                    <input
                        ref={inputRef}
                        type="text"
                        value={query}
                        onChange={e => { setQuery(e.target.value); setSelectedIdx(0); }}
                        onKeyDown={handleKeyInInput}
                        placeholder="Go to chunk..."
                        className="flex-1 bg-transparent text-sm outline-none"
                    />
                    <span className="text-[10px] text-muted-foreground font-mono">Esc to close</span>
                </div>
                <div className="max-h-96 overflow-y-auto py-1">
                    {filtered.length === 0 ? (
                        <div className="px-3 py-4 text-center text-xs text-muted-foreground">No matches</div>
                    ) : (
                        filtered.map((chunk, i) => (
                            <button
                                key={chunk.id}
                                type="button"
                                onClick={() => {
                                    void navigate({ to: "/chunks/$chunkId", params: { chunkId: chunk.id } });
                                    setOpen(false);
                                }}
                                className={`flex w-full items-center gap-2 px-3 py-2 text-left text-sm transition-colors ${i === selectedIdx ? "bg-muted" : "hover:bg-muted/50"}`}
                            >
                                <FileText className="size-3.5 shrink-0 text-muted-foreground" />
                                <span className="truncate flex-1">{chunk.title}</span>
                                <span className="text-[10px] text-muted-foreground font-mono shrink-0">{chunk.type}</span>
                            </button>
                        ))
                    )}
                </div>
            </div>
        </div>
    );
}
```

- [ ] **Step 3: Mount and commit**

```tsx
// In apps/web/src/routes/__root.tsx:
import { QuickOpen } from "@/features/nav/quick-open";
// Near CommandPalette:
<QuickOpen />
```

```bash
git add apps/web/src/features/nav/quick-open.tsx apps/web/src/routes/__root.tsx
git commit -m "feat(nav): add Ctrl+O quick open for fuzzy chunk navigation"
```

---

# PHASE 3: BROWSE ENTRY POINTS

Five tasks that add structured browsing entry points.

---

### Task 9: Alphabetical Index Page

**Files:**
- Create: `apps/web/src/routes/browse.tsx`
- Create: `apps/web/src/features/browse/alphabetical-index.tsx`

**Context:** New `/browse` page with an A-Z index of all chunks. Click a letter to scroll to that group.

- [ ] **Step 1: Create the alphabetical index component**

```typescript
// apps/web/src/features/browse/alphabetical-index.tsx
import { Link } from "@tanstack/react-router";
import { useMemo } from "react";

interface ChunkRef {
    id: string;
    title: string;
    type: string;
}

const LETTERS = "ABCDEFGHIJKLMNOPQRSTUVWXYZ".split("");

export function AlphabeticalIndex({ chunks }: { chunks: ChunkRef[] }) {
    const groups = useMemo(() => {
        const map = new Map<string, ChunkRef[]>();
        for (const c of chunks) {
            const letter = (c.title.trim()[0] || "#").toUpperCase();
            const key = /[A-Z]/.test(letter) ? letter : "#";
            const existing = map.get(key) ?? [];
            existing.push(c);
            map.set(key, existing);
        }
        for (const letter of LETTERS) {
            if (!map.has(letter)) map.set(letter, []);
        }
        if (!map.has("#")) map.set("#", []);
        const sorted = Array.from(map.entries()).sort(([a], [b]) => {
            if (a === "#") return 1;
            if (b === "#") return -1;
            return a.localeCompare(b);
        });
        for (const [, items] of sorted) {
            items.sort((a, b) => a.title.localeCompare(b.title));
        }
        return sorted;
    }, [chunks]);

    return (
        <div className="space-y-8">
            {/* Jump bar */}
            <div className="sticky top-0 z-10 flex flex-wrap gap-1 bg-background/95 backdrop-blur py-3 border-b">
                {groups.map(([letter, items]) => (
                    <a
                        key={letter}
                        href={`#letter-${letter}`}
                        className={`rounded px-2 py-1 text-xs font-mono transition-colors ${
                            items.length > 0
                                ? "hover:bg-muted text-foreground"
                                : "text-muted-foreground/30 pointer-events-none"
                        }`}
                    >
                        {letter}
                    </a>
                ))}
            </div>

            {/* Groups */}
            {groups.map(([letter, items]) => (
                items.length > 0 && (
                    <section key={letter} id={`letter-${letter}`}>
                        <h2 className="mb-3 border-b pb-1 text-lg font-bold">{letter}</h2>
                        <div className="grid gap-1 sm:grid-cols-2 lg:grid-cols-3">
                            {items.map(item => (
                                <Link
                                    key={item.id}
                                    to="/chunks/$chunkId"
                                    params={{ chunkId: item.id }}
                                    className="hover:bg-muted/50 rounded px-2 py-1 text-sm transition-colors"
                                >
                                    {item.title}
                                    <span className="ml-2 text-[9px] text-muted-foreground font-mono">
                                        {item.type}
                                    </span>
                                </Link>
                            ))}
                        </div>
                    </section>
                )
            ))}
        </div>
    );
}
```

- [ ] **Step 2: Create the browse route**

```typescript
// apps/web/src/routes/browse.tsx
import { useQuery } from "@tanstack/react-query";
import { createFileRoute } from "@tanstack/react-router";
import { AlphabeticalIndex } from "@/features/browse/alphabetical-index";
import { api } from "@/utils/api";
import { unwrapEden } from "@/utils/eden";

export const Route = createFileRoute("/browse")({
    component: BrowsePage,
});

function BrowsePage() {
    const { data } = useQuery({
        queryKey: ["browse-chunks"],
        queryFn: async () => unwrapEden(await api.api.chunks.get({ query: { limit: "1000" } as any })),
    });

    const chunks = ((data as any)?.chunks ?? []) as Array<{ id: string; title: string; type: string }>;

    return (
        <div className="container mx-auto max-w-6xl px-4 py-8">
            <div className="mb-6">
                <h1 className="text-2xl font-bold tracking-tight">Browse</h1>
                <p className="text-muted-foreground text-sm">All chunks in alphabetical order</p>
            </div>
            <AlphabeticalIndex chunks={chunks} />
        </div>
    );
}
```

- [ ] **Step 3: Verify and commit**

```bash
pnpm --filter web run check-types 2>&1 | grep -E "browse|alphabetical"
git add apps/web/src/routes/browse.tsx apps/web/src/features/browse/alphabetical-index.tsx
git commit -m "feat(browse): add alphabetical A-Z index page"
```

---

### Task 10: Tag Cloud

**Files:**
- Create: `apps/web/src/features/browse/tag-cloud.tsx`
- Modify: `apps/web/src/routes/browse.tsx`

**Context:** Weighted tag cloud where tag font size reflects usage count. Click a tag to filter chunks by it.

- [ ] **Step 1: Create the tag cloud component**

```typescript
// apps/web/src/features/browse/tag-cloud.tsx
import { Link } from "@tanstack/react-router";
import { useMemo } from "react";

interface TagWithCount {
    name: string;
    count: number;
}

export function TagCloud({ tags }: { tags: TagWithCount[] }) {
    const normalized = useMemo(() => {
        if (tags.length === 0) return [];
        const counts = tags.map(t => t.count);
        const min = Math.min(...counts);
        const max = Math.max(...counts);
        const range = max - min || 1;
        return tags.map(t => ({
            ...t,
            weight: (t.count - min) / range,
        }));
    }, [tags]);

    function fontSize(weight: number): string {
        // 0.75rem (xs) to 1.5rem (2xl)
        const size = 0.75 + weight * 0.75;
        return `${size}rem`;
    }

    function opacity(weight: number): number {
        return 0.5 + weight * 0.5;
    }

    return (
        <div className="flex flex-wrap items-center justify-center gap-x-3 gap-y-2 p-6">
            {normalized.map(tag => (
                <Link
                    key={tag.name}
                    to="/chunks"
                    search={{ tags: tag.name } as any}
                    className="hover:text-primary transition-colors"
                    style={{ fontSize: fontSize(tag.weight), opacity: opacity(tag.weight) }}
                    title={`${tag.count} chunk${tag.count === 1 ? "" : "s"}`}
                >
                    {tag.name}
                </Link>
            ))}
        </div>
    );
}
```

- [ ] **Step 2: Integrate into browse page**

Add a tabs component to the browse page to switch between "Alphabetical" and "Tags" views. In `apps/web/src/routes/browse.tsx`:

```tsx
import { useState } from "react";
import { TagCloud } from "@/features/browse/tag-cloud";

// Add a tags query:
const { data: tagsData } = useQuery({
    queryKey: ["browse-tags"],
    queryFn: async () => unwrapEden(await api.api.tags.get()),
});

const tagsWithCounts = ((tagsData as any) ?? []) as Array<{ name: string; usage?: number }>;
// Transform to TagWithCount
const tagsForCloud = tagsWithCounts.map(t => ({ name: t.name, count: t.usage ?? 1 }));

// Simple tab state:
const [view, setView] = useState<"alphabetical" | "tags">("alphabetical");

// In the render:
<div className="mb-4 flex gap-2">
    <button onClick={() => setView("alphabetical")} className={`text-sm rounded px-3 py-1.5 ${view === "alphabetical" ? "bg-muted font-semibold" : "hover:bg-muted/50"}`}>
        A-Z
    </button>
    <button onClick={() => setView("tags")} className={`text-sm rounded px-3 py-1.5 ${view === "tags" ? "bg-muted font-semibold" : "hover:bg-muted/50"}`}>
        Tag cloud
    </button>
</div>

{view === "alphabetical" ? <AlphabeticalIndex chunks={chunks} /> : <TagCloud tags={tagsForCloud} />}
```

- [ ] **Step 3: Verify and commit**

```bash
git add apps/web/src/features/browse/tag-cloud.tsx apps/web/src/routes/browse.tsx
git commit -m "feat(browse): add weighted tag cloud view"
```

---

### Task 11: Codebase Dashboards

**Files:**
- Create: `apps/web/src/routes/codebases.$codebaseId.tsx`

**Context:** Each codebase gets its own landing page with summary stats, top chunks, recent activity, and category breakdown. Reuses existing API endpoints filtered by `codebaseId`.

- [ ] **Step 1: Create the route**

```typescript
// apps/web/src/routes/codebases.$codebaseId.tsx
import { useQuery } from "@tanstack/react-query";
import { createFileRoute, Link } from "@tanstack/react-router";
import { Blocks, Clock, Network, Tag } from "lucide-react";
import { Badge } from "@/components/ui/badge";
import { api } from "@/utils/api";
import { unwrapEden } from "@/utils/eden";

export const Route = createFileRoute("/codebases/$codebaseId")({
    component: CodebaseDashboard,
});

function CodebaseDashboard() {
    const { codebaseId } = Route.useParams();

    const statsQuery = useQuery({
        queryKey: ["codebase-stats", codebaseId],
        queryFn: async () => unwrapEden(await api.api.stats.get({ query: { codebaseId } as any })),
    });

    const chunksQuery = useQuery({
        queryKey: ["codebase-chunks", codebaseId],
        queryFn: async () => unwrapEden(await api.api.chunks.get({ query: { codebaseId, limit: "10", sort: "updated" } as any })),
    });

    const codebaseQuery = useQuery({
        queryKey: ["codebase", codebaseId],
        queryFn: async () => unwrapEden(await api.api.codebases({ id: codebaseId }).get()),
    });

    const codebase = codebaseQuery.data as any;
    const stats = statsQuery.data as any;
    const chunks = ((chunksQuery.data as any)?.chunks ?? []) as Array<{ id: string; title: string; type: string; updatedAt: string }>;

    return (
        <div className="container mx-auto max-w-6xl px-4 py-8">
            <div className="mb-8">
                <h1 className="text-2xl font-bold tracking-tight">{codebase?.name ?? "Codebase"}</h1>
                {codebase?.remoteUrl && (
                    <p className="text-xs text-muted-foreground font-mono mt-1">{codebase.remoteUrl}</p>
                )}
            </div>

            {/* Stats */}
            <div className="mb-8 grid grid-cols-2 gap-3 sm:grid-cols-4">
                <StatCard icon={Blocks} label="Chunks" value={stats?.chunks} />
                <StatCard icon={Network} label="Connections" value={stats?.connections} />
                <StatCard icon={Tag} label="Tags" value={stats?.tags} />
                <StatCard icon={Clock} label="Updated" value={codebase?.updatedAt ? new Date(codebase.updatedAt).toLocaleDateString() : "—"} />
            </div>

            {/* Recent chunks */}
            <div className="rounded-lg border">
                <div className="border-b px-4 py-3">
                    <h2 className="text-sm font-semibold">Recent chunks</h2>
                </div>
                <div className="divide-y">
                    {chunks.map(chunk => (
                        <Link
                            key={chunk.id}
                            to="/chunks/$chunkId"
                            params={{ chunkId: chunk.id }}
                            className="flex items-center justify-between px-4 py-3 hover:bg-muted/50 transition-colors"
                        >
                            <span className="truncate text-sm">{chunk.title}</span>
                            <Badge variant="secondary" size="sm" className="font-mono text-[9px]">
                                {chunk.type}
                            </Badge>
                        </Link>
                    ))}
                </div>
            </div>
        </div>
    );
}

function StatCard({ icon: Icon, label, value }: { icon: typeof Blocks; label: string; value: unknown }) {
    return (
        <div className="rounded-lg border bg-card p-4">
            <div className="flex items-center gap-2">
                <Icon className="text-muted-foreground size-4" />
                <span className="text-muted-foreground text-xs font-medium uppercase tracking-wide">{label}</span>
            </div>
            <div className="mt-2 text-2xl font-bold tabular-nums">
                {typeof value === "number" ? value : String(value ?? "—")}
            </div>
        </div>
    );
}
```

- [ ] **Step 2: Verify and commit**

```bash
git add apps/web/src/routes/codebases.$codebaseId.tsx
git commit -m "feat(codebases): add per-codebase dashboard page"
```

---

### Task 12: Featured Chunk Widget

**Files:**
- Create: `apps/web/src/features/chunks/featured-chunk-widget.tsx`
- Modify: `apps/web/src/routes/dashboard.tsx`

**Context:** A dashboard widget that shows a "chunk of the day" — a randomly selected (or rotating) chunk to surface forgotten knowledge. Use a deterministic daily rotation based on date.

- [ ] **Step 1: Create the widget**

```typescript
// apps/web/src/features/chunks/featured-chunk-widget.tsx
import { useQuery } from "@tanstack/react-query";
import { Link } from "@tanstack/react-router";
import { Sparkles } from "lucide-react";
import { useMemo } from "react";
import { Badge } from "@/components/ui/badge";
import { api } from "@/utils/api";
import { unwrapEden } from "@/utils/eden";

export function FeaturedChunkWidget() {
    const { data } = useQuery({
        queryKey: ["featured-chunks"],
        queryFn: async () => unwrapEden(await api.api.chunks.get({ query: { limit: "100" } as any })),
    });

    const chunks = ((data as any)?.chunks ?? []) as Array<{ id: string; title: string; type: string; summary?: string | null }>;

    // Deterministic daily selection
    const featured = useMemo(() => {
        if (chunks.length === 0) return null;
        const today = new Date().toISOString().slice(0, 10); // YYYY-MM-DD
        const hash = today.split("").reduce((acc, ch) => acc + ch.charCodeAt(0), 0);
        return chunks[hash % chunks.length];
    }, [chunks]);

    if (!featured) return null;

    return (
        <div className="rounded-lg border bg-gradient-to-br from-primary/5 to-transparent p-4">
            <div className="mb-2 flex items-center gap-1.5 text-xs font-semibold text-primary">
                <Sparkles className="size-3.5" />
                Chunk of the day
            </div>
            <Link
                to="/chunks/$chunkId"
                params={{ chunkId: featured.id }}
                className="group block"
            >
                <div className="flex items-center gap-2">
                    <h3 className="text-base font-semibold group-hover:text-primary transition-colors">
                        {featured.title}
                    </h3>
                    <Badge variant="secondary" size="sm" className="font-mono text-[9px]">
                        {featured.type}
                    </Badge>
                </div>
                {featured.summary && (
                    <p className="mt-1.5 line-clamp-2 text-xs text-muted-foreground">{featured.summary}</p>
                )}
            </Link>
        </div>
    );
}
```

- [ ] **Step 2: Add to dashboard**

In `apps/web/src/routes/dashboard.tsx`, import and render the widget in a suitable spot (e.g., above the recent chunks section).

```tsx
import { FeaturedChunkWidget } from "@/features/chunks/featured-chunk-widget";

// In the render:
<FeaturedChunkWidget />
```

- [ ] **Step 3: Verify and commit**

```bash
git add apps/web/src/features/chunks/featured-chunk-widget.tsx apps/web/src/routes/dashboard.tsx
git commit -m "feat(dashboard): add chunk-of-the-day featured widget"
```

---

### Task 13: Entry Point Markers ("Start here")

**Files:**
- Modify: `packages/db/src/schema/chunk.ts`
- Modify: `packages/db/src/repository/chunk.ts`
- Modify: `apps/web/src/routes/chunks.$chunkId.tsx`
- Modify: `apps/web/src/routes/dashboard.tsx`

**Context:** Add a boolean `isEntryPoint` flag on chunks. Users can mark chunks as entry points for a topic. Dashboard surfaces them as "Start here" reading paths.

- [ ] **Step 1: Add schema column**

In `packages/db/src/schema/chunk.ts`, add to the chunk table definition:
```typescript
isEntryPoint: boolean("is_entry_point").notNull().default(false),
```

Run `pnpm db:push` to apply.

- [ ] **Step 2: Update chunk repository and API**

Find the chunk service/routes that handle PATCH. Add `isEntryPoint` to the update params type, the Elysia schema, and the update function.

- [ ] **Step 3: Add toggle button to chunk detail**

In `apps/web/src/routes/chunks.$chunkId.tsx`, add a toggle button near other actions:
```tsx
<Button
    variant={chunk.isEntryPoint ? "default" : "outline"}
    size="sm"
    onClick={() => toggleEntryPoint()}
>
    <Flag className="size-3.5" />
    {chunk.isEntryPoint ? "Entry point" : "Mark as entry"}
</Button>
```

Implement `toggleEntryPoint` as a mutation that calls the PATCH endpoint.

- [ ] **Step 4: Add dashboard widget for entry points**

In `apps/web/src/routes/dashboard.tsx`, add a section listing all chunks where `isEntryPoint === true`. Label it "Start here".

- [ ] **Step 5: Commit**

```bash
git add packages/db/src/schema/chunk.ts packages/db/src/repository/chunk.ts packages/api/src/chunks apps/web/src/routes/chunks.\$chunkId.tsx apps/web/src/routes/dashboard.tsx
git commit -m "feat(chunks): add isEntryPoint flag and 'Start here' dashboard section"
```

---

# PHASE 4: READING EXPERIENCE

Six tasks that improve the reading experience on chunk detail pages.

---

### Task 14: Auto-generated Table of Contents in Chunk Detail

**Files:**
- Create: `apps/web/src/features/chunks/chunk-toc.tsx`
- Modify: `apps/web/src/routes/chunks.$chunkId.tsx`

**Context:** If a chunk's content has markdown headings (## or ###), auto-generate a sticky ToC sidebar (or right-rail on chunk detail). Similar to the compose view's ToC but based on content headings, not chunks.

- [ ] **Step 1: Create the ToC component**

```typescript
// apps/web/src/features/chunks/chunk-toc.tsx
import { useMemo } from "react";

interface Heading {
    level: number;
    text: string;
    id: string;
}

function slugify(text: string): string {
    return text.toLowerCase().replace(/[^a-z0-9]+/g, "-").replace(/^-|-$/g, "");
}

export function ChunkToc({ content }: { content: string }) {
    const headings = useMemo(() => {
        const lines = content.split("\n");
        const result: Heading[] = [];
        for (const line of lines) {
            const match = line.match(/^(#{2,4})\s+(.+)/);
            if (match) {
                const level = match[1]!.length;
                const text = match[2]!.trim();
                result.push({ level, text, id: slugify(text) });
            }
        }
        return result;
    }, [content]);

    if (headings.length < 2) return null;

    return (
        <nav className="sticky top-8 max-h-[calc(100vh-4rem)] overflow-y-auto">
            <div className="text-xs font-semibold uppercase tracking-wider text-muted-foreground mb-3">
                On this page
            </div>
            <ul className="space-y-1">
                {headings.map((h, i) => (
                    <li key={i} style={{ paddingLeft: `${(h.level - 2) * 12}px` }}>
                        <a
                            href={`#${h.id}`}
                            className="block truncate text-xs text-muted-foreground hover:text-foreground transition-colors py-1"
                            title={h.text}
                        >
                            {h.text}
                        </a>
                    </li>
                ))}
            </ul>
        </nav>
    );
}
```

- [ ] **Step 2: Render in chunk detail**

In `apps/web/src/routes/chunks.$chunkId.tsx`, add the ToC as a right-rail (or left sidebar). Likely wrap the existing content in a flex layout similar to what compose.tsx does.

- [ ] **Step 3: Commit**

```bash
git add apps/web/src/features/chunks/chunk-toc.tsx apps/web/src/routes/chunks.\$chunkId.tsx
git commit -m "feat(chunks): auto-generate table of contents from content headings"
```

---

### Task 15: Reading Time Estimator

**Files:**
- Create: `apps/web/src/features/chunks/reading-time.ts`
- Modify: `apps/web/src/routes/chunks.$chunkId.tsx`
- Modify: `apps/web/src/routes/chunks.index.tsx`

**Context:** Word count based reading time estimate shown on chunk detail and in chunk lists.

- [ ] **Step 1: Create the estimator**

```typescript
// apps/web/src/features/chunks/reading-time.ts
const WORDS_PER_MINUTE = 200;

export function estimateReadingTime(content: string | null | undefined): { minutes: number; label: string } {
    if (!content) return { minutes: 0, label: "< 1 min" };
    const words = content.trim().split(/\s+/).length;
    const minutes = Math.max(1, Math.round(words / WORDS_PER_MINUTE));
    return { minutes, label: `${minutes} min read` };
}
```

- [ ] **Step 2: Show on chunk detail**

In `apps/web/src/routes/chunks.$chunkId.tsx`, near the chunk metadata:
```tsx
import { estimateReadingTime } from "@/features/chunks/reading-time";

// In the render:
<span className="text-xs text-muted-foreground flex items-center gap-1">
    <Clock className="size-3" />
    {estimateReadingTime(chunk.content).label}
</span>
```

- [ ] **Step 3: (Optional) Show in chunks list**

In `apps/web/src/routes/chunks.index.tsx`, add the reading time badge to each row.

- [ ] **Step 4: Commit**

```bash
git add apps/web/src/features/chunks/reading-time.ts apps/web/src/routes/chunks.\$chunkId.tsx apps/web/src/routes/chunks.index.tsx
git commit -m "feat(chunks): add reading time estimate"
```

---

### Task 16: Chunk Type Icons

**Files:**
- Create: `apps/web/src/features/chunks/chunk-type-icon.tsx`

**Context:** A component that renders an appropriate lucide-react icon for each chunk type. Use this consistently across the UI wherever a chunk type is shown.

- [ ] **Step 1: Create the component**

```typescript
// apps/web/src/features/chunks/chunk-type-icon.tsx
import { BookOpen, ClipboardCheck, Database, FileText, Lightbulb, Wrench, type LucideIcon } from "lucide-react";

const ICON_MAP: Record<string, LucideIcon> = {
    note: Lightbulb,
    document: BookOpen,
    reference: FileText,
    schema: Database,
    checklist: ClipboardCheck,
    guide: Wrench,
};

export function ChunkTypeIcon({ type, className }: { type: string; className?: string }) {
    const Icon = ICON_MAP[type] ?? FileText;
    return <Icon className={className ?? "size-3.5"} />;
}
```

- [ ] **Step 2: Use it in chunk lists and detail pages**

Find places where chunk type is rendered (chunks list, search results, dashboard widgets). Add the icon next to the type badge.

Example: in `chunks.index.tsx`:
```tsx
<div className="flex items-center gap-1.5">
    <ChunkTypeIcon type={chunk.type} />
    <Badge variant="secondary" size="sm" className="font-mono text-[9px]">
        {chunk.type}
    </Badge>
</div>
```

- [ ] **Step 3: Commit**

```bash
git add apps/web/src/features/chunks/chunk-type-icon.tsx
git commit -m "feat(chunks): add chunk type icons for visual recognition"
```

---

### Task 17: Content Thumbnail (Text Shape Preview)

**Files:**
- Create: `apps/web/src/features/chunks/content-thumbnail.tsx`

**Context:** For card grid views, generate a small SVG "text shape" based on chunk content — like a code minimap. Just renders lines of varying lengths based on actual line lengths in the content.

- [ ] **Step 1: Create the component**

```typescript
// apps/web/src/features/chunks/content-thumbnail.tsx
import { useMemo } from "react";

export function ContentThumbnail({ content, className }: { content: string | null | undefined; className?: string }) {
    const lines = useMemo(() => {
        if (!content) return [];
        return content
            .split("\n")
            .slice(0, 20)
            .map(line => Math.min(100, line.trim().length));
    }, [content]);

    if (lines.length === 0) return null;

    return (
        <svg
            className={className ?? "h-16 w-full"}
            viewBox="0 0 100 80"
            preserveAspectRatio="none"
            xmlns="http://www.w3.org/2000/svg"
        >
            {lines.map((width, i) => (
                <rect
                    key={i}
                    x={2}
                    y={i * 4}
                    width={width}
                    height={1.5}
                    fill="currentColor"
                    opacity={0.2}
                    rx={0.5}
                />
            ))}
        </svg>
    );
}
```

- [ ] **Step 2: Use in card grid**

In `apps/web/src/features/chunks/chunk-card-grid.tsx` (from Task 5), add the thumbnail at the bottom of each card:
```tsx
{chunk.content && (
    <div className="mt-3 text-muted-foreground">
        <ContentThumbnail content={chunk.content} />
    </div>
)}
```

- [ ] **Step 3: Commit**

```bash
git add apps/web/src/features/chunks/content-thumbnail.tsx apps/web/src/features/chunks/chunk-card-grid.tsx
git commit -m "feat(chunks): add content thumbnail text-shape preview"
```

---

### Task 18: Scroll Progress Bar on Chunk Detail

**Files:**
- Modify: `apps/web/src/routes/chunks.$chunkId.tsx`

**Context:** Same progress bar pattern from the compose view — a thin bar at the top of the viewport showing scroll position through the chunk content.

- [ ] **Step 1: Add scroll progress state and effect**

In `apps/web/src/routes/chunks.$chunkId.tsx`:

```typescript
const [scrollProgress, setScrollProgress] = useState(0);

useEffect(() => {
    function handleScroll() {
        const scrollTop = window.scrollY;
        const docHeight = document.documentElement.scrollHeight - window.innerHeight;
        const progress = docHeight > 0 ? (scrollTop / docHeight) * 100 : 0;
        setScrollProgress(Math.min(100, Math.max(0, progress)));
    }
    window.addEventListener("scroll", handleScroll, { passive: true });
    handleScroll();
    return () => window.removeEventListener("scroll", handleScroll);
}, []);
```

- [ ] **Step 2: Render the progress bar**

At the top of the return:
```tsx
<div className="fixed top-0 left-0 right-0 z-50 h-0.5 bg-transparent print:hidden">
    <div className="h-full bg-primary transition-[width] duration-100 ease-out" style={{ width: `${scrollProgress}%` }} />
</div>
```

- [ ] **Step 3: Commit**

```bash
git add apps/web/src/routes/chunks.\$chunkId.tsx
git commit -m "feat(chunks): add reading progress bar on chunk detail"
```

---

### Task 19: Adjustable Reader Settings

**Files:**
- Create: `apps/web/src/hooks/use-reader-settings.ts`
- Create: `apps/web/src/features/chunks/reader-settings.tsx`
- Modify: `apps/web/src/routes/chunks.$chunkId.tsx`

**Context:** Font size, line height, and column width controls for chunk detail. Stored in localStorage per-user.

- [ ] **Step 1: Create the settings hook**

```typescript
// apps/web/src/hooks/use-reader-settings.ts
import { useCallback, useEffect, useState } from "react";

const STORAGE_KEY = "fubbik-reader-settings";

export interface ReaderSettings {
    fontSize: "sm" | "base" | "lg" | "xl";
    lineHeight: "tight" | "normal" | "relaxed";
    maxWidth: "narrow" | "normal" | "wide";
}

const DEFAULTS: ReaderSettings = {
    fontSize: "base",
    lineHeight: "normal",
    maxWidth: "normal",
};

export function useReaderSettings() {
    const [settings, setSettings] = useState<ReaderSettings>(DEFAULTS);

    useEffect(() => {
        try {
            const stored = localStorage.getItem(STORAGE_KEY);
            if (stored) setSettings({ ...DEFAULTS, ...JSON.parse(stored) });
        } catch {
            // ignore
        }
    }, []);

    const update = useCallback((partial: Partial<ReaderSettings>) => {
        setSettings(prev => {
            const next = { ...prev, ...partial };
            try {
                localStorage.setItem(STORAGE_KEY, JSON.stringify(next));
            } catch {
                // ignore
            }
            return next;
        });
    }, []);

    return { settings, update };
}

export function getReaderClasses(settings: ReaderSettings): string {
    const fontSize = {
        sm: "text-sm",
        base: "text-base",
        lg: "text-lg",
        xl: "text-xl",
    }[settings.fontSize];
    const lineHeight = {
        tight: "leading-tight",
        normal: "leading-relaxed",
        relaxed: "leading-loose",
    }[settings.lineHeight];
    const maxWidth = {
        narrow: "max-w-2xl",
        normal: "max-w-3xl",
        wide: "max-w-5xl",
    }[settings.maxWidth];
    return `${fontSize} ${lineHeight} ${maxWidth}`;
}
```

- [ ] **Step 2: Create the popover component**

```typescript
// apps/web/src/features/chunks/reader-settings.tsx
import { Type } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Popover, PopoverContent, PopoverTrigger } from "@/components/ui/popover";
import { useReaderSettings } from "@/hooks/use-reader-settings";

export function ReaderSettingsPopover() {
    const { settings, update } = useReaderSettings();

    return (
        <Popover>
            <PopoverTrigger>
                <Button variant="ghost" size="sm" className="gap-1.5" render={<span />}>
                    <Type className="size-3.5" />
                    Reader
                </Button>
            </PopoverTrigger>
            <PopoverContent className="w-64 p-4">
                <div className="space-y-4">
                    <div>
                        <label className="text-xs font-semibold text-muted-foreground">Font size</label>
                        <div className="mt-1 grid grid-cols-4 gap-1">
                            {(["sm", "base", "lg", "xl"] as const).map(size => (
                                <button
                                    key={size}
                                    type="button"
                                    onClick={() => update({ fontSize: size })}
                                    className={`rounded border px-2 py-1 text-xs ${settings.fontSize === size ? "bg-muted font-semibold" : "hover:bg-muted/50"}`}
                                >
                                    {size.toUpperCase()}
                                </button>
                            ))}
                        </div>
                    </div>
                    <div>
                        <label className="text-xs font-semibold text-muted-foreground">Line height</label>
                        <div className="mt-1 grid grid-cols-3 gap-1">
                            {(["tight", "normal", "relaxed"] as const).map(lh => (
                                <button
                                    key={lh}
                                    type="button"
                                    onClick={() => update({ lineHeight: lh })}
                                    className={`rounded border px-2 py-1 text-xs ${settings.lineHeight === lh ? "bg-muted font-semibold" : "hover:bg-muted/50"}`}
                                >
                                    {lh}
                                </button>
                            ))}
                        </div>
                    </div>
                    <div>
                        <label className="text-xs font-semibold text-muted-foreground">Column width</label>
                        <div className="mt-1 grid grid-cols-3 gap-1">
                            {(["narrow", "normal", "wide"] as const).map(mw => (
                                <button
                                    key={mw}
                                    type="button"
                                    onClick={() => update({ maxWidth: mw })}
                                    className={`rounded border px-2 py-1 text-xs ${settings.maxWidth === mw ? "bg-muted font-semibold" : "hover:bg-muted/50"}`}
                                >
                                    {mw}
                                </button>
                            ))}
                        </div>
                    </div>
                </div>
            </PopoverContent>
        </Popover>
    );
}
```

- [ ] **Step 3: Apply settings to chunk detail**

In `apps/web/src/routes/chunks.$chunkId.tsx`:

```typescript
import { useReaderSettings, getReaderClasses } from "@/hooks/use-reader-settings";
import { ReaderSettingsPopover } from "@/features/chunks/reader-settings";

const { settings } = useReaderSettings();
const readerClasses = getReaderClasses(settings);

// Apply to the content container:
<div className={`prose dark:prose-invert mx-auto ${readerClasses}`}>
    ...
</div>

// Add the popover to the action bar:
<ReaderSettingsPopover />
```

- [ ] **Step 4: Commit**

```bash
git add apps/web/src/hooks/use-reader-settings.ts apps/web/src/features/chunks/reader-settings.tsx apps/web/src/routes/chunks.\$chunkId.tsx
git commit -m "feat(chunks): add adjustable reader settings (font size, line height, width)"
```

---

# PHASE 5: DISCOVERY & CLUSTERING

Five tasks that use AI/embeddings and new schema to enable deeper discovery.

---

### Task 20: "Show Similar" Button on Chunk Detail

**Files:**
- Create: `apps/web/src/features/chunks/similar-button.tsx`
- Modify: `apps/web/src/routes/chunks.$chunkId.tsx`

**Context:** A button on chunk detail that navigates to search with a pre-filled `similar-to:"current title"` clause. Leverages existing semantic search.

- [ ] **Step 1: Create the button**

```typescript
// apps/web/src/features/chunks/similar-button.tsx
import { useNavigate } from "@tanstack/react-router";
import { Sparkles } from "lucide-react";
import { Button } from "@/components/ui/button";

export function SimilarButton({ chunkTitle }: { chunkTitle: string }) {
    const navigate = useNavigate();

    function handleClick() {
        const q = `similar-to:"${chunkTitle}"`;
        void navigate({ to: "/search", search: { q } as any });
    }

    return (
        <Button variant="outline" size="sm" onClick={handleClick} className="gap-1.5">
            <Sparkles className="size-3.5" />
            Show similar
        </Button>
    );
}
```

- [ ] **Step 2: Use in chunk detail**

```tsx
<SimilarButton chunkTitle={chunk.title} />
```

- [ ] **Step 3: Commit**

```bash
git add apps/web/src/features/chunks/similar-button.tsx apps/web/src/routes/chunks.\$chunkId.tsx
git commit -m "feat(chunks): add 'show similar' button using semantic search"
```

---

### Task 21: Embedding-Based Clusters

**Files:**
- Create: `packages/api/src/chunks/clusters.ts`
- Create: `packages/api/src/chunks/cluster-routes.ts`
- Create: `apps/web/src/routes/browse.clusters.tsx`
- Modify: `packages/api/src/index.ts`

**Context:** Use pgvector to compute topical clusters of chunks. For v1, a simple algorithm: find the "densest" groups by picking a seed chunk and gathering its N nearest neighbors by embedding similarity.

- [ ] **Step 1: Create the clustering service**

```typescript
// packages/api/src/chunks/clusters.ts
import { db } from "@fubbik/db";
import { sql } from "drizzle-orm";
import { Effect } from "effect";
import { DatabaseError } from "@fubbik/db/errors";

export interface Cluster {
    seedId: string;
    seedTitle: string;
    members: Array<{ id: string; title: string; type: string; similarity: number }>;
}

export function computeClusters(userId: string, maxClusters = 10, clusterSize = 8) {
    return Effect.tryPromise({
        try: async () => {
            // Get all chunks with embeddings for this user
            const seeds = await db.execute(sql`
                SELECT id, title, type
                FROM chunk
                WHERE user_id = ${userId} AND embedding IS NOT NULL AND archived_at IS NULL
                ORDER BY updated_at DESC
                LIMIT ${maxClusters}
            `);

            const clusters: Cluster[] = [];
            const used = new Set<string>();

            for (const seed of seeds.rows as Array<{ id: string; title: string; type: string }>) {
                if (used.has(seed.id)) continue;

                // Find nearest neighbors by cosine distance
                const neighbors = await db.execute(sql`
                    SELECT id, title, type, 1 - (embedding <=> (SELECT embedding FROM chunk WHERE id = ${seed.id})) AS similarity
                    FROM chunk
                    WHERE user_id = ${userId} AND embedding IS NOT NULL AND archived_at IS NULL AND id != ${seed.id}
                    ORDER BY embedding <=> (SELECT embedding FROM chunk WHERE id = ${seed.id})
                    LIMIT ${clusterSize}
                `);

                const members = (neighbors.rows as Array<{ id: string; title: string; type: string; similarity: string }>).map(r => ({
                    id: r.id,
                    title: r.title,
                    type: r.type,
                    similarity: Number(r.similarity),
                }));

                used.add(seed.id);
                for (const m of members) used.add(m.id);

                clusters.push({
                    seedId: seed.id,
                    seedTitle: seed.title,
                    members,
                });
            }

            return clusters;
        },
        catch: cause => new DatabaseError({ cause }),
    });
}
```

- [ ] **Step 2: Add API route**

```typescript
// packages/api/src/chunks/cluster-routes.ts
import { Effect } from "effect";
import { Elysia } from "elysia";
import { requireSession } from "../require-session";
import { computeClusters } from "./clusters";

export const clusterRoutes = new Elysia().get(
    "/chunks/clusters",
    ctx =>
        Effect.runPromise(
            requireSession(ctx).pipe(
                Effect.flatMap(session => computeClusters(session.user.id).pipe(
                    Effect.orElse(() => Effect.succeed([]))
                )),
            ),
        ),
);
```

Register in `packages/api/src/index.ts`:
```typescript
import { clusterRoutes } from "./chunks/cluster-routes";
// ...
.use(clusterRoutes)
```

- [ ] **Step 3: Create the cluster browse page**

```typescript
// apps/web/src/routes/browse.clusters.tsx
import { useQuery } from "@tanstack/react-query";
import { createFileRoute, Link } from "@tanstack/react-router";
import { api } from "@/utils/api";
import { unwrapEden } from "@/utils/eden";

export const Route = createFileRoute("/browse/clusters")({
    component: ClustersPage,
});

function ClustersPage() {
    const { data } = useQuery({
        queryKey: ["chunk-clusters"],
        queryFn: async () => unwrapEden(await api.api.chunks.clusters.get()),
    });

    const clusters = (data as any) ?? [];

    return (
        <div className="container mx-auto max-w-6xl px-4 py-8">
            <h1 className="mb-6 text-2xl font-bold">Topic Clusters</h1>
            {clusters.length === 0 ? (
                <p className="text-muted-foreground">No clusters found. Chunks need embeddings for clustering to work.</p>
            ) : (
                <div className="space-y-8">
                    {clusters.map((c: any) => (
                        <section key={c.seedId} className="rounded-lg border p-4">
                            <Link to="/chunks/$chunkId" params={{ chunkId: c.seedId }} className="text-lg font-semibold hover:text-primary">
                                {c.seedTitle}
                            </Link>
                            <div className="mt-3 grid gap-2 sm:grid-cols-2">
                                {c.members.map((m: any) => (
                                    <Link
                                        key={m.id}
                                        to="/chunks/$chunkId"
                                        params={{ chunkId: m.id }}
                                        className="flex items-center justify-between rounded border px-3 py-2 hover:bg-muted/50"
                                    >
                                        <span className="text-sm truncate">{m.title}</span>
                                        <span className="text-xs text-muted-foreground">{Math.round(m.similarity * 100)}%</span>
                                    </Link>
                                ))}
                            </div>
                        </section>
                    ))}
                </div>
            )}
        </div>
    );
}
```

- [ ] **Step 4: Commit**

```bash
git add packages/api/src/chunks/clusters.ts packages/api/src/chunks/cluster-routes.ts packages/api/src/index.ts apps/web/src/routes/browse.clusters.tsx
git commit -m "feat(browse): add embedding-based topic cluster browsing"
```

---

### Task 22: Smart Collections

**Files:**
- Create: `apps/web/src/features/chunks/smart-collections.tsx`
- Modify: `apps/web/src/routes/dashboard.tsx`

**Context:** A sidebar or dashboard section with auto-updating "smart" collections: "Recently updated", "Needs review", "Well-connected", "Deep dives", "Orphans". Each is a link that pre-fills the search query builder.

- [ ] **Step 1: Create the component**

```typescript
// apps/web/src/features/chunks/smart-collections.tsx
import { Link } from "@tanstack/react-router";
import { AlertTriangle, Clock, Link2Off, Network, ScrollText } from "lucide-react";

const COLLECTIONS = [
    { label: "Recently updated", icon: Clock, q: "updated:7d" },
    { label: "Needs review", icon: AlertTriangle, q: "review:draft" },
    { label: "Well-connected", icon: Network, q: "connections:5+" },
    { label: "Deep dives", icon: ScrollText, q: "type:document" },
    { label: "Orphans", icon: Link2Off, q: "connections:0+" },
];

export function SmartCollections() {
    return (
        <div className="rounded-lg border">
            <div className="border-b px-4 py-3">
                <h2 className="text-sm font-semibold">Smart collections</h2>
            </div>
            <div className="divide-y">
                {COLLECTIONS.map(col => (
                    <Link
                        key={col.label}
                        to="/search"
                        search={{ q: col.q } as any}
                        className="flex items-center gap-3 px-4 py-3 hover:bg-muted/50 transition-colors"
                    >
                        <col.icon className="size-4 text-muted-foreground" />
                        <span className="text-sm">{col.label}</span>
                        <code className="ml-auto text-[10px] text-muted-foreground font-mono">{col.q}</code>
                    </Link>
                ))}
            </div>
        </div>
    );
}
```

- [ ] **Step 2: Add to dashboard**

In `apps/web/src/routes/dashboard.tsx`:
```tsx
import { SmartCollections } from "@/features/chunks/smart-collections";
// In the render:
<SmartCollections />
```

- [ ] **Step 3: Commit**

```bash
git add apps/web/src/features/chunks/smart-collections.tsx apps/web/src/routes/dashboard.tsx
git commit -m "feat(dashboard): add smart collections sidebar"
```

---

### Task 23: Learning Paths

**Files:**
- Create: `packages/db/src/schema/learning-path.ts`
- Create: `packages/db/src/repository/learning-path.ts`
- Create: `packages/api/src/learning-paths/service.ts`
- Create: `packages/api/src/learning-paths/routes.ts`
- Create: `apps/web/src/routes/learn.tsx`
- Create: `apps/web/src/routes/learn.$pathId.tsx`
- Modify: `packages/api/src/index.ts`
- Modify: `packages/db/src/schema/index.ts`
- Modify: `packages/db/src/repository/index.ts`

**Context:** A new entity — ordered sequences of chunks that form a guided reading path. Users can create, share, and follow paths.

- [ ] **Step 1: Create the schema**

```typescript
// packages/db/src/schema/learning-path.ts
import { relations } from "drizzle-orm";
import { index, integer, jsonb, pgTable, text, timestamp } from "drizzle-orm/pg-core";
import { user } from "./auth";

export const learningPath = pgTable(
    "learning_path",
    {
        id: text("id").primaryKey(),
        title: text("title").notNull(),
        description: text("description"),
        chunkIds: jsonb("chunk_ids").$type<string[]>().notNull().default([]),
        userId: text("user_id").notNull().references(() => user.id, { onDelete: "cascade" }),
        createdAt: timestamp("created_at").defaultNow().notNull(),
        updatedAt: timestamp("updated_at").defaultNow().$onUpdate(() => new Date()).notNull(),
    },
    table => [index("learning_path_userId_idx").on(table.userId)],
);

export const learningPathRelations = relations(learningPath, ({ one }) => ({
    user: one(user, { fields: [learningPath.userId], references: [user.id] }),
}));
```

Add to `packages/db/src/schema/index.ts`:
```typescript
export * from "./learning-path";
```

Run `pnpm db:push`.

- [ ] **Step 2: Create repository**

```typescript
// packages/db/src/repository/learning-path.ts
import { and, desc, eq } from "drizzle-orm";
import { Effect } from "effect";
import { DatabaseError } from "../errors";
import { db } from "../index";
import { learningPath } from "../schema/learning-path";

export function listLearningPaths(userId: string) {
    return Effect.tryPromise({
        try: () =>
            db.select().from(learningPath).where(eq(learningPath.userId, userId)).orderBy(desc(learningPath.updatedAt)),
        catch: cause => new DatabaseError({ cause }),
    });
}

export function getLearningPath(id: string, userId: string) {
    return Effect.tryPromise({
        try: async () => {
            const [row] = await db.select().from(learningPath).where(and(eq(learningPath.id, id), eq(learningPath.userId, userId)));
            return row ?? null;
        },
        catch: cause => new DatabaseError({ cause }),
    });
}

export function createLearningPath(params: {
    id: string;
    title: string;
    description?: string;
    chunkIds: string[];
    userId: string;
}) {
    return Effect.tryPromise({
        try: async () => {
            const [created] = await db.insert(learningPath).values(params).returning();
            return created;
        },
        catch: cause => new DatabaseError({ cause }),
    });
}

export function updateLearningPath(id: string, userId: string, params: { title?: string; description?: string; chunkIds?: string[] }) {
    return Effect.tryPromise({
        try: async () => {
            const [updated] = await db
                .update(learningPath)
                .set(params)
                .where(and(eq(learningPath.id, id), eq(learningPath.userId, userId)))
                .returning();
            return updated ?? null;
        },
        catch: cause => new DatabaseError({ cause }),
    });
}

export function deleteLearningPath(id: string, userId: string) {
    return Effect.tryPromise({
        try: async () => {
            const [deleted] = await db
                .delete(learningPath)
                .where(and(eq(learningPath.id, id), eq(learningPath.userId, userId)))
                .returning();
            return deleted ?? null;
        },
        catch: cause => new DatabaseError({ cause }),
    });
}
```

Add to `packages/db/src/repository/index.ts`:
```typescript
export * from "./learning-path";
```

- [ ] **Step 3: Create API service and routes**

```typescript
// packages/api/src/learning-paths/service.ts
export { listLearningPaths, getLearningPath, createLearningPath, updateLearningPath, deleteLearningPath } from "@fubbik/db/repository";
```

```typescript
// packages/api/src/learning-paths/routes.ts
import { Effect } from "effect";
import { Elysia, t } from "elysia";
import { requireSession } from "../require-session";
import * as service from "./service";

export const learningPathRoutes = new Elysia()
    .get("/learning-paths", ctx =>
        Effect.runPromise(requireSession(ctx).pipe(Effect.flatMap(session => service.listLearningPaths(session.user.id)))),
    )
    .get("/learning-paths/:id", ctx =>
        Effect.runPromise(
            requireSession(ctx).pipe(Effect.flatMap(session => service.getLearningPath(ctx.params.id, session.user.id))),
        ),
    )
    .post(
        "/learning-paths",
        ctx =>
            Effect.runPromise(
                requireSession(ctx).pipe(
                    Effect.flatMap(session =>
                        service.createLearningPath({
                            id: crypto.randomUUID(),
                            title: ctx.body.title,
                            description: ctx.body.description,
                            chunkIds: ctx.body.chunkIds,
                            userId: session.user.id,
                        }),
                    ),
                ),
            ),
        {
            body: t.Object({
                title: t.String(),
                description: t.Optional(t.String()),
                chunkIds: t.Array(t.String()),
            }),
        },
    );
```

Register in `packages/api/src/index.ts`:
```typescript
import { learningPathRoutes } from "./learning-paths/routes";
// ...
.use(learningPathRoutes)
```

- [ ] **Step 4: Create the frontend route**

```typescript
// apps/web/src/routes/learn.tsx
import { useQuery } from "@tanstack/react-query";
import { createFileRoute, Link } from "@tanstack/react-router";
import { BookOpen } from "lucide-react";
import { api } from "@/utils/api";
import { unwrapEden } from "@/utils/eden";

export const Route = createFileRoute("/learn")({
    component: LearnPage,
});

function LearnPage() {
    const { data } = useQuery({
        queryKey: ["learning-paths"],
        queryFn: async () => unwrapEden(await api.api["learning-paths"].get()),
    });

    const paths = ((data as any) ?? []) as Array<{ id: string; title: string; description?: string; chunkIds: string[] }>;

    return (
        <div className="container mx-auto max-w-4xl px-4 py-8">
            <h1 className="mb-6 text-2xl font-bold">Learning Paths</h1>
            {paths.length === 0 ? (
                <p className="text-muted-foreground">No learning paths yet.</p>
            ) : (
                <div className="space-y-3">
                    {paths.map(p => (
                        <Link
                            key={p.id}
                            to="/learn/$pathId"
                            params={{ pathId: p.id }}
                            className="flex items-start gap-3 rounded-lg border bg-card p-4 hover:bg-muted/50 transition-colors"
                        >
                            <BookOpen className="mt-0.5 size-5 text-muted-foreground shrink-0" />
                            <div className="min-w-0 flex-1">
                                <div className="font-semibold">{p.title}</div>
                                {p.description && <p className="mt-1 text-sm text-muted-foreground">{p.description}</p>}
                                <p className="mt-2 text-xs text-muted-foreground">{p.chunkIds.length} chunks in sequence</p>
                            </div>
                        </Link>
                    ))}
                </div>
            )}
        </div>
    );
}
```

Also create `apps/web/src/routes/learn.$pathId.tsx` for the detail view — a page that shows the path metadata and the ordered list of chunks with "Start", "Next", "Previous" navigation.

- [ ] **Step 5: Commit**

```bash
git add packages/db/src/schema/learning-path.ts packages/db/src/repository/learning-path.ts packages/db/src/schema/index.ts packages/db/src/repository/index.ts packages/api/src/learning-paths apps/web/src/routes/learn.tsx apps/web/src/routes/learn.\$pathId.tsx packages/api/src/index.ts
git commit -m "feat(learn): add learning paths — ordered chunk sequences"
```

---

### Task 24: "You Might Have Missed This" Widget

**Files:**
- Create: `apps/web/src/features/dashboard/missed-chunks-widget.tsx`
- Modify: `apps/web/src/routes/dashboard.tsx`

**Context:** Dashboard widget showing chunks the user hasn't visited in 30+ days (based on the visit history from Task 3 — or `updated:>30d` if no visit tracking yet). Reuses existing search.

- [ ] **Step 1: Create the widget**

```typescript
// apps/web/src/features/dashboard/missed-chunks-widget.tsx
import { useQuery } from "@tanstack/react-query";
import { Link } from "@tanstack/react-router";
import { EyeOff } from "lucide-react";
import { Badge } from "@/components/ui/badge";
import { api } from "@/utils/api";
import { unwrapEden } from "@/utils/eden";

export function MissedChunksWidget() {
    // Use search to find old chunks
    const { data } = useQuery({
        queryKey: ["missed-chunks"],
        queryFn: async () =>
            unwrapEden(
                await api.api.chunks.get({
                    query: { sort: "oldest", limit: "5" } as any,
                }),
            ),
    });

    const chunks = ((data as any)?.chunks ?? []) as Array<{ id: string; title: string; type: string }>;

    if (chunks.length === 0) return null;

    return (
        <div className="rounded-lg border">
            <div className="border-b px-4 py-3">
                <div className="flex items-center gap-1.5">
                    <EyeOff className="size-3.5 text-muted-foreground" />
                    <h2 className="text-sm font-semibold">You might have missed this</h2>
                </div>
            </div>
            <div className="divide-y">
                {chunks.map(chunk => (
                    <Link
                        key={chunk.id}
                        to="/chunks/$chunkId"
                        params={{ chunkId: chunk.id }}
                        className="flex items-center justify-between px-4 py-3 hover:bg-muted/50 transition-colors"
                    >
                        <span className="truncate text-sm">{chunk.title}</span>
                        <Badge variant="secondary" size="sm" className="font-mono text-[9px]">
                            {chunk.type}
                        </Badge>
                    </Link>
                ))}
            </div>
        </div>
    );
}
```

- [ ] **Step 2: Add to dashboard**

In `apps/web/src/routes/dashboard.tsx`:
```tsx
import { MissedChunksWidget } from "@/features/dashboard/missed-chunks-widget";
<MissedChunksWidget />
```

- [ ] **Step 3: Commit**

```bash
git add apps/web/src/features/dashboard/missed-chunks-widget.tsx apps/web/src/routes/dashboard.tsx
git commit -m "feat(dashboard): add 'you might have missed this' widget"
```

---

## Final Verification

After completing all phases:

- [ ] **Step 1: Run full type check**

```bash
pnpm --filter web run check-types 2>&1 | head -30
pnpm --filter @fubbik/api run check-types 2>&1 | head -30
```

Fix any errors.

- [ ] **Step 2: Build**

```bash
pnpm build
```

- [ ] **Step 3: Smoke test each phase**

1. **Phase 1:** Hover a chunk link → preview appears. Open a chunk with titles in content → titles are auto-linked. Visit 3 chunks → trail sidebar shows them. Click focus mode → UI hides. Switch to card grid → cards show.
2. **Phase 2:** Press `j/k` → scrolls. Press `?` → help modal appears. Press `1` on chunks list → first chunk opens. Press `Ctrl+O` → quick open appears.
3. **Phase 3:** Navigate to `/browse` → A-Z index shows. Click "Tag cloud" → cloud renders. Navigate to `/codebases/<id>` → dashboard shows. Dashboard has featured chunk.
4. **Phase 4:** Chunk detail shows ToC, reading time, type icon, progress bar. Open reader settings → change font size → content resizes.
5. **Phase 5:** Click "Show similar" → search opens with similar-to clause. Navigate to `/browse/clusters` → clusters render. Dashboard has smart collections + missed chunks widgets.

- [ ] **Step 4: Commit any final fixes**

```bash
git commit -am "fix: final smoke test corrections"
```
