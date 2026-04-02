# Docs Usability Improvements — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make the `/docs` page feel like a polished documentation site with URL persistence, keyboard nav, sequential reading, sticky TOC, mobile support, grouped search results, and quality-of-life features.

**Architecture:** All changes are frontend-only, primarily in `document-browser.tsx` and `docs.tsx`. The route gains search params (`id`, `section`) for URL-driven navigation. The component gets a three-column layout on wide screens (sidebar | content | sticky TOC). No backend changes.

**Tech Stack:** React, TanStack Router (search params), lucide-react icons, Tailwind CSS

---

## File Structure

### Modified Files
| File | Changes |
|------|---------|
| `apps/web/src/routes/docs.tsx` | Add `id` and `section` search params, pass to DocumentBrowser |
| `apps/web/src/features/documents/document-browser.tsx` | All usability improvements (URL sync, auto-select, keyboard nav, prev/next, sticky TOC, mobile drawer, grouped search, reading progress, copy link, doc ordering) |

---

### Task 1: Auto-Select First Document + URL-Driven Navigation

**Files:**
- Modify: `apps/web/src/routes/docs.tsx`
- Modify: `apps/web/src/features/documents/document-browser.tsx`

- [ ] **Step 1: Add `id` and `section` search params to the route**

In `apps/web/src/routes/docs.tsx`, update `validateSearch`:

```typescript
validateSearch: (search: Record<string, unknown>): { tab?: string; id?: string; section?: string } => ({
    tab: (search.tab as string) ?? undefined,
    id: (search.id as string) ?? undefined,
    section: (search.section as string) ?? undefined
}),
```

Pass these to `DocumentBrowser`:

```tsx
{tab === "docs" && <DocumentBrowser initialDocId={search.id} initialSection={search.section} />}
```

Add `useSearch` import if not present, and update the `search` variable usage.

- [ ] **Step 2: Accept props and sync URL in DocumentBrowser**

In `apps/web/src/features/documents/document-browser.tsx`, update the component signature:

```typescript
interface DocumentBrowserProps {
    initialDocId?: string;
    initialSection?: string;
}

export function DocumentBrowser({ initialDocId, initialSection }: DocumentBrowserProps) {
```

Add `useNavigate` import from `@tanstack/react-router`:

```typescript
import { Link, useNavigate } from "@tanstack/react-router";
```

Replace the `useState` for `selectedId` with URL-synced state:

```typescript
const navigate = useNavigate();
const [selectedId, setSelectedIdState] = useState<string | null>(initialDocId ?? null);

const setSelectedId = (id: string | null) => {
    setSelectedIdState(id);
    navigate({
        to: "/docs",
        search: (prev: Record<string, unknown>) => ({ ...prev, id: id ?? undefined, section: undefined }),
        replace: true
    });
};
```

- [ ] **Step 3: Auto-select first document when none selected**

After the `documents` variable is set, add an effect:

```typescript
useEffect(() => {
    if (!selectedId && documents.length > 0 && !isSearching) {
        const firstId = initialDocId && documents.some(d => d.id === initialDocId) ? initialDocId : documents[0]!.id;
        setSelectedIdState(firstId);
        navigate({
            to: "/docs",
            search: (prev: Record<string, unknown>) => ({ ...prev, id: firstId }),
            replace: true
        });
    }
}, [documents]);
```

Also add `useEffect` import and handle `initialSection` — after detail loads, scroll to section:

```typescript
useEffect(() => {
    if (initialSection && detail) {
        const el = document.getElementById(`section-${initialSection}`);
        el?.scrollIntoView({ behavior: "smooth", block: "start" });
    }
}, [initialSection, detail]);
```

- [ ] **Step 4: Remove the empty "Select a document" placeholder**

Remove the block:
```tsx
{!selectedId && (
    <div className="flex flex-col items-center gap-3 py-16">
        ...
    </div>
)}
```

