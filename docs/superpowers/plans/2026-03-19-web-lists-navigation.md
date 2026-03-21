# Web UI Lists & Navigation Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Improve list navigation with infinite scroll, keyboard shortcut discoverability, requirements count badges, settings page polish, and activity filtering.

**Architecture:** All changes in `apps/web/src/`. Mostly isolated route-level changes. Infinite scroll requires modifying the TanStack Query pattern from paginated to infinite. Settings polish is form-only changes.

**Tech Stack:** React, TanStack Router, TanStack Query (`useInfiniteQuery`), shadcn-ui, Tailwind CSS

**Codebase notes:**
- Chunk list uses `limit = 20` at `chunks.index.tsx:100` with Previous/Next pagination at lines 1160-1172
- Activity page fetches 50 items with no pagination UI (`activity.tsx:92`)
- Requirements sidebar filters at `features/requirements/sidebar-filters.tsx` use checkbox groups with no count badges
- Settings page at `routes/settings.tsx` has 3 tabs with debounced auto-save
- Keyboard shortcut hint: only `?` dialog exists, no inline hints

---

## File Structure

### New files to create:
- `apps/web/src/hooks/use-intersection-observer.ts` — Reusable hook for infinite scroll trigger
- `apps/web/src/features/nav/shortcut-hint.tsx` — Dismissable inline shortcut hint

### Files to modify:
- `apps/web/src/routes/chunks.index.tsx` — Replace pagination with "Load more" / infinite scroll
- `apps/web/src/routes/activity.tsx` — Add filtering and pagination
- `apps/web/src/features/requirements/sidebar-filters.tsx` — Add count badges
- `apps/web/src/routes/requirements.tsx` — Pass counts to sidebar
- `apps/web/src/routes/settings.tsx` — Polish form inputs
- `apps/web/src/features/nav/keyboard-shortcuts.tsx` — Export shortcut data for hint

---

## Task 1: Load More / Infinite Scroll for Chunks

Replace Previous/Next pagination with a "Load more" button and intersection observer.

**Files:**
- Create: `apps/web/src/hooks/use-intersection-observer.ts`
- Modify: `apps/web/src/routes/chunks.index.tsx`

- [ ] **Step 1: Create useIntersectionObserver hook**

```tsx
// apps/web/src/hooks/use-intersection-observer.ts
import { useEffect, useRef } from "react";

export function useIntersectionObserver(callback: () => void, enabled: boolean) {
    const ref = useRef<HTMLDivElement>(null);

    useEffect(() => {
        if (!enabled || !ref.current) return;
        const observer = new IntersectionObserver(
            ([entry]) => { if (entry?.isIntersecting) callback(); },
            { rootMargin: "200px" }
        );
        observer.observe(ref.current);
        return () => observer.disconnect();
    }, [callback, enabled]);

    return ref;
}
```

- [ ] **Step 2: Read chunks.index.tsx pagination logic**

Understand the current `page` search param, `chunksQuery` using `useQuery`, and the pagination UI at lines 1160-1172.

- [ ] **Step 3: Switch to useInfiniteQuery**

Replace the current paginated query with `useInfiniteQuery`. **Important:** The API uses `offset`-based pagination (not `page`), so pass `offset: String((pageParam - 1) * limit)`:
```tsx
const chunksQuery = useInfiniteQuery({
    queryKey: ["chunks", { ...searchParams }],
    queryFn: async ({ pageParam = 1 }) => {
        return unwrapEden(await api.api.chunks.get({
            query: { ...queryParams, offset: String((pageParam - 1) * limit), limit: String(limit) }
        }));
    },
    getNextPageParam: (lastPage, allPages) => {
        const loaded = allPages.reduce((acc, p) => acc + (p?.chunks?.length ?? 0), 0);
        return loaded < (lastPage?.total ?? 0) ? allPages.length + 1 : undefined;
    },
    initialPageParam: 1,
});

const allChunks = chunksQuery.data?.pages.flatMap(p => p?.chunks ?? []) ?? [];
const total = chunksQuery.data?.pages[0]?.total ?? 0;

// Note: client-side size filters (processedChunks) are applied AFTER fetching.
// The total from the API reflects unfiltered count. This is acceptable — load-more
// fetches more server data, and the client filter re-applies.
```

- [ ] **Step 4: Replace pagination UI with load-more trigger**

