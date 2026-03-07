# UX Features Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add 7 user-facing features: chunk templates, keyboard navigation, duplicate detection, backlinks, rich markdown preview, mobile
nav drawer, and force-directed graph layout.

**Architecture:** Most tasks are frontend-only (React components + CSS). Duplicate detection adds a service-layer check using existing
pg_trgm. Backlinks surfaces existing connection data directionally. The graph task replaces the grid layout with dagre auto-layout. Rich
markdown replaces the custom parser with react-markdown.

**Tech Stack:** TypeScript, React 19, TanStack Router/Query, shadcn-ui (base-ui variant), react-markdown, @dagrejs/dagre, @xyflow/react

---

### Task 1: Chunk Templates

Add a "New from template" option on the create page. Templates are hardcoded (no DB) — a simple array of predefined chunk structures.

**Files:**

- Create: `apps/web/src/features/chunks/templates.ts`
- Modify: `apps/web/src/routes/chunks.new.tsx`

**Step 1: Create templates data file**

Create `apps/web/src/features/chunks/templates.ts`:

```typescript
export interface ChunkTemplate {
    name: string;
    description: string;
    type: string;
    tags: string[];
    content: string;
}

export const chunkTemplates: ChunkTemplate[] = [
    {
        name: "Meeting Notes",
        description: "Structured meeting notes with attendees and action items",
        type: "note",
        tags: ["meeting"],
        content: `## Attendees\n\n- \n\n## Agenda\n\n- \n\n## Discussion\n\n\n\n## Action Items\n\n- [ ] `
    },
    {
        name: "Decision Record",
        description: "Document a technical or product decision",
        type: "document",
        tags: ["decision"],
        content: `## Context\n\nWhat is the issue that we're seeing that is motivating this decision?\n\n## Decision\n\nWhat is the change that we're proposing and/or doing?\n\n## Consequences\n\nWhat becomes easier or harder because of this change?`
    },
    {
        name: "API Reference",
        description: "Document an API endpoint or service",
        type: "reference",
        tags: ["api"],
        content: `## Endpoint\n\n\`METHOD /path\`\n\n## Request\n\n\n\n## Response\n\n\n\n## Examples\n\n`
    },
    {
        name: "Checklist",
        description: "A reusable checklist",
        type: "checklist",
        tags: ["checklist"],
        content: `- [ ] \n- [ ] \n- [ ] `
    },
    {
        name: "Schema",
        description: "Document a data model or schema",
        type: "schema",
        tags: ["schema"],
        content: `## Fields\n\n- \`id\` — \n- \`name\` — \n\n## Relations\n\n\n\n## Constraints\n\n`
    }
];
```

**Step 2: Add template selector to new chunk page**

In `apps/web/src/routes/chunks.new.tsx`, add a template selector between the AI Generate card and the form Card.

Add imports:

```typescript
import { FileText } from "lucide-react";
import { chunkTemplates } from "@/features/chunks/templates";
```

Add this JSX between the AI Generate `</Card>` (line 144) and the form `<Card>` (line 146):

```tsx
<Card className="mb-6">
    <CardPanel className="p-6">
        <div className="flex items-center gap-2 mb-3">
            <FileText className="size-4" />
            <span className="text-sm font-medium">Start from Template</span>
        </div>
        <div className="grid grid-cols-2 gap-2 sm:grid-cols-3">
            {chunkTemplates.map(tmpl => (
                <button
                    key={tmpl.name}
                    type="button"
                    onClick={() => {
                        setTitle("");
                        setContent(tmpl.content);
                        setType(tmpl.type);
                        setTags(tmpl.tags);
                    }}
                    className="hover:bg-muted rounded-md border p-3 text-left transition-colors"
                >
                    <p className="text-sm font-medium">{tmpl.name}</p>
                    <p className="text-muted-foreground mt-0.5 text-xs">{tmpl.description}</p>
                </button>
            ))}
        </div>
    </CardPanel>
