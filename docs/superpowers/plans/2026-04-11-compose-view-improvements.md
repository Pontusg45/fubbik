# Compose View Improvements Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Improve the `/compose` composite view with proper markdown rendering, table of contents, navigation, export options, filtering controls, and UX polish. Excludes AI integration.

**Architecture:** All changes are to `apps/web/src/routes/compose.tsx` (one file). Reuses the existing `MarkdownRenderer` component and search API. No backend changes needed except possibly reusing existing export endpoints.

**Tech Stack:** React, TanStack Router, Tailwind CSS, react-markdown (already a dep), lucide-react

---

## File Structure

| File | Action | Responsibility |
|------|--------|---------------|
| `apps/web/src/routes/compose.tsx` | Modify | All compose view enhancements |
| `apps/web/src/features/compose/` | Create | Extract components if file grows too large |

All changes are incremental modifications to the existing compose page.

---

### Task 1: Render Markdown Properly

**Files:**
- Modify: `apps/web/src/routes/compose.tsx`

**Context:** Chunk content is currently rendered as `whitespace-pre-wrap` plain text. Replace with the existing `MarkdownRenderer` component at `apps/web/src/components/markdown-renderer.tsx` which supports GFM, syntax highlighting (rehype-highlight), and mermaid diagrams.

- [ ] **Step 1: Import the MarkdownRenderer**

In `apps/web/src/routes/compose.tsx`, add at the top with other imports:
```typescript
import { MarkdownRenderer } from "@/components/markdown-renderer";
```

- [ ] **Step 2: Replace the content div**

Find this block in the article map:
```tsx
{chunk.content && (
    <div className="prose prose-sm dark:prose-invert max-w-none whitespace-pre-wrap text-sm leading-relaxed">
        {chunk.content}
    </div>
)}
```

Replace with:
```tsx
{chunk.content && (
    <div className="prose prose-sm dark:prose-invert max-w-none">
        <MarkdownRenderer>{chunk.content}</MarkdownRenderer>
    </div>
)}
```

Also update the rationale block the same way:
```tsx
{chunk.rationale && (
    <div className="mt-4 rounded-md border-l-2 border-amber-500/40 bg-amber-500/5 px-4 py-3">
        <div className="text-xs font-semibold uppercase tracking-wider text-amber-600 dark:text-amber-400 mb-1">
            Rationale
        </div>
        <div className="prose prose-sm dark:prose-invert max-w-none text-muted-foreground">
            <MarkdownRenderer>{chunk.rationale}</MarkdownRenderer>
        </div>
    </div>
)}
```

- [ ] **Step 3: Verify**

Run: `pnpm --filter web run check-types 2>&1 | grep compose`

Expected: No errors.

- [ ] **Step 4: Commit**

```bash
git add apps/web/src/routes/compose.tsx
git commit -m "feat(compose): render chunk content as markdown with highlighting"
```

---

### Task 2: Table of Contents Sidebar

**Files:**
- Modify: `apps/web/src/routes/compose.tsx`

**Context:** Add a sticky left sidebar listing all chunks in the view, with click-to-scroll anchors. Essential for longer composite views.

- [ ] **Step 1: Add anchor IDs to each article**

In the article map, add an `id` to each article element:
```tsx
<article key={chunk.id} id={`chunk-${chunk.id}`} className={i > 0 ? "border-t pt-12" : ""}>
```

- [ ] **Step 2: Restructure the page layout to have a sidebar**

Wrap the current content in a flex layout with a sticky sidebar on the left. Update the container div:

```tsx
return (
    <div className="container mx-auto max-w-6xl px-4 py-8 print:py-4">
        {/* Header (unchanged) */}
        ...

        {/* Two-column layout: ToC + content */}
        <div className="flex gap-8">
            {/* ToC sidebar */}
            <aside className="hidden lg:block w-56 shrink-0 print:hidden">
                <div className="sticky top-8">
                    <div className="text-xs font-semibold uppercase tracking-wider text-muted-foreground mb-3">
                        Contents
                    </div>
                    {chunks.length > 0 ? (
                        <nav className="space-y-1 max-h-[calc(100vh-8rem)] overflow-y-auto">
                            {chunks.map((chunk) => (
                                <a
                                    key={chunk.id}
                                    href={`#chunk-${chunk.id}`}
                                    className="block truncate rounded px-2 py-1 text-xs text-muted-foreground hover:bg-muted hover:text-foreground transition-colors"
                                    title={chunk.title}
                                >
                                    {chunk.title}
                                </a>
                            ))}
                        </nav>
                    ) : (
                        <p className="text-xs text-muted-foreground">No chunks</p>
                    )}
                </div>
            </aside>

            {/* Main content */}
            <div className="flex-1 min-w-0">
                {/* Title and filter summary */}
                ...
                {/* Loading / error / empty states */}
                ...
                {/* Article list */}
                ...
            </div>
        </div>
    </div>
);
```

Move the title/filter summary, loading/error/empty states, and article list INTO the main content div. The header with back/copy/print buttons stays at the top above the flex layout.

- [ ] **Step 3: Smooth scroll behavior**

Add to the main div or the `html` element via a CSS class:
```tsx
<div className="container mx-auto max-w-6xl px-4 py-8 print:py-4 scroll-smooth">
```

Actually, `scroll-smooth` on the container isn't enough — anchor navigation uses document scroll. Add it to the html element via a `useEffect`:
```tsx
useEffect(() => {
    document.documentElement.classList.add("scroll-smooth");
    return () => document.documentElement.classList.remove("scroll-smooth");
}, []);
```

- [ ] **Step 4: Verify**

Run: `pnpm --filter web run check-types 2>&1 | grep compose`

- [ ] **Step 5: Commit**

```bash
git add apps/web/src/routes/compose.tsx
git commit -m "feat(compose): add table of contents sidebar with anchor links"
```

---

### Task 3: Keyboard Navigation (j/k shortcuts)

**Files:**
- Modify: `apps/web/src/routes/compose.tsx`

**Context:** Add `j` / `k` keyboard shortcuts to jump between chunks, like Gmail or Vim.

- [ ] **Step 1: Add keyboard handler**

Add a `useEffect` that listens for keydown events and scrolls to the next/previous chunk:

```typescript
useEffect(() => {
    function handleKey(e: KeyboardEvent) {
        // Skip if user is typing in an input
        const tag = document.activeElement?.tagName;
        if (tag === "INPUT" || tag === "TEXTAREA" || (document.activeElement as HTMLElement)?.isContentEditable) {
            return;
        }

        if (e.key !== "j" && e.key !== "k") return;
        if (chunks.length === 0) return;

        // Find which chunk is currently in view
        const articles = chunks.map(c => document.getElementById(`chunk-${c.id}`)).filter(Boolean) as HTMLElement[];
        const viewportTop = window.scrollY + 100; // 100px offset for header
        let currentIdx = 0;
        for (let i = 0; i < articles.length; i++) {
            if (articles[i].offsetTop <= viewportTop) {
                currentIdx = i;
            }
        }

        let targetIdx = currentIdx;
        if (e.key === "j") targetIdx = Math.min(chunks.length - 1, currentIdx + 1);
        if (e.key === "k") targetIdx = Math.max(0, currentIdx - 1);

        const target = articles[targetIdx];
        if (target) {
            target.scrollIntoView({ behavior: "smooth", block: "start" });
        }
    }

    window.addEventListener("keydown", handleKey);
    return () => window.removeEventListener("keydown", handleKey);
}, [chunks]);
```

- [ ] **Step 2: Add a hint in the header**

Add a small text hint next to the action buttons:
```tsx
<span className="hidden lg:inline text-[10px] text-muted-foreground/60 font-mono mr-2">
    press <kbd className="rounded border bg-muted px-1 py-0.5">j</kbd> / <kbd className="rounded border bg-muted px-1 py-0.5">k</kbd> to navigate
</span>
```

- [ ] **Step 3: Verify and commit**

```bash
git add apps/web/src/routes/compose.tsx
git commit -m "feat(compose): add j/k keyboard shortcuts to navigate between chunks"
```

---

### Task 4: Reading Progress Bar

**Files:**
- Modify: `apps/web/src/routes/compose.tsx`

**Context:** Small progress indicator at the top of the viewport showing scroll position through the document.

- [ ] **Step 1: Add scroll progress state**

Add state and effect:
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
}, [chunks]);
```

- [ ] **Step 2: Render the progress bar**