Since we auto-select, this is no longer needed.

- [ ] **Step 5: Verify**

Reload `/docs` — should auto-select first doc. Navigate to `/docs?id=<some-id>` — should load that doc. Refresh — should persist selection.

- [ ] **Step 6: Commit**

```bash
git add apps/web/src/routes/docs.tsx apps/web/src/features/documents/document-browser.tsx
git commit -m "feat(docs): auto-select first document and URL-driven navigation"
```

---

### Task 2: Previous/Next Navigation

**Files:**
- Modify: `apps/web/src/features/documents/document-browser.tsx`

- [ ] **Step 1: Compute prev/next documents**

After the `detail` variable, add:

```typescript
const currentIndex = documents.findIndex(d => d.id === selectedId);
const prevDoc = currentIndex > 0 ? documents[currentIndex - 1] : null;
const nextDoc = currentIndex < documents.length - 1 ? documents[currentIndex + 1] : null;
```

- [ ] **Step 2: Add prev/next buttons at the bottom of the document**

After the sections `</div>`, before the closing `</div>` of the detail block, add:

```tsx
{/* Prev / Next navigation */}
{(prevDoc || nextDoc) && (
    <div className="border-border mt-10 flex items-center justify-between border-t pt-6">
        {prevDoc ? (
            <button
                onClick={() => setSelectedId(prevDoc.id)}
                className="text-muted-foreground hover:text-foreground group flex items-center gap-2 text-sm transition-colors"
            >
                <ChevronLeft className="size-4 transition-transform group-hover:-translate-x-0.5" />
                <div className="text-left">
                    <p className="text-xs text-muted-foreground">Previous</p>
                    <p className="font-medium">{prevDoc.title}</p>
                </div>
            </button>
        ) : <div />}
        {nextDoc ? (
            <button
                onClick={() => setSelectedId(nextDoc.id)}
                className="text-muted-foreground hover:text-foreground group flex items-center gap-2 text-sm transition-colors"
            >
                <div className="text-right">
                    <p className="text-xs text-muted-foreground">Next</p>
                    <p className="font-medium">{nextDoc.title}</p>
                </div>
                <ChevronRight className="size-4 transition-transform group-hover:translate-x-0.5" />
            </button>
        ) : <div />}
    </div>
)}
```

Add `ChevronLeft` and `ChevronRight` to the lucide-react imports.

- [ ] **Step 3: Scroll to top when navigating**

In the `setSelectedId` function, add scroll-to-top:

```typescript
const setSelectedId = (id: string | null) => {
    setSelectedIdState(id);
    navigate({
        to: "/docs",
        search: (prev: Record<string, unknown>) => ({ ...prev, id: id ?? undefined, section: undefined }),
        replace: true
    });
    window.scrollTo({ top: 0, behavior: "smooth" });
};
```

- [ ] **Step 4: Commit**

```bash
git add apps/web/src/features/documents/document-browser.tsx
git commit -m "feat(docs): add previous/next document navigation"
```

---

### Task 3: Sticky TOC as Right Sidebar

**Files:**
- Modify: `apps/web/src/features/documents/document-browser.tsx`

- [ ] **Step 1: Switch to three-column layout when detail is loaded and has 3+ sections**

Change the outer grid from:
```tsx
<div className="grid gap-6 lg:grid-cols-[280px_1fr]">
```
to a dynamic class based on whether the TOC should show:

```typescript
const showToc = detail && detail.chunks.length >= 3;
```

```tsx
<div className={`grid gap-6 ${showToc ? "lg:grid-cols-[280px_1fr_200px]" : "lg:grid-cols-[280px_1fr]"}`}>
```

- [ ] **Step 2: Move the "On this page" box to a sticky right column**

Remove the existing inline "On this page" `<div>` from inside the content area.

Add a new third column after the main content `</div>`:

```tsx
{/* ─── Sticky TOC ─── */}
{showToc && (
    <nav className="hidden lg:block">
        <div className="sticky top-24 space-y-2">
            <p className="text-xs font-semibold uppercase tracking-wider text-muted-foreground">On this page</p>
            <ul className="space-y-1 border-l border-border pl-3">
                {detail.chunks.map(chunk => (
                    <li key={chunk.id}>
                        <a
                            href={`#section-${chunk.id}`}
                            className="text-muted-foreground hover:text-foreground block text-xs leading-relaxed transition-colors"
                        >
                            {chunk.title}
                        </a>
                    </li>
                ))}
            </ul>
        </div>
    </nav>
)}
```

- [ ] **Step 3: Add active section tracking via IntersectionObserver**

Add a `useRef` and `useState` for the active section:

```typescript
const [activeSection, setActiveSection] = useState<string | null>(null);
const contentRef = useRef<HTMLDivElement>(null);
```

Add an IntersectionObserver effect:

```typescript
useEffect(() => {
    if (!detail) return;
    const observer = new IntersectionObserver(
        entries => {
            for (const entry of entries) {
                if (entry.isIntersecting) {
                    setActiveSection(entry.target.id);
                }
            }
        },
        { rootMargin: "-80px 0px -70% 0px", threshold: 0 }
    );
    const sections = document.querySelectorAll("[id^='section-']");
    sections.forEach(s => observer.observe(s));
    return () => observer.disconnect();
}, [detail]);
```

Use `activeSection` to highlight the current TOC item:

```tsx
<a
    href={`#section-${chunk.id}`}
    className={`block text-xs leading-relaxed transition-colors ${
        activeSection === `section-${chunk.id}`
            ? "text-foreground font-medium"
            : "text-muted-foreground hover:text-foreground"
    }`}
>
```

Add `useRef` to the React imports.

- [ ] **Step 4: Commit**

```bash
git add apps/web/src/features/documents/document-browser.tsx
git commit -m "feat(docs): add sticky TOC with active section tracking"
```

---

### Task 4: Keyboard Navigation

**Files:**
- Modify: `apps/web/src/features/documents/document-browser.tsx`

- [ ] **Step 1: Add keyboard event handler**

Add a `useEffect` for keyboard shortcuts:

```typescript
useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
        const target = e.target as HTMLElement;
        if (target.tagName === "INPUT" || target.tagName === "TEXTAREA" || target.isContentEditable) return;

        if (e.key === "/" && !e.metaKey && !e.ctrlKey) {
            e.preventDefault();
            const input = document.querySelector<HTMLInputElement>("[data-docs-search]");
            input?.focus();
            return;
        }

        if (e.key === "Escape") {
            if (isSearching) {
                clearSearch();
                return;
            }
            const input = document.querySelector<HTMLInputElement>("[data-docs-search]");
            input?.blur();
            return;
        }

        if (e.key === "ArrowUp" || e.key === "k") {
            e.preventDefault();
            if (prevDoc) setSelectedId(prevDoc.id);
            return;
        }

        if (e.key === "ArrowDown" || e.key === "j") {
            e.preventDefault();
            if (nextDoc) setSelectedId(nextDoc.id);
            return;
        }
    };

    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
}, [prevDoc, nextDoc, isSearching]);
```

- [ ] **Step 2: Add `data-docs-search` attribute to the search input**

Change the search input to include:
```tsx
<input
    data-docs-search
    type="text"
    ...
```

- [ ] **Step 3: Commit**

```bash
git add apps/web/src/features/documents/document-browser.tsx
git commit -m "feat(docs): add keyboard navigation (j/k, arrows, /, Esc)"
```

---

### Task 5: Breadcrumb

**Files:**
- Modify: `apps/web/src/features/documents/document-browser.tsx`

- [ ] **Step 1: Add breadcrumb above the document title**

In the document header area, before the `<h2>`, add:

```tsx
{/* Breadcrumb */}
<div className="text-muted-foreground mb-2 flex items-center gap-1 text-xs">
    <span>Docs</span>
    {folderFromPath(detail.sourcePath) !== "/" && (
        <>
            <ChevronRight className="size-3" />
            <span>{folderFromPath(detail.sourcePath)}</span>
        </>
    )}
    <ChevronRight className="size-3" />
    <span className="text-foreground font-medium">{detail.title}</span>