</Card>
```

**Step 3: Run type check**

Run: `cd /Users/pontus/GitHub/fubbik && bun tsc --noEmit`

**Step 4: Commit**

```bash
git add apps/web/src/features/chunks/templates.ts apps/web/src/routes/chunks.new.tsx
git commit -m "feat: add chunk templates on create page"
```

---

### Task 2: Keyboard Navigation

Add keyboard shortcuts to the chunks list page: `j`/`k` to move selection, `Enter` to open, `n` for new chunk, `e` to edit selected.

**Files:**

- Modify: `apps/web/src/routes/chunks.index.tsx`

**Step 1: Add keyboard navigation to chunks list**

In `apps/web/src/routes/chunks.index.tsx`, add a `selectedIndex` state and a keyboard event handler.

Add to imports:

```typescript
import { useEffect, useState } from "react";
```

(Replace the existing `import { useState } from "react"` line.)

Add state inside `ChunksList`:

```typescript
const [selectedIndex, setSelectedIndex] = useState(-1);
```

Add keyboard handler after the state declarations:

```typescript
useEffect(() => {
    function handleKeyDown(e: KeyboardEvent) {
        // Don't handle if typing in an input
        if (e.target instanceof HTMLInputElement || e.target instanceof HTMLTextAreaElement) return;

        switch (e.key) {
            case "j":
                setSelectedIndex(i => Math.min(i + 1, chunks.length - 1));
                break;
            case "k":
                setSelectedIndex(i => Math.max(i - 1, 0));
                break;
            case "Enter":
                if (selectedIndex >= 0 && selectedIndex < chunks.length) {
                    navigate({ to: "/chunks/$chunkId", params: { chunkId: chunks[selectedIndex].id } });
                }
                break;
            case "n":
                navigate({ to: "/chunks/new" });
                break;
            case "e":
                if (selectedIndex >= 0 && selectedIndex < chunks.length) {
                    navigate({
                        to: "/chunks/$chunkId/edit",
                        params: { chunkId: chunks[selectedIndex].id }
                    });
                }
                break;
        }
    }
    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
}, [selectedIndex, chunks, navigate]);
```

Update the `navigate` hook to include the `from` parameter for typed navigation:

```typescript
const navigate = useNavigate();
```

(Remove the `{ from: "/chunks/" }` if it causes type issues with the new routes.)

In the chunk list rendering, add a visual selection indicator. Replace the `<Link>` wrapper for each chunk (the
`<Link to="/chunks/$chunkId" ...>` block) to include a conditional ring:

Change the outer `<CardPanel>` className from:

```
hover:bg-muted/50 flex items-center justify-between gap-4 p-4 transition-colors
```

to:

```tsx
className={`flex items-center justify-between gap-4 p-4 transition-colors ${
    selectedIndex === i ? "bg-muted/50 ring-primary/50 ring-2 ring-inset" : "hover:bg-muted/50"
}`}
```

Reset selection when data changes:

```typescript
useEffect(() => {
    setSelectedIndex(-1);
}, [chunks]);
```

**Step 2: Add keyboard hint to page header**

Add a small hint below the page title:

```tsx
<p className="text-muted-foreground text-xs mt-1">
    <kbd className="bg-muted rounded px-1 py-0.5 text-[10px] font-mono">j</kbd>/
    <kbd className="bg-muted rounded px-1 py-0.5 text-[10px] font-mono">k</kbd> navigate
    <kbd className="bg-muted rounded px-1 py-0.5 text-[10px] font-mono ml-2">Enter</kbd> open
    <kbd className="bg-muted rounded px-1 py-0.5 text-[10px] font-mono ml-2">n</kbd> new