At the top of the component return, before the container div:
```tsx
<div className="fixed top-0 left-0 right-0 z-50 h-0.5 bg-transparent print:hidden">
    <div
        className="h-full bg-primary transition-[width] duration-100 ease-out"
        style={{ width: `${scrollProgress}%` }}
    />
</div>
```

- [ ] **Step 3: Verify and commit**

```bash
git add apps/web/src/routes/compose.tsx
git commit -m "feat(compose): add reading progress bar at top of viewport"
```

---

### Task 5: Sort and Group Controls

**Files:**
- Modify: `apps/web/src/routes/compose.tsx`

**Context:** Add dropdown controls for sorting (title, updated, created, type, connections) and grouping (none, type, tags, codebase).

- [ ] **Step 1: Add sort/group state and URL params**

Update the `validateSearch` to include sort and group:
```typescript
validateSearch: (search: Record<string, unknown>) => ({
    q: (search.q as string) || undefined,
    sort: (search.sort as string) || "updated",
    group: (search.group as string) || "none",
}),
```

In the component:
```typescript
const { q, sort, group } = Route.useSearch();

function updateParam(key: string, value: string) {
    void navigate({
        to: "/compose",
        search: { q, sort, group, [key]: value } as any,
        replace: true,
    });
}
```

- [ ] **Step 2: Add sort controls to the header**

Add a dropdown/select below the filter summary (or next to the copy button):

```tsx
<div className="flex items-center gap-3 mt-3 text-xs">
    <label className="flex items-center gap-1.5 text-muted-foreground">
        Sort:
        <select
            value={sort}
            onChange={e => updateParam("sort", e.target.value)}
            className="bg-muted/50 rounded px-2 py-1 border text-xs"
        >
            <option value="updated">Recently updated</option>
            <option value="newest">Newest</option>
            <option value="oldest">Oldest</option>
            <option value="title">Title A-Z</option>
            <option value="type">By type</option>
            <option value="connections">Most connected</option>
        </select>
    </label>
    <label className="flex items-center gap-1.5 text-muted-foreground">
        Group by:
        <select
            value={group}
            onChange={e => updateParam("group", e.target.value)}
            className="bg-muted/50 rounded px-2 py-1 border text-xs"
        >
            <option value="none">None</option>
            <option value="type">Type</option>
            <option value="tag">Tag</option>
        </select>
    </label>
</div>
```

- [ ] **Step 3: Apply client-side sorting**

After fetching chunks, apply sort before rendering:

```typescript
const sortedChunks = useMemo(() => {
    const copy = [...chunks];
    switch (sort) {
        case "title":
            return copy.sort((a, b) => a.title.localeCompare(b.title));
        case "type":
            return copy.sort((a, b) => a.type.localeCompare(b.type));
        case "connections":
            return copy.sort((a, b) => (b.connectionCount ?? 0) - (a.connectionCount ?? 0));
        case "newest":
        case "oldest":
            // If we have updatedAt/createdAt, use it. Otherwise fall back.
            return copy;
        default:
            return copy;
    }
}, [chunks, sort]);
```

Add `useMemo` to imports: `import { useEffect, useMemo, useState } from "react";`

- [ ] **Step 4: Apply client-side grouping**

Compute groups and render sections if group !== "none":

```typescript
const groupedChunks = useMemo(() => {
    if (group === "none") return null;
    const groups = new Map<string, ComposedChunk[]>();
    for (const chunk of sortedChunks) {
        let key: string;
        if (group === "type") {
            key = chunk.type;
        } else if (group === "tag") {
            key = chunk.tags && chunk.tags.length > 0 ? chunk.tags[0] : "untagged";
        } else {
            key = "all";
        }
        const existing = groups.get(key) ?? [];
        existing.push(chunk);
        groups.set(key, existing);
    }
    return Array.from(groups.entries()).sort(([a], [b]) => a.localeCompare(b));
}, [sortedChunks, group]);
```

In the render, if `groupedChunks` is not null, render with section headings:

```tsx
{!loading && chunks.length > 0 && groupedChunks && (
    <div className="space-y-12">
        {groupedChunks.map(([groupKey, groupChunks]) => (
            <section key={groupKey}>
                <h2 className="text-xs font-semibold uppercase tracking-wider text-muted-foreground mb-6 pb-2 border-b">
                    {groupKey} <span className="text-muted-foreground/60">({groupChunks.length})</span>
                </h2>
                <div className="space-y-12">
                    {groupChunks.map((chunk, i) => (
                        <article key={chunk.id} id={`chunk-${chunk.id}`} className={i > 0 ? "border-t pt-12" : ""}>
                            {/* same article content */}
                        </article>
                    ))}
                </div>
            </section>
        ))}
    </div>
)}

{!loading && chunks.length > 0 && !groupedChunks && (
    <div className="space-y-12">
        {sortedChunks.map((chunk, i) => (
            <article key={chunk.id} id={`chunk-${chunk.id}`} className={i > 0 ? "border-t pt-12" : ""}>
                {/* same article content */}
            </article>
        ))}
    </div>
)}
```

Note: This duplicates the article markup. Extract it into an inline `renderChunk` function to avoid duplication:

```typescript
function renderChunk(chunk: ComposedChunk, i: number, showBorder: boolean) {
    return (
        <article key={chunk.id} id={`chunk-${chunk.id}`} className={showBorder && i > 0 ? "border-t pt-12" : ""}>
            {/* existing article content */}
        </article>
    );
}
```

Then use `renderChunk(chunk, i, true)` in both branches.

- [ ] **Step 5: Verify and commit**

```bash
git add apps/web/src/routes/compose.tsx
git commit -m "feat(compose): add sort and group-by controls with URL persistence"
```

---

### Task 6: Download as Markdown File

**Files:**
- Modify: `apps/web/src/routes/compose.tsx`

**Context:** Add a "Download .md" button next to the copy button. Creates a Blob from the same markdown string used for copy, then triggers a download.

- [ ] **Step 1: Add the download function**

Add a helper function that builds the markdown (reuse from copy handler):

```typescript
function buildMarkdown(): string {
    const filterLine = q ? `<!-- Filters: ${q} -->\n\n` : "";
    const body = sortedChunks
        .map(c => {
            const parts = [`## ${c.title}`];
            if (c.summary) parts.push(`*${c.summary}*`);
            if (c.content) parts.push(c.content);
            if (c.rationale) parts.push(`**Rationale:** ${c.rationale}`);
            return parts.join("\n\n");
        })
        .join("\n\n---\n\n");
    return `# Composite View\n\n${filterLine}${body}`;
}
```

- [ ] **Step 2: Refactor handleCopy to use buildMarkdown**

```typescript
function handleCopy() {
    void navigator.clipboard.writeText(buildMarkdown());
    setCopied(true);
    setTimeout(() => setCopied(false), 2000);
}
```

- [ ] **Step 3: Add download handler**

```typescript
function handleDownload() {
    const md = buildMarkdown();
    const blob = new Blob([md], { type: "text/markdown" });
    const url = URL.createObjectURL(blob);
    const a = document.createElement("a");
    a.href = url;
    a.download = `composite-${new Date().toISOString().slice(0, 10)}.md`;
    a.click();
    URL.revokeObjectURL(url);
}
```

- [ ] **Step 4: Add the download button**

Import `Download` from lucide-react and add the button next to Copy:
```tsx
<Button variant="outline" size="sm" onClick={handleDownload} className="gap-1.5">
    <Download className="size-3.5" />
    Download
</Button>
```

- [ ] **Step 5: Verify and commit**

```bash
git add apps/web/src/routes/compose.tsx
git commit -m "feat(compose): add download as markdown file button"
```

---

### Task 7: Print-Optimized CSS

**Files:**
- Modify: `apps/web/src/routes/compose.tsx`

**Context:** Polish the print output so the composite view looks good when printed or saved as PDF from the browser. Hide UI chrome, use serif fonts, add page breaks between chunks.

- [ ] **Step 1: Add print-specific Tailwind classes**

Many elements in the page already have `print:hidden` where needed. Review and add:

1. Hide the header action buttons (already done with `print:hidden` on header)
2. Hide the ToC sidebar (already done)
3. Hide the reading progress bar (already done)
4. Hide the sort/group controls — add `print:hidden` to the controls div
5. Add page breaks between articles — on the article element:
```tsx
<article
    key={chunk.id}
    id={`chunk-${chunk.id}`}
    className={`${showBorder && i > 0 ? "border-t pt-12" : ""} print:break-before-page print:pt-0 print:border-0`}