</div>
```

- [ ] **Step 2: Commit**

```bash
git add apps/web/src/features/documents/document-browser.tsx
git commit -m "feat(docs): add breadcrumb navigation"
```

---

### Task 6: Mobile Sidebar as Sheet

**Files:**
- Modify: `apps/web/src/features/documents/document-browser.tsx`

- [ ] **Step 1: Add mobile toggle button and Sheet wrapper**

Import the Sheet component:
```typescript
import { Sheet, SheetContent, SheetTitle, SheetTrigger } from "@/components/ui/sheet";
```

Add `Menu` to lucide-react imports.

Add state:
```typescript
const [mobileOpen, setMobileOpen] = useState(false);
```

Wrap the sidebar content in a responsive pattern. The existing `<div className="space-y-3">` sidebar becomes:

```tsx
{/* ─── Sidebar (desktop) ─── */}
<div className="hidden lg:block space-y-3">
    {/* existing search + nav content */}
</div>

{/* ─── Sidebar (mobile) ─── */}
<div className="lg:hidden">
    <Sheet open={mobileOpen} onOpenChange={setMobileOpen}>
        <SheetTrigger asChild>
            <button className="border-input bg-background flex w-full items-center gap-2 rounded-md border px-3 py-2 text-sm">
                <Menu className="size-4" />
                <span className="text-muted-foreground">{detail?.title ?? "Select document..."}</span>
            </button>
        </SheetTrigger>
        <SheetContent side="left" className="w-80 p-4">
            <SheetTitle className="mb-4 text-sm font-semibold">Documents</SheetTitle>
            {/* Same search + nav content as desktop, but clicking a doc also calls setMobileOpen(false) */}
        </SheetContent>
    </Sheet>