</p>
```

Place this after the `<h1>` tag inside the header flex container.

**Step 3: Run type check**

Run: `cd /Users/pontus/GitHub/fubbik && bun tsc --noEmit`

**Step 4: Commit**

```bash
git add apps/web/src/routes/chunks.index.tsx
git commit -m "feat: add keyboard navigation to chunks list (j/k/Enter/n/e)"
```

---

### Task 3: Duplicate Chunk Detection

Warn the user when creating a chunk with a title similar to an existing one. Uses the existing search endpoint with pg_trgm.

**Files:**

- Modify: `apps/web/src/routes/chunks.new.tsx`

**Step 1: Add duplicate check query**

In `apps/web/src/routes/chunks.new.tsx`, add a `useQuery` import and a debounced duplicate check.

Add to imports:

```typescript
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
```

Add state for debounced title:

```typescript
const [debouncedTitle, setDebouncedTitle] = useState("");
```

Add debounce effect:

```typescript
useEffect(() => {
    const timer = setTimeout(() => setDebouncedTitle(title), 500);
    return () => clearTimeout(timer);
}, [title]);
```

Add `useEffect` import (add it to existing react import):

```typescript
import { useEffect, useState } from "react";
```

Add duplicate check query after the state declarations:

```typescript
const duplicateQuery = useQuery({
    queryKey: ["duplicate-check", debouncedTitle],
    queryFn: async () => {
        if (!debouncedTitle.trim() || debouncedTitle.length < 3) return [];
        try {
            const result = unwrapEden(await api.api.chunks.get({ query: { search: debouncedTitle, limit: "3" } }));
            return (
                result?.chunks?.filter((c: { title: string }) => c.title.toLowerCase() !== debouncedTitle.toLowerCase()).slice(0, 3) ?? []
            );
        } catch {
            return [];
        }
    },
    enabled: debouncedTitle.length >= 3
});

const duplicates = duplicateQuery.data ?? [];
```

Add a warning below the title input (after the `errors.title` paragraph, around line 157):

```tsx
{
    duplicates.length > 0 && (
        <div className="mt-2 rounded-md border border-yellow-500/30 bg-yellow-500/10 px-3 py-2">
            <p className="text-xs font-medium text-yellow-600 dark:text-yellow-400">Similar chunks exist:</p>
            <ul className="mt-1 space-y-0.5">
                {duplicates.map((d: { id: string; title: string }) => (
                    <li key={d.id} className="text-xs">
                        <Link to="/chunks/$chunkId" params={{ chunkId: d.id }} className="text-yellow-600 underline dark:text-yellow-400">
                            {d.title}
                        </Link>
                    </li>
                ))}
            </ul>
        </div>
    );
}
```

**Step 2: Run type check**

Run: `cd /Users/pontus/GitHub/fubbik && bun tsc --noEmit`

**Step 3: Commit**

```bash
git add apps/web/src/routes/chunks.new.tsx
git commit -m "feat: add duplicate chunk detection on create page"
```

---

### Task 4: Chunk Backlinks

Show directional connections on the chunk detail page: "Links to" and "Linked from" as separate sections.

**Files:**

- Modify: `apps/web/src/routes/chunks.$chunkId.tsx`

**Step 1: Split connections into outgoing and incoming**

In `apps/web/src/routes/chunks.$chunkId.tsx`, replace the single connections list with two sections.

Currently (around line 88):

```typescript
const connections = data.connections ?? [];
```

Replace with:

```typescript
const connections = data.connections ?? [];
const outgoing = connections.filter(c => c.sourceId === chunkId);
const incoming = connections.filter(c => c.sourceId !== chunkId);
```

**Step 2: Update the connections card**

Replace the current connections Card (lines 178-218) with two sections. Add `ArrowRight, ArrowLeft` to the lucide-react import (line 3 — add
them to the existing import).

Actually, to keep it clean, use a single card with two subsections:

```tsx
<Card>
    <CardHeader>
        <div className="flex items-center justify-between">
            <div>
                <CardTitle className="flex items-center gap-2 text-sm">
                    <Network className="size-4" />
                    Connections
                </CardTitle>
                <CardDescription>{connections.length} linked chunks</CardDescription>
            </div>
            <LinkChunkDialog chunkId={chunkId} />
        </div>
    </CardHeader>
    {outgoing.length > 0 && (
        <CardPanel className="space-y-2 pt-0">
            <p className="text-muted-foreground text-xs font-medium">Links to</p>
            {outgoing.map(conn => (
                <div
                    key={conn.id}
                    className="hover:bg-muted flex items-center justify-between rounded-md border px-3 py-2 text-sm transition-colors"
                >
                    <Link to="/chunks/$chunkId" params={{ chunkId: conn.targetId }} className="flex-1 font-medium">
                        {conn.title ?? conn.targetId}
                    </Link>
                    <div className="flex items-center gap-2">
                        <Badge variant="outline" size="sm" className="text-[10px]">
                            {conn.relation}
                        </Badge>
                        <DeleteConnectionButton connectionId={conn.id} chunkId={chunkId} />
                    </div>
                </div>
            ))}
        </CardPanel>
    )}
    {incoming.length > 0 && (
        <CardPanel className="space-y-2 pt-0">
            <p className="text-muted-foreground text-xs font-medium">Linked from</p>
            {incoming.map(conn => (
                <div
                    key={conn.id}
                    className="hover:bg-muted flex items-center justify-between rounded-md border px-3 py-2 text-sm transition-colors"
                >
                    <Link to="/chunks/$chunkId" params={{ chunkId: conn.sourceId }} className="flex-1 font-medium">
                        {conn.title ?? conn.sourceId}
                    </Link>
                    <div className="flex items-center gap-2">
                        <Badge variant="outline" size="sm" className="text-[10px]">
                            {conn.relation}
                        </Badge>
                        <DeleteConnectionButton connectionId={conn.id} chunkId={chunkId} />
                    </div>
                </div>
            ))}
        </CardPanel>
    )}
    {connections.length === 0 && (
        <CardPanel className="pt-0">
            <p className="text-muted-foreground text-sm">No connections yet</p>
        </CardPanel>
    )}