>
```

For the first article, don't force a page break:
```tsx
className={`${i > 0 ? "border-t pt-12" : ""} ${i > 0 ? "print:break-before-page" : ""} print:pt-0 print:border-0`}
```

- [ ] **Step 2: Add a print stylesheet tweak**

In the component's JSX, add a small `<style>` block for print-specific tweaks that Tailwind can't express easily:

```tsx
<style>{`
    @media print {
        body { font-family: Georgia, 'Times New Roman', serif; }
        .prose { max-width: none !important; }
        a { color: inherit; text-decoration: none; }
        h1, h2, h3 { page-break-after: avoid; }
        article { page-break-inside: avoid; }
    }
`}</style>
```

Place it at the top of the return, inside the outermost div.

- [ ] **Step 3: Test by pressing Cmd+P / Ctrl+P in browser**

Verify:
1. No UI chrome visible
2. Each chunk starts on a new page
3. Serif font for readability
4. Headings don't get orphaned at page bottoms

- [ ] **Step 4: Commit**

```bash
git add apps/web/src/routes/compose.tsx
git commit -m "feat(compose): add print-optimized styles with page breaks and serif font"
```

---

### Task 8: Inline Filter Pills (editable from compose view)

**Files:**
- Modify: `apps/web/src/routes/compose.tsx`

**Context:** Show the current filters as removable pills. Clicking × removes that filter and re-queries (via URL update). Users can refine without going back to search.

- [ ] **Step 1: Parse filters into a pill-friendly format**

You already have `parseSimpleQuery(q)`. The result is a `clauses` array. Use it to render pills.

- [ ] **Step 2: Add a removeFilter function**

```typescript
function removeFilter(index: number) {
    const clauses = parseSimpleQuery(q ?? "");
    const remaining = clauses.filter((_, i) => i !== index);
    const newQ = remaining
        .map(c => `${c.negate ? "NOT " : ""}${c.field}:${c.value}`)
        .join(" ");
    void navigate({
        to: "/compose",
        search: { q: newQ || undefined, sort, group } as any,
        replace: true,
    });
}
```

- [ ] **Step 3: Replace the filter summary with editable pills**

Replace this block:
```tsx
{q && (
    <div className="flex items-center gap-2">
        <span className="text-xs text-muted-foreground">Filters:</span>
        <code className="bg-muted/60 rounded px-2 py-0.5 font-mono text-xs">{q}</code>
    </div>
)}
```

With:
```tsx
{q && (
    <div className="flex flex-wrap items-center gap-2">
        <span className="text-xs text-muted-foreground">Filters:</span>
        {parseSimpleQuery(q).map((clause, idx) => (
            <span
                key={idx}
                className="inline-flex items-center gap-1 rounded border border-slate-500/30 bg-slate-500/15 text-slate-400 px-2 py-0.5 text-xs"
            >
                {clause.negate && <span className="font-semibold">NOT</span>}
                <span className="font-semibold">{clause.field}</span>
                <span className="text-muted-foreground">is</span>
                <span>{clause.value}</span>
                <button
                    type="button"
                    onClick={() => removeFilter(idx)}
                    className="opacity-50 hover:opacity-100 transition-opacity"
                    aria-label={`Remove ${clause.field} filter`}
                >
                    ×
                </button>
            </span>
        ))}
    </div>
)}
```

- [ ] **Step 4: Verify and commit**

```bash
git add apps/web/src/routes/compose.tsx
git commit -m "feat(compose): add removable filter pills for inline filter editing"
```

---

### Task 9: Chunk Count Limit

**Files:**
- Modify: `apps/web/src/routes/compose.tsx`

**Context:** Add a limit control so users can cap how many chunks are shown (10, 25, 50, 100, all). Default 50.

- [ ] **Step 1: Add limit to URL params**

Update `validateSearch`:
```typescript
validateSearch: (search: Record<string, unknown>) => ({
    q: (search.q as string) || undefined,
    sort: (search.sort as string) || "updated",
    group: (search.group as string) || "none",
    limit: (search.limit as string) || "50",
}),
```

Use it in the search mutation:
```typescript
const result = await searchMutation.mutateAsync({
    clauses,
    limit: limit === "all" ? 500 : Number(limit),
});
```

Update the mutation function to accept params:
```typescript
const searchMutation = useMutation({
    mutationFn: async ({ clauses, limit }: { clauses: any[]; limit: number }) =>
        unwrapEden(
            await api.api.search.query.post({
                clauses,
                join: "and",
                sort: "updated",
                limit,
                offset: 0,
                ...(codebaseId ? { codebaseId } : {}),
            } as any),
        ),
});
```

- [ ] **Step 2: Add limit control next to sort/group**

```tsx
<label className="flex items-center gap-1.5 text-muted-foreground">
    Limit:
    <select
        value={limit}
        onChange={e => updateParam("limit", e.target.value)}
        className="bg-muted/50 rounded px-2 py-1 border text-xs"
    >
        <option value="10">10</option>
        <option value="25">25</option>
        <option value="50">50</option>
        <option value="100">100</option>
        <option value="all">All</option>
    </select>