Replace the Previous/Next buttons (lines 1160-1172) with:
```tsx
{chunksQuery.hasNextPage && (
    <div ref={loadMoreRef} className="mt-4 flex justify-center">
        <Button
            variant="outline"
            size="sm"
            onClick={() => chunksQuery.fetchNextPage()}
            disabled={chunksQuery.isFetchingNextPage}
        >
            {chunksQuery.isFetchingNextPage ? "Loading..." : "Load more"}
        </Button>
    </div>
)}
```

Wire up the intersection observer:
```tsx
const loadMoreRef = useIntersectionObserver(
    () => chunksQuery.fetchNextPage(),
    chunksQuery.hasNextPage && !chunksQuery.isFetchingNextPage
);
```

- [ ] **Step 5: Update total/page references and route schema**

Remove `page` from the route's `validateSearch` schema definition (it's a TanStack Router schema in the same file). Update any references to `page` in the component to use the flattened `allChunks` array. Keep the `total` display from the first page's total. Also remove the `page` param from `updateSearch` calls.

- [ ] **Step 6: Commit**

```bash
git add apps/web/src/hooks/use-intersection-observer.ts apps/web/src/routes/chunks.index.tsx
git commit -m "feat(web): replace pagination with infinite scroll on chunk list"
```

---

## Task 2: Keyboard Shortcut Discoverability

Show a subtle, dismissable hint on the chunks list page.

**Files:**
- Create: `apps/web/src/features/nav/shortcut-hint.tsx`
- Modify: `apps/web/src/routes/chunks.index.tsx`

- [ ] **Step 1: Create ShortcutHint component**

```tsx
// apps/web/src/features/nav/shortcut-hint.tsx
import { useState } from "react";
import { X } from "lucide-react";

const DISMISSED_KEY = "fubbik-shortcut-hint-dismissed";

export function ShortcutHint() {
    const [dismissed, setDismissed] = useState(() =>
        localStorage.getItem(DISMISSED_KEY) === "true"
    );

    if (dismissed) return null;

    const dismiss = () => {
        setDismissed(true);
        localStorage.setItem(DISMISSED_KEY, "true");
    };

    return (
        <div className="text-muted-foreground flex items-center gap-2 text-xs">
            <span>
                Press <kbd className="bg-muted rounded border px-1 font-mono text-[10px]">/</kbd> to search,{" "}
                <kbd className="bg-muted rounded border px-1 font-mono text-[10px]">j</kbd>/<kbd className="bg-muted rounded border px-1 font-mono text-[10px]">k</kbd> to navigate,{" "}
                <kbd className="bg-muted rounded border px-1 font-mono text-[10px]">?</kbd> for all shortcuts
            </span>
            <button onClick={dismiss} className="hover:text-foreground">
                <X className="size-3" />
            </button>
        </div>
    );
}
```

- [ ] **Step 2: Add to chunks list page**

In `chunks.index.tsx`, render `<ShortcutHint />` below the search/filter bar, above the chunk list.

- [ ] **Step 3: Commit**

```bash
git add apps/web/src/features/nav/shortcut-hint.tsx apps/web/src/routes/chunks.index.tsx
git commit -m "feat(web): add dismissable keyboard shortcut hint on chunk list"
```

---

## Task 3: Requirements Count Badges

Show counts per status on the requirements filter sidebar.

**Files:**
- Modify: `apps/web/src/features/requirements/sidebar-filters.tsx`
- Modify: `apps/web/src/routes/requirements.tsx`

- [ ] **Step 1: Read sidebar-filters.tsx**

Understand the `STATUS_OPTIONS` structure and how filters are rendered (lines 73-91). The component receives filter state as props.

- [ ] **Step 2: Add counts prop to sidebar**

Extend the sidebar filter component props to accept counts:
```tsx
interface SidebarFiltersProps {
    // ... existing props
    statusCounts?: Record<string, number>;
}
```

- [ ] **Step 3: Display counts next to status labels**

```tsx
{STATUS_OPTIONS.map(opt => (
    <label key={opt.value} className="flex cursor-pointer items-center gap-2 text-sm">
        <Checkbox ... />
        {opt.label}
        {statusCounts?.[opt.value] != null && (
            <span className="text-muted-foreground ml-auto text-xs">
                {statusCounts[opt.value]}
            </span>
        )}
    </label>
))}
```