</Card>
```

**Step 3: Run type check**

Run: `cd /Users/pontus/GitHub/fubbik && bun tsc --noEmit`

**Step 4: Commit**

```bash
git add apps/web/src/routes/chunks.\$chunkId.tsx
git commit -m "feat: show directional backlinks — 'links to' and 'linked from'"
```

---

### Task 5: Rich Markdown Preview

Replace the custom `## ` / `- ` parser with `react-markdown` for proper rendering in both the editor preview and chunk detail page.

**Files:**

- Modify: `apps/web/package.json` (add dependency)
- Modify: `apps/web/src/features/editor/markdown-editor.tsx`
- Modify: `apps/web/src/routes/chunks.$chunkId.tsx`

**Step 1: Install react-markdown**

Run: `cd /Users/pontus/GitHub/fubbik/apps/web && bun add react-markdown`

**Step 2: Replace markdown preview in editor**

In `apps/web/src/features/editor/markdown-editor.tsx`, replace the entire `renderMarkdownPreview` function and update imports:

```typescript
import Markdown from "react-markdown";
import { Tabs, TabsContent, TabsList, TabsTab } from "@/components/ui/tabs";

interface MarkdownEditorProps {
    value: string;
    onChange: (value: string) => void;
    placeholder?: string;
    rows?: number;
    error?: string;
}

export function MarkdownEditor({ value, onChange, placeholder, rows = 10, error }: MarkdownEditorProps) {
    return (
        <div>
            <label className="mb-1.5 block text-sm font-medium">Content</label>
            <Tabs defaultValue={0}>
                <TabsList>
                    <TabsTab value={0}>Write</TabsTab>
                    <TabsTab value={1}>Preview</TabsTab>
                </TabsList>
                <TabsContent value={0}>
                    <textarea
                        value={value}
                        onChange={e => onChange(e.target.value)}
                        placeholder={placeholder}
                        rows={rows}
                        className="bg-background focus:ring-ring w-full resize-y rounded-md border px-3 py-2 text-sm focus:ring-2 focus:outline-none"
                    />
                </TabsContent>
                <TabsContent value={1}>
                    <div className="min-h-[120px] rounded-md border px-3 py-2">
                        {!value.trim() ? (
                            <p className="text-muted-foreground text-sm italic">Nothing to preview</p>
                        ) : (
                            <Markdown className="prose prose-invert prose-sm max-w-none">{value}</Markdown>
                        )}
                    </div>
                </TabsContent>
            </Tabs>
            {error && <p className="text-destructive mt-1 text-xs">{error}</p>}
        </div>
    );
}
```