</label>
```

- [ ] **Step 3: Verify and commit**

```bash
git add apps/web/src/routes/compose.tsx
git commit -m "feat(compose): add chunk count limit control (10/25/50/100/all)"
```

---

### Task 10: Share Button and Empty-State CTA

**Files:**
- Modify: `apps/web/src/routes/compose.tsx`

**Context:** Small polish items. Add a share button that copies the current URL, and improve the empty state with a button to go back to search.

- [ ] **Step 1: Add share button**

Import `Share2` from lucide-react. Add next to the other action buttons:

```tsx
const [shared, setShared] = useState(false);

function handleShare() {
    void navigator.clipboard.writeText(window.location.href);
    setShared(true);
    setTimeout(() => setShared(false), 2000);
}

// In the header actions:
<Button variant="outline" size="sm" onClick={handleShare} className="gap-1.5">
    {shared ? <Check className="size-3.5" /> : <Share2 className="size-3.5" />}
    {shared ? "Copied" : "Share"}
</Button>
```

- [ ] **Step 2: Improve empty state**

Replace:
```tsx
{!loading && !error && chunks.length === 0 && (
    <div className="py-16 text-center text-muted-foreground">
        No chunks match the current filter.
    </div>
)}
```

With:
```tsx
{!loading && !error && chunks.length === 0 && (
    <div className="py-16 flex flex-col items-center gap-4">
        <div className="text-center">
            <p className="text-muted-foreground mb-2">No chunks match the current filter.</p>
            <p className="text-muted-foreground/70 text-xs">Try broadening your filters.</p>
        </div>
        <Button
            variant="outline"
            size="sm"
            onClick={() => void navigate({ to: "/search", search: { q } as any })}
            className="gap-1.5"
        >
            <ArrowLeft className="size-3.5" />
            Back to search
        </Button>
    </div>
)}
```

- [ ] **Step 3: Verify and commit**

```bash
git add apps/web/src/routes/compose.tsx
git commit -m "feat(compose): add share button and improved empty-state CTA"
```

---

### Task 11: Final Verification

**Files:** (none — verification only)

- [ ] **Step 1: Run full type check**

Run: `pnpm --filter web run check-types 2>&1 | grep compose`

Expected: No errors.

- [ ] **Step 2: Run build**

Run: `pnpm build --filter=web`

Expected: Build succeeds.

- [ ] **Step 3: Manual smoke test**

Start dev server and test:
1. Navigate to `/search`, add filters, click "View as document"
2. Verify ToC sidebar shows all chunks
3. Click a ToC entry — smooth scroll works
4. Press `j` and `k` — navigation between chunks works
5. Scroll — progress bar updates
6. Change sort dropdown — order changes
7. Change group dropdown — grouping appears
8. Click × on a filter pill — removes that filter and re-queries
9. Change limit — chunk count updates
10. Click Download — .md file downloads
11. Click Share — URL copies to clipboard
12. Print (Cmd+P) — clean print layout
13. Empty state — navigate to a filter that matches nothing, verify CTA appears
14. Markdown in chunk content renders with headings, code blocks, lists

If all pass, the task is complete.

- [ ] **Step 4: Final commit (if any fixes needed)**

If any smoke tests fail, fix and commit. Otherwise, nothing to do.