- [ ] **Step 4: Pass counts from existing stats query in requirements.tsx**

**Important:** There is no `allRequirements` variable. The requirements page is server-side paginated (`data?.requirements`). Instead, use the existing stats query which already provides `{ passing, failing, untested, total }` counts. Read the file to find the stats query (look for a query that fetches requirement counts/stats). Build the `statusCounts` object from that:

```tsx
const statusCounts = {
    passing: statsData?.passing ?? 0,
    failing: statsData?.failing ?? 0,
    untested: statsData?.untested ?? 0,
};
```

Pass `statusCounts` to the sidebar component.

- [ ] **Step 5: Commit**

```bash
git add apps/web/src/features/requirements/sidebar-filters.tsx apps/web/src/routes/requirements.tsx
git commit -m "feat(web): add count badges to requirements status filters"
```

---

## Task 4: Settings Page Polish

Improve the settings form inputs.

**Files:**
- Modify: `apps/web/src/routes/settings.tsx`

- [ ] **Step 1: Read settings.tsx**

Understand the three tabs and which fields need polish.

- [ ] **Step 2: Replace Default Chunk Type text input with select**

**Important:** The codebase uses `@base-ui/react/select`, NOT standard shadcn Select. The component at `apps/web/src/components/ui/select.tsx` exports different primitives than Radix-based shadcn. **You MUST read the existing Select component AND find an existing usage of `<Select>` elsewhere in the codebase** (e.g., in chunk forms or other settings) to understand the correct API pattern before implementing.

Find the "Default chunk type" input. Replace the free-form `<Input>` with a Select dropdown offering the 5 chunk types: note, document, reference, schema, checklist. Wire the `onChange`/`onValueChange` to `debouncedSave("defaultChunkType", value)`.

- [ ] **Step 3: Add helper text to Default Template ID**

Add a description below the template ID input:
```tsx
<p className="text-muted-foreground text-xs mt-1">
    Find template IDs on the <Link to="/templates" className="underline">templates page</Link>.
</p>
```

- [ ] **Step 4: Add saved indicator**

Add a subtle "Settings saved" feedback. The debounced save already triggers a toast — verify this works. If not, add:
```tsx
toast.success("Settings saved", { duration: 1500 });
```

- [ ] **Step 5: Commit**

```bash
git add apps/web/src/routes/settings.tsx
git commit -m "feat(web): polish settings page with typed selects and helper text"
```

---

## Task 5: Activity Page Filtering and Pagination

Add action type filters and pagination to the activity page.

**Files:**
- Modify: `apps/web/src/routes/activity.tsx`

- [ ] **Step 1: Read activity.tsx**

Understand the current query and existing UI. **Note:** Entity-type filtering already exists (buttons for All/Chunks/Requirements/Connections/Tags/Codebases at lines 108-119). What's missing is **action-type** filtering (created/updated/deleted/archived) and pagination.

- [ ] **Step 2: Add action type filter**

Add filter buttons above the activity list for common action types:
```tsx
const [actionFilter, setActionFilter] = useState<string>("");

const actions = ["created", "updated", "deleted", "archived"];

<div className="flex gap-1 mb-3">
    <Button size="sm" variant={!actionFilter ? "default" : "outline"} onClick={() => setActionFilter("")}>
        All
    </Button>
    {actions.map(a => (
        <Button key={a} size="sm" variant={actionFilter === a ? "default" : "outline"} onClick={() => setActionFilter(a)}>
            {a}
        </Button>
    ))}
</div>
```

Filter the displayed activities client-side:
```tsx
const filtered = activities.filter(e => !actionFilter || e.action === actionFilter);
```

- [ ] **Step 3: Add "Load more" pagination**

Change the query to support offset/limit pagination. Add a "Load more" button at the bottom:
```tsx
const [displayCount, setDisplayCount] = useState(20);
const displayed = filtered.slice(0, displayCount);

// At bottom:
{displayCount < filtered.length && (
    <Button variant="outline" size="sm" onClick={() => setDisplayCount(c => c + 20)}>
        Load more ({filtered.length - displayCount} remaining)
    </Button>
)}
```

- [ ] **Step 4: Commit**

```bash
git add apps/web/src/routes/activity.tsx
git commit -m "feat(web): add action type filters and load-more to activity page"
```
