# Interaction Improvements Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Improve interactions: clickable dashboard stats, chunk hover preview, quick status toggle, clickable graph legend, workspace management page, search page in nav.

**Architecture:** Mostly small component changes. Workspace page is the largest addition — new route with CRUD UI. Graph legend filter is a state change in the existing graph view.

**Tech Stack:** React, TanStack Router, TanStack Query, shadcn-ui, Eden treaty

---

## File Structure

### New files:
- `apps/web/src/routes/workspaces.tsx` — Workspace management page

### Files to modify:
- `apps/web/src/routes/dashboard.tsx` — Make stat cards clickable links
- `apps/web/src/routes/chunks.index.tsx` — Add hover preview tooltip
- `apps/web/src/features/graph/graph-legend.tsx` — Make legend items clickable filters
- `apps/web/src/features/graph/graph-view.tsx` — Wire legend filter state
- `apps/web/src/routes/__root.tsx` — Add Search + Workspaces to nav
- `apps/web/src/features/nav/mobile-nav.tsx` — Same

---

## Task 1: Clickable Dashboard Stats

**Files:**
- Modify: `apps/web/src/routes/dashboard.tsx`

- [ ] **Step 1: Read the StatCard component**

At lines 442-461. It's a plain div with icon + label + value.

- [ ] **Step 2: Add optional `to` prop and wrap in Link**

```tsx
interface StatCardProps {
    icon: LucideIcon;
    label: string;
    value?: number;
    loading?: boolean;
    sub?: string;
    to?: string; // new
}

function StatCard({ icon: Icon, label, value, loading, sub, to }: StatCardProps) {
    const content = (
        <div className={`bg-card rounded-lg border p-4 ${to ? "hover:bg-muted/50 transition-colors cursor-pointer" : ""}`}>
            {/* existing content */}
        </div>
    );

    if (to) {
        // NOTE: TanStack Router's Link has strict route typing.
        // Use `as any` if the `to` string type doesn't match the router's typed routes.
        return <Link to={to as any}>{content}</Link>;
    }
    return content;
}
```

- [ ] **Step 3: Add `to` props to stat cards**

```tsx
<StatCard icon={Blocks} label="Chunks" value={...} to="/chunks" />
<StatCard icon={Network} label="Connections" value={...} to="/graph" />
<StatCard icon={Tags} label="Tags" value={...} to="/tags" />
<StatCard icon={FileText} label="Requirements" value={...} to="/requirements" />
```

- [ ] **Step 4: Commit**

```bash
git commit -m "feat(web): make dashboard stat cards clickable links"
```

---

## Task 2: Chunk List Hover Preview

**Files:**
- Modify: `apps/web/src/routes/chunks.index.tsx`

- [ ] **Step 1: Add summary tooltip on chunk rows**

On each chunk list item, show a tooltip with the chunk's content preview on hover. Use the base-ui Tooltip component (check if `apps/web/src/components/ui/tooltip.tsx` exists).

If Tooltip exists, wrap each chunk row's title in:
```tsx
<Tooltip>
    <TooltipTrigger render={<span className="truncate text-sm font-medium" />}>
        {chunk.title}
    </TooltipTrigger>
    <TooltipPopup className="max-w-sm">
        <p className="text-xs">{chunk.content?.slice(0, 150)}{chunk.content?.length > 150 ? "..." : ""}</p>
    </TooltipPopup>
</Tooltip>
```

Read the existing Tooltip component to match the exact API (base-ui uses `render` prop, `TooltipPopup` instead of `TooltipContent`).

If no Tooltip component exists, use a simple CSS `title` attribute as a fallback:
```tsx
<p className="truncate text-sm font-medium" title={chunk.content?.slice(0, 150)}>
    {chunk.title}
</p>
```

- [ ] **Step 2: Commit**

```bash
git commit -m "feat(web): add content preview tooltip on chunk list items"
```

---

## Task 3: Clickable Graph Legend as Filter

**Files:**
- Modify: `apps/web/src/features/graph/graph-legend.tsx`
- Modify: `apps/web/src/features/graph/graph-view.tsx`

- [ ] **Step 1: Read the graph legend and filter state**

The legend at `graph-legend.tsx` shows type colors and relation colors. The graph view at `graph-view.tsx` has `filterTypes` state (a Set of visible types). Read how `filterTypes` is used to filter nodes.

- [ ] **Step 2: Make legend items clickable**

**NOTE:** `GraphLegend` already accepts `activeTypes: Set<string>` and `activeRelations: Set<string>` props. The existing rendering already dims items not in `activeTypes`. You just need to add click handlers — don't add duplicate `filterTypes` prop.