**Step 3: Replace markdown rendering on chunk detail page**

In `apps/web/src/routes/chunks.$chunkId.tsx`, replace the content rendering block (lines 148-171) — the
`<div className="prose prose-invert prose-sm max-w-none">` block with the custom split/map logic.

Add import:

```typescript
import Markdown from "react-markdown";
```

Replace the content block with:

```tsx
<Markdown className="prose prose-invert prose-sm max-w-none">{chunk.content}</Markdown>
```

**Step 4: Run type check**

Run: `cd /Users/pontus/GitHub/fubbik && bun tsc --noEmit`

**Step 5: Commit**

```bash
git add apps/web/package.json bun.lock \
  apps/web/src/features/editor/markdown-editor.tsx \
  apps/web/src/routes/chunks.\$chunkId.tsx
git commit -m "feat: replace custom markdown parser with react-markdown"
```

---

### Task 6: Mobile Nav Drawer

Add a hamburger menu that shows on mobile screens with links to Dashboard, Chunks, Graph, Tags.

**Files:**

- Create: `apps/web/src/features/nav/mobile-nav.tsx`
- Modify: `apps/web/src/routes/__root.tsx`

**Step 1: Create mobile nav component**

Create `apps/web/src/features/nav/mobile-nav.tsx`:

```typescript
import { Link } from "@tanstack/react-router";
import { Blocks, LayoutDashboard, Menu, Network, Tags } from "lucide-react";
import { useState } from "react";

import { Sheet, SheetContent, SheetTrigger } from "@/components/ui/sheet";
import { Button } from "@/components/ui/button";
import { Separator } from "@/components/ui/separator";

const navItems = [
    { label: "Dashboard", to: "/dashboard" as const, icon: LayoutDashboard },
    { label: "Chunks", to: "/chunks" as const, icon: Blocks },
    { label: "Graph", to: "/graph" as const, icon: Network },
    { label: "Tags", to: "/tags" as const, icon: Tags },
];

export function MobileNav() {
    const [open, setOpen] = useState(false);

    return (
        <Sheet open={open} onOpenChange={setOpen}>
            <SheetTrigger render={<Button variant="ghost" size="sm" className="md:hidden" />}>
                <Menu className="size-5" />
            </SheetTrigger>
            <SheetContent side="left" className="w-64 p-0">
                <div className="p-4">
                    <span className="text-lg font-bold">fubbik</span>
                </div>
                <Separator />
                <nav className="space-y-1 p-2">
                    {navItems.map(item => (
                        <Link
                            key={item.to}
                            to={item.to}
                            onClick={() => setOpen(false)}
                            className="hover:bg-muted flex items-center gap-3 rounded-md px-3 py-2 text-sm font-medium transition-colors"
                        >
                            <item.icon className="size-4" />
                            {item.label}
                        </Link>
                    ))}
                </nav>
            </SheetContent>
        </Sheet>
    );
}
```

**NOTE:** The Sheet component API depends on what's in `apps/web/src/components/ui/sheet.tsx`. Read that file first and adapt the import
names. If `SheetContent` doesn't accept a `side` prop, use `className` positioning instead. If the Sheet uses base-ui Dialog internally,
`SheetTrigger` might use `render` prop or `asChild` pattern.