</div>
```

To avoid duplicating the sidebar content, extract it into a `SidebarContent` component or a render function:

```typescript
const sidebarContent = (onSelect?: () => void) => (
    <div className="space-y-3">
        {/* Search input */}
        ...
        {/* Search results or folder nav — when a doc is clicked, also call onSelect?.() */}
        ...
    </div>
);
```

Then use: `{sidebarContent()}` for desktop and `{sidebarContent(() => setMobileOpen(false))}` for mobile.

- [ ] **Step 2: Verify the Sheet component exists and check its API**

Read `apps/web/src/components/ui/sheet.tsx` to confirm the import path and prop names (`side`, `open`, `onOpenChange`, `SheetContent`, `SheetTitle`, `SheetTrigger`).

- [ ] **Step 3: Commit**

```bash
git add apps/web/src/features/documents/document-browser.tsx
git commit -m "feat(docs): add mobile sidebar sheet for document navigation"
```

---

### Task 7: Grouped Search Results

**Files:**
- Modify: `apps/web/src/features/documents/document-browser.tsx`

- [ ] **Step 1: Group search results by document**

Add a grouped results memo:

```typescript
const groupedSearchResults = useMemo(() => {
    const map = new Map<string, { doc: { id: string; title: string; sourcePath: string }; results: SearchResult[] }>();
    for (const result of searchResults) {
        const existing = map.get(result.documentId);
        if (existing) {
            existing.results.push(result);
        } else {
            map.set(result.documentId, {
                doc: { id: result.documentId, title: result.documentTitle, sourcePath: result.sourcePath },
                results: [result]
            });
        }
    }
    return Array.from(map.values());
}, [searchResults]);
```

- [ ] **Step 2: Update search results rendering to show groups**

Replace the flat search results list with:

```tsx
{groupedSearchResults.map(group => (
    <div key={group.doc.id} className="mb-3">
        <div className="flex items-center gap-1.5 px-2 py-1">
            <FileText className="text-muted-foreground size-3.5" />
            <span className="text-xs font-semibold">{group.doc.title}</span>
            <Badge variant="secondary" size="sm" className="ml-auto text-[9px]">
                {group.results.length}
            </Badge>
        </div>
        {group.results.map((result, i) => (
            <button
                key={`${result.chunk.id}-${i}`}
                onClick={() => navigateToResult(result)}
                className="hover:bg-muted/50 w-full rounded-md px-3 py-2 text-left transition-colors"
            >
                <p className="text-sm font-medium">{highlightMatches(result.chunk.title, searchQuery)}</p>
                <p className="text-muted-foreground mt-0.5 line-clamp-2 text-xs">
                    {highlightMatches(result.snippet, searchQuery)}
                </p>
            </button>
        ))}
    </div>
))}
```

- [ ] **Step 3: Commit**

```bash
git add apps/web/src/features/documents/document-browser.tsx
git commit -m "feat(docs): group search results by document"
```

---

### Task 8: Copy Section Link

**Files:**
- Modify: `apps/web/src/features/documents/document-browser.tsx`

- [ ] **Step 1: Add copy link button next to section headings**

Add `LinkIcon` (as `LinkIconLucide`) to lucide-react imports (rename to avoid collision with TanStack `Link`):

```typescript
import { ..., Link as LinkIconLucide } from "lucide-react";
```

Actually, use `Link2` from lucide-react to avoid naming conflict:

```typescript
import { ..., Link2 } from "lucide-react";
```

Update the section heading area — add a copy-link button next to the edit button:

```tsx
<div className="group mb-3 flex items-center gap-2">
    <h3 className="text-lg font-semibold">{chunk.title}</h3>
    <button
        onClick={() => {
            const url = `${window.location.origin}/docs?id=${detail.id}&section=${chunk.id}`;
            navigator.clipboard.writeText(url);
        }}
        className="text-muted-foreground hover:text-foreground opacity-0 transition-opacity group-hover:opacity-100"
        title="Copy link to section"
    >
        <Link2 className="size-3.5" />
    </button>
    <Link
        to="/chunks/$chunkId/edit"
        params={{ chunkId: chunk.id }}
        className="text-muted-foreground hover:text-foreground opacity-0 transition-opacity group-hover:opacity-100"
        title="Edit this section"
    >
        <Pencil className="size-3.5" />
    </Link>
</div>
```

- [ ] **Step 2: Add a brief toast or visual feedback on copy**

Add a `copiedId` state:

```typescript
const [copiedId, setCopiedId] = useState<string | null>(null);
```

Update the onClick:

```typescript
onClick={() => {
    const url = `${window.location.origin}/docs?id=${detail.id}&section=${chunk.id}`;
    navigator.clipboard.writeText(url);
    setCopiedId(chunk.id);
    setTimeout(() => setCopiedId(null), 1500);
}}
```

Show a check icon when copied:

```tsx
<button ...>
    {copiedId === chunk.id ? (
        <Check className="size-3.5 text-green-500" />
    ) : (
        <Link2 className="size-3.5" />
    )}