Add `onToggleType` and `onToggleRelation` callback props to the existing interface:

```tsx
// In graph-view.tsx, pass callbacks to existing GraphLegend:
<GraphLegend
    activeTypes={gs.filterTypes}
    activeRelations={gs.filterRelations}
    onToggleType={(type) => dispatch({ type: "TOGGLE_FILTER_TYPE", filterType: type })}
    onToggleRelation={(rel) => dispatch({ type: "TOGGLE_FILTER_RELATION", relation: rel })}
/>
```

In `graph-legend.tsx`, extend the existing interface with optional callbacks:

```tsx
interface GraphLegendProps {
    activeTypes: Set<string>;
    activeRelations: Set<string>;
    onToggleType?: (type: string) => void;    // new
    onToggleRelation?: (rel: string) => void; // new
}

// Make each type entry a clickable button (it already dims via activeTypes):
<button
    onClick={() => onToggleType?.(type)}
    className={`flex items-center gap-2 text-xs cursor-pointer hover:opacity-80`}
>
    <div className="size-3 rounded-sm border" style={{ background: colors.bg, borderColor: colors.border }} />
    {colors.label}
</button>
```

When a type is filtered out, the legend item appears dimmed. Clicking toggles visibility.

- [ ] **Step 3: Commit**

```bash
git commit -m "feat(web): make graph legend items clickable to filter node types"
```

---

## Task 4: Workspace Management Page

**Files:**
- Create: `apps/web/src/routes/workspaces.tsx`
- Modify: `apps/web/src/routes/__root.tsx` — Add to nav
- Modify: `apps/web/src/features/nav/mobile-nav.tsx` — Add to nav

- [ ] **Step 1: Create workspace page**

Read the workspace API endpoints (at `packages/api/src/workspaces/routes.ts`):
```
GET    /workspaces              — list
POST   /workspaces              — create
GET    /workspaces/:id          — detail
PATCH  /workspaces/:id          — update
DELETE /workspaces/:id          — delete
POST   /workspaces/:id/codebases     — add codebase
DELETE /workspaces/:id/codebases/:id  — remove codebase
```

Create `apps/web/src/routes/workspaces.tsx` with:
- List of workspaces with name, description, codebase count
- Create form (inline or dialog): name + description
- Each workspace expandable to show its codebases
- "Add Codebase" button that shows a dropdown of available codebases
- Remove codebase button on each
- Edit/delete workspace

Use `PageContainer`, `PageHeader`, `PageEmpty`, `PageLoading` from `@/components/ui/page`.

Read `apps/web/src/routes/codebases.tsx` for a similar CRUD page pattern.

- [ ] **Step 2: Regenerate route tree**

```bash
cd apps/web && npx @tanstack/router-cli generate
```

- [ ] **Step 3: Add to navigation**

In `__root.tsx`, add "Workspaces" to the Manage dropdown (near Codebases).
In `mobile-nav.tsx`, add to manage items.

Also add "Search" to the main nav (it exists at `/search` but isn't in the nav):
```tsx
<Link to="/search">Search</Link>
```

- [ ] **Step 4: Commit**

```bash
git commit -m "feat(web): add workspace management page and search to nav"
```

---

## Task 5: Quick Review Status Toggle on Chunk List

**Files:**
- Modify: `apps/web/src/routes/chunks.index.tsx`

- [ ] **Step 1: Read chunk list item rendering**

Find where each chunk row renders (the CardPanel with checkbox, pin, link).

- [ ] **Step 2: Add status dot for AI-generated chunks**

For chunks with `origin === "ai"`, show a clickable status dot that cycles through review statuses:

```tsx
{(chunk as any).origin === "ai" && (
    <button
        onClick={(e) => {
            e.preventDefault();
            e.stopPropagation();
            const next = { draft: "reviewed", reviewed: "approved", approved: "draft" }[(chunk as any).reviewStatus] ?? "reviewed";
            reviewMutation.mutate({ id: chunk.id, status: next });
        }}
        className="size-2.5 rounded-full shrink-0"
        style={{
            backgroundColor: (chunk as any).reviewStatus === "approved" ? "#22c55e"
                : (chunk as any).reviewStatus === "reviewed" ? "#3b82f6"
                : "#f59e0b"
        }}
        title={`Review: ${(chunk as any).reviewStatus} (click to change)`}
    />
)}
```

Add a `reviewMutation` using the Eden treaty PATCH endpoint.

- [ ] **Step 3: Commit**

```bash
git commit -m "feat(web): add quick review status toggle on chunk list rows"
```