**Step 2: Add MobileNav to root layout**

In `apps/web/src/routes/__root.tsx`, add import:

```typescript
import { MobileNav } from "@/features/nav/mobile-nav";
```

Add `<MobileNav />` as the first item in the header's right-side `<div>` (line 60-64):

```tsx
<div className="flex items-center gap-2">
    <MobileNav />
    <ChunkSearch />
    <ThemeToggle />
    <UserMenu />
</div>
```

**Step 3: Hide desktop nav items on mobile (optional)**

If needed, add `className="hidden md:flex"` to specific desktop-only elements. The current header is minimal enough that this may not be
needed.

**Step 4: Run type check**

Run: `cd /Users/pontus/GitHub/fubbik && bun tsc --noEmit`

**Step 5: Commit**

```bash
git add apps/web/src/features/nav/mobile-nav.tsx apps/web/src/routes/__root.tsx
git commit -m "feat: add mobile navigation drawer"
```

---

### Task 7: Force-Directed Graph Layout

Replace the grid-based node positions with dagre auto-layout for meaningful graph visualization.

**Files:**

- Modify: `apps/web/package.json` (add dependency)
- Modify: `apps/web/src/routes/graph.tsx`

**Step 1: Install dagre**

Run: `cd /Users/pontus/GitHub/fubbik/apps/web && bun add @dagrejs/dagre`

Check if types are needed: `bun add -d @types/dagre` (the `@dagrejs/dagre` package may include types already).

**Step 2: Replace grid layout with dagre**

In `apps/web/src/routes/graph.tsx`, replace the `initialNodes` memo with dagre-based layout.

Add import:

```typescript
import Dagre from "@dagrejs/dagre";
```

Replace both `initialNodes` and `initialEdges` memos (lines 41-60) with a single combined memo:

```typescript
const { layoutNodes, layoutEdges } = useMemo(() => {
    const chunks = data?.chunks ?? [];
    const connections = data?.connections ?? [];

    if (chunks.length === 0) return { layoutNodes: [], layoutEdges: [] };

    const g = new Dagre.graphlib.Graph().setDefaultEdgeLabel(() => ({}));
    g.setGraph({ rankdir: "TB", nodesep: 80, ranksep: 100 });

    const rawNodes: Node[] = chunks.map(c => ({
        id: c.id,
        data: { label: c.title },
        position: { x: 0, y: 0 },
        style: { cursor: "pointer" }
    }));

    const rawEdges: Edge[] = connections.map(conn => ({
        id: conn.id,
        source: conn.sourceId,
        target: conn.targetId,
        label: conn.relation,
        animated: true
    }));

    for (const node of rawNodes) {
        g.setNode(node.id, { width: 200, height: 50 });
    }
    for (const edge of rawEdges) {
        g.setEdge(edge.source, edge.target);
    }

    Dagre.layout(g);

    const layoutNodes = rawNodes.map(node => {
        const pos = g.node(node.id);
        return { ...node, position: { x: pos.x - 100, y: pos.y - 25 } };
    });

    return { layoutNodes, layoutEdges: rawEdges };
}, [data]);
```

Update the useEffect hooks to use `layoutNodes` and `layoutEdges`:

```typescript
useEffect(() => {
    setNodes(layoutNodes);
}, [layoutNodes, setNodes]);

useEffect(() => {
    setEdges(layoutEdges);
}, [layoutEdges, setEdges]);
```

Remove the old separate `initialNodes` and `initialEdges` memos.

**Step 3: Run type check**

Run: `cd /Users/pontus/GitHub/fubbik && bun tsc --noEmit`

**Step 4: Commit**

```bash
git add apps/web/package.json bun.lock apps/web/src/routes/graph.tsx
git commit -m "feat: replace grid graph layout with dagre force-directed layout"
```