</button>
```

Add `Check` to lucide-react imports.

- [ ] **Step 3: Commit**

```bash
git add apps/web/src/features/documents/document-browser.tsx
git commit -m "feat(docs): add copy section link button"
```

---

### Task 9: Reading Progress Indicator

**Files:**
- Modify: `apps/web/src/features/documents/document-browser.tsx`

- [ ] **Step 1: Add scroll progress state**

```typescript
const [readProgress, setReadProgress] = useState(0);
```

Add a scroll listener effect:

```typescript
useEffect(() => {
    if (!detail) return;
    const handleScroll = () => {
        const scrollTop = window.scrollY;
        const docHeight = document.documentElement.scrollHeight - window.innerHeight;
        if (docHeight <= 0) { setReadProgress(100); return; }
        setReadProgress(Math.min(100, Math.round((scrollTop / docHeight) * 100)));
    };
    window.addEventListener("scroll", handleScroll, { passive: true });
    handleScroll();
    return () => window.removeEventListener("scroll", handleScroll);
}, [detail]);
```

- [ ] **Step 2: Render progress bar at top of content area**

At the very top of the main content column `<div className="min-w-0">`, add:

```tsx
{selectedId && detail && (
    <div className="bg-muted mb-4 h-0.5 w-full overflow-hidden rounded-full">
        <div
            className="bg-foreground/30 h-full transition-all duration-150"
            style={{ width: `${readProgress}%` }}
        />
    </div>
)}
```

- [ ] **Step 3: Commit**

```bash
git add apps/web/src/features/documents/document-browser.tsx
git commit -m "feat(docs): add reading progress indicator"
```

---

### Task 10: Last Updated Indicator

**Files:**
- Modify: `apps/web/src/features/documents/document-browser.tsx`

- [ ] **Step 1: Show updatedAt in the document header**

The `DocumentDetail` type doesn't include `updatedAt` yet. We can get it from the list data. Add to the header after the sourcePath:

```typescript
const selectedListItem = documents.find(d => d.id === selectedId);
```

Then in the header:

```tsx
{selectedListItem?.updatedAt && (
    <p className="text-muted-foreground mt-1 text-xs">
        Last updated {new Date(selectedListItem.updatedAt).toLocaleDateString(undefined, { year: "numeric", month: "short", day: "numeric" })}
    </p>
)}
```

- [ ] **Step 2: Commit**

```bash
git add apps/web/src/features/documents/document-browser.tsx
git commit -m "feat(docs): show last updated date in document header"
```

---

### Task 11: Document Ordering in Sidebar

**Files:**
- Modify: `apps/web/src/features/documents/document-browser.tsx`

- [ ] **Step 1: Sort documents by sourcePath within each folder**

The documents are already grouped by folder. Within each folder group, sort alphabetically by title:

In the `grouped` memo, after `map.set`, sort each group:

```typescript
const grouped = useMemo(() => {
    const map = new Map<string, DocumentListItem[]>();
    for (const doc of sidebarFiltered) {
        const folder = folderFromPath(doc.sourcePath);
        const list = map.get(folder) ?? [];
        list.push(doc);
        map.set(folder, list);
    }
    // Sort docs within each folder by title
    for (const [, docs] of map) {
        docs.sort((a, b) => a.title.localeCompare(b.title));
    }
    return Array.from(map.entries()).sort(([a], [b]) => a.localeCompare(b));
}, [sidebarFiltered]);
```

- [ ] **Step 2: Commit**

```bash
git add apps/web/src/features/documents/document-browser.tsx
git commit -m "feat(docs): sort documents alphabetically within folders"
```

---

### Task 12: Verification

- [ ] **Step 1: Test all features**

Open `http://localhost:3001/docs` and verify:

1. First document auto-selected on load
2. URL updates when selecting docs (`?id=...`)
3. Refreshing preserves selection
4. Sharing `/docs?id=abc&section=xyz` loads the right doc and scrolls
5. Prev/Next buttons at bottom work, scroll to top
6. Sticky TOC on right side tracks active section
7. `j`/`k` or arrow keys switch documents
8. `/` focuses search, `Esc` clears it
9. Breadcrumb shows `Docs > folder > title`
10. Mobile: sheet opens with doc list, closes on selection
11. Search results grouped by document
12. Copy link button shows check icon, URL is correct
13. Reading progress bar moves on scroll
14. Last updated date shows in header
15. Documents sorted alphabetically in sidebar

- [ ] **Step 2: Final commit if any fixes needed**

```bash
git add -A && git commit -m "fix: address issues found during docs usability verification"
```
