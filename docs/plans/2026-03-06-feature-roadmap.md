# Feature Roadmap Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Build out the 12 features that complete fubbik's core loop — editing, connections, search, graph visualization, AI, and more.

**Architecture:** Features build on the existing repository -> service -> route -> UI pattern with Effect typed errors. Backend additions go
through the same layers. Frontend uses TanStack Router file-based routes, TanStack Query for data, and shadcn-ui components.

**Tech Stack:** TypeScript, Effect, Elysia, Drizzle, TanStack Router/Query, React 19, shadcn-ui, Vercel AI SDK, @xyflow/react

---

### Task 1: Chunk Edit Page

The PATCH endpoint exists but the web UI has no edit page. The "Edit" button on the detail page is non-functional.

**Files:**

- Create: `apps/web/src/routes/chunks.$chunkId.edit.tsx`
- Modify: `apps/web/src/routes/chunks.$chunkId.tsx` (wire Edit button)

**Step 1: Create the edit route**

Create `apps/web/src/routes/chunks.$chunkId.edit.tsx`. This reuses the same form layout as `chunks.new.tsx` but pre-fills from an existing
chunk and calls PATCH instead of POST.

```typescript
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { createFileRoute, Link, redirect, useNavigate } from "@tanstack/react-router";
import { ArrowLeft } from "lucide-react";
import { useEffect, useState } from "react";
import { toast } from "sonner";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardPanel } from "@/components/ui/card";
import { Separator } from "@/components/ui/separator";
import { getUser } from "@/functions/get-user";
import { api } from "@/utils/api";

export const Route = createFileRoute("/chunks/$chunkId/edit")({
    component: EditChunk,
    beforeLoad: async () => {
        try {
            const session = await getUser();
            return { session };
        } catch {
            throw redirect({ to: "/login" });
        }
    }
});

function EditChunk() {
    const { chunkId } = Route.useParams();
    const navigate = useNavigate();
    const queryClient = useQueryClient();
    const [title, setTitle] = useState("");
    const [content, setContent] = useState("");
    const [type, setType] = useState("note");
    const [tagInput, setTagInput] = useState("");
    const [tags, setTags] = useState<string[]>([]);
    const [errors, setErrors] = useState<Record<string, string>>({});
    const [initialized, setInitialized] = useState(false);

    const { data, isLoading } = useQuery({
        queryKey: ["chunk", chunkId],
        queryFn: async () => {
            const { data, error } = await api.api.chunks({ id: chunkId }).get();
            if (error) throw new Error("Failed to load chunk");
            return data;
        }
    });

    useEffect(() => {
        if (data?.chunk && !initialized) {
            setTitle(data.chunk.title);
            setContent(data.chunk.content);
            setType(data.chunk.type);
            setTags(data.chunk.tags as string[]);
            setInitialized(true);
        }
    }, [data, initialized]);

    function validate() {
        const e: Record<string, string> = {};
        if (!title.trim()) e.title = "Title is required";
        else if (title.length > 200) e.title = "Title must be 200 characters or less";
        if (content.length > 50000) e.content = "Content must be 50,000 characters or less";
        setErrors(e);
        return Object.keys(e).length === 0;
    }

    const updateMutation = useMutation({
        mutationFn: async () => {
            const { error } = await api.api.chunks({ id: chunkId }).patch({
                title,
                content,
                type,
                tags
            });
            if (error) throw new Error("Failed to update chunk");
        },
        onSuccess: () => {
            queryClient.invalidateQueries({ queryKey: ["chunk", chunkId] });
            queryClient.invalidateQueries({ queryKey: ["chunks"] });
            toast.success("Chunk updated");
            navigate({ to: "/chunks/$chunkId", params: { chunkId } });
        },
        onError: () => {
            toast.error("Failed to update chunk");
        }
    });

    const addTag = () => {
        const tag = tagInput.trim().toLowerCase();
        if (tag && !tags.includes(tag)) {
            setTags([...tags, tag]);
        }
        setTagInput("");
    };

    if (isLoading) {
        return (
            <div className="container mx-auto max-w-3xl px-4 py-8">
                <p className="text-muted-foreground text-center">Loading...</p>
            </div>
        );
    }

    // Same JSX as chunks.new.tsx but:
    // - Title says "Edit Chunk"
    // - Button says "Save Changes" / "Saving..."
    // - Cancel links back to /chunks/$chunkId
    // - Uses updateMutation.mutate() instead of createMutation
    return (
        <div className="container mx-auto max-w-3xl px-4 py-8">
            {/* Back button */}
            <div className="mb-6 flex items-center justify-between">
                <Button variant="ghost" size="sm" render={<Link to="/chunks/$chunkId" params={{ chunkId }} />}>
                    <ArrowLeft className="size-4" />
                    Back
                </Button>
            </div>

            <h1 className="mb-6 text-2xl font-bold tracking-tight">Edit Chunk</h1>

            <Card>
                <CardPanel className="space-y-4 p-6">
                    {/* Title input */}
                    <div>
                        <label className="mb-1.5 block text-sm font-medium">Title</label>
                        <input
                            type="text"
                            value={title}
                            onChange={e => setTitle(e.target.value)}
                            placeholder="Enter a title..."
                            className="bg-background focus:ring-ring w-full rounded-md border px-3 py-2 text-sm focus:ring-2 focus:outline-none"
                        />
                        {errors.title && <p className="text-destructive mt-1 text-xs">{errors.title}</p>}
                    </div>

                    {/* Type selector */}
                    <div>
                        <label className="mb-1.5 block text-sm font-medium">Type</label>
                        <select
                            value={type}
                            onChange={e => setType(e.target.value)}
                            className="bg-background focus:ring-ring w-full rounded-md border px-3 py-2 text-sm focus:ring-2 focus:outline-none"
                        >
                            <option value="note">Note</option>
                            <option value="document">Document</option>
                            <option value="reference">Reference</option>
                            <option value="schema">Schema</option>
                            <option value="checklist">Checklist</option>
                        </select>
                    </div>

                    {/* Tags input (same as chunks.new.tsx) */}
                    <div>
                        <label className="mb-1.5 block text-sm font-medium">Tags</label>
                        <div className="mb-2 flex flex-wrap gap-2">
                            {tags.map(tag => (
                                <Badge key={tag} variant="secondary" size="sm" className="cursor-pointer" onClick={() => setTags(tags.filter(t => t !== tag))}>
                                    {tag} x
                                </Badge>
                            ))}
                        </div>
                        <input
                            type="text"
                            value={tagInput}
                            onChange={e => setTagInput(e.target.value)}
                            onKeyDown={e => { if (e.key === "Enter") { e.preventDefault(); addTag(); } }}
                            placeholder="Add a tag and press Enter..."
                            className="bg-background focus:ring-ring w-full rounded-md border px-3 py-2 text-sm focus:ring-2 focus:outline-none"
                        />
                    </div>

                    {/* Content textarea */}
                    <div>
                        <label className="mb-1.5 block text-sm font-medium">Content</label>
                        <textarea
                            value={content}
                            onChange={e => setContent(e.target.value)}
                            placeholder="Write your content..."
                            rows={10}
                            className="bg-background focus:ring-ring w-full resize-y rounded-md border px-3 py-2 text-sm focus:ring-2 focus:outline-none"
                        />
                        {errors.content && <p className="text-destructive mt-1 text-xs">{errors.content}</p>}
                    </div>

                    <Separator />

                    <div className="flex justify-end gap-2">
                        <Button variant="outline" render={<Link to="/chunks/$chunkId" params={{ chunkId }} />}>
                            Cancel
                        </Button>
                        <Button
                            onClick={() => { if (validate()) updateMutation.mutate(); }}
                            disabled={updateMutation.isPending}
                        >
                            {updateMutation.isPending ? "Saving..." : "Save Changes"}
                        </Button>
                    </div>
                </CardPanel>
            </Card>
        </div>
    );
}
```

**Step 2: Wire the Edit button on the detail page**

In `apps/web/src/routes/chunks.$chunkId.tsx`, change the Edit button (line 94-97) from a dead button to a link:

```typescript
// Before:
<Button variant="outline" size="sm">
    <Edit className="size-3.5" />
    Edit
</Button>

// After:
<Button variant="outline" size="sm" render={<Link to="/chunks/$chunkId/edit" params={{ chunkId }} />}>
    <Edit className="size-3.5" />
    Edit
</Button>
```

**Step 3: Run type check**

Run: `cd /Users/pontus/GitHub/fubbik && bun run check-types` Expected: No new errors

**Step 4: Manual test**

Run: `bun dev`, navigate to a chunk, click Edit, modify fields, save.

**Step 5: Commit**

```bash
git add apps/web/src/routes/chunks.\$chunkId.edit.tsx apps/web/src/routes/chunks.\$chunkId.tsx
git commit -m "feat: add chunk edit page and wire Edit button"
```

---

### Task 2: Connection Management (CRUD)

Connections exist in the DB schema but can only be created via seed data. Add full CRUD: API endpoints, service layer, repository functions,
and UI on the chunk detail page.

**Files:**

- Create: `packages/db/src/repository/connection.ts`
- Modify: `packages/db/src/repository/index.ts` (re-export)
- Create: `packages/api/src/connections/service.ts`
- Create: `packages/api/src/connections/routes.ts`
- Modify: `packages/api/src/index.ts` (use connectionRoutes)
- Modify: `apps/web/src/routes/chunks.$chunkId.tsx` (add connection UI)

**Step 1: Add connection repository**

Create `packages/db/src/repository/connection.ts`:

```typescript
import { and, eq, or } from "drizzle-orm";
import { Effect } from "effect";

import { DatabaseError } from "../errors";
import { db } from "../index";
import { chunkConnection } from "../schema/chunk";

export function createConnection(params: { id: string; sourceId: string; targetId: string; relation: string }) {
    return Effect.tryPromise({
        try: async () => {
            const [created] = await db.insert(chunkConnection).values(params).returning();
            return created;
        },
        catch: cause => new DatabaseError({ cause })
    });
}

export function deleteConnection(connectionId: string) {
    return Effect.tryPromise({
        try: async () => {
            const [deleted] = await db.delete(chunkConnection).where(eq(chunkConnection.id, connectionId)).returning();
            return deleted ?? null;
        },
        catch: cause => new DatabaseError({ cause })
    });
}

export function getConnectionById(connectionId: string) {
    return Effect.tryPromise({
        try: async () => {
            const [found] = await db.select().from(chunkConnection).where(eq(chunkConnection.id, connectionId));
            return found ?? null;
        },
        catch: cause => new DatabaseError({ cause })
    });
}
```

**Step 2: Export from repository index**

Add to `packages/db/src/repository/index.ts`:

```typescript
export * from "./connection";
```

**Step 3: Add connection service**

Create `packages/api/src/connections/service.ts`:

```typescript
import {
    createConnection as createConnectionRepo,
    deleteConnection as deleteConnectionRepo,
    getChunkById,
    getConnectionById
} from "@fubbik/db/repository";
import { Effect } from "effect";

import { NotFoundError } from "../errors";

export function createConnection(userId: string, body: { sourceId: string; targetId: string; relation: string }) {
    return getChunkById(body.sourceId, userId).pipe(
        Effect.flatMap(source => (source ? Effect.succeed(source) : Effect.fail(new NotFoundError({ resource: "Source chunk" })))),
        Effect.flatMap(() => getChunkById(body.targetId, userId)),
        Effect.flatMap(target => (target ? Effect.succeed(target) : Effect.fail(new NotFoundError({ resource: "Target chunk" })))),
        Effect.flatMap(() =>
            createConnectionRepo({
                id: crypto.randomUUID(),
                sourceId: body.sourceId,
                targetId: body.targetId,
                relation: body.relation
            })
        )
    );
}

export function deleteConnection(connectionId: string, userId: string) {
    return getConnectionById(connectionId).pipe(
        Effect.flatMap(conn => (conn ? Effect.succeed(conn) : Effect.fail(new NotFoundError({ resource: "Connection" })))),
        // Verify user owns the source chunk
        Effect.flatMap(conn =>
            getChunkById(conn.sourceId, userId).pipe(
                Effect.flatMap(source => (source ? Effect.succeed(conn) : Effect.fail(new NotFoundError({ resource: "Connection" }))))
            )
        ),
        Effect.flatMap(conn => deleteConnectionRepo(conn.id))
    );
}
```

**Step 4: Add connection routes**

Create `packages/api/src/connections/routes.ts`:

```typescript
import { Effect } from "effect";
import { Elysia, t } from "elysia";

import { requireSession } from "../require-session";
import * as connectionService from "./service";

export const connectionRoutes = new Elysia()
    .post(
        "/connections",
        ctx =>
            Effect.runPromise(
                requireSession(ctx).pipe(
                    Effect.flatMap(session => connectionService.createConnection(session.user.id, ctx.body)),
                    Effect.tap(() =>
                        Effect.sync(() => {
                            ctx.set.status = 201;
                        })
                    )
                )
            ),
        {
            body: t.Object({
                sourceId: t.String(),
                targetId: t.String(),
                relation: t.String()
            })
        }
    )
    .delete("/connections/:id", ctx =>
        Effect.runPromise(
            requireSession(ctx).pipe(
                Effect.flatMap(session => connectionService.deleteConnection(ctx.params.id, session.user.id)),
                Effect.map(() => ({ message: "Deleted" }))
            )
        )
    );
```

**Step 5: Register routes in API index**

In `packages/api/src/index.ts`, add:

```typescript
import { connectionRoutes } from "./connections/routes";
```

And add `.use(connectionRoutes)` after `.use(chunkRoutes)`.

**Step 6: Add connection UI to chunk detail page**

In `apps/web/src/routes/chunks.$chunkId.tsx`, add a "Link Chunk" button in the connections section that opens a dialog. The dialog has:

- A search input to find chunks (calls `GET /api/chunks?search=...`)
- A relation type text input (default: "related")
- A submit button that calls `POST /api/connections`

Also add a delete button (X icon) on each existing connection that calls `DELETE /api/connections/:id`.

This is the most complex UI piece — use a `Dialog` component from shadcn-ui for the "Link Chunk" modal, and the existing `api` Eden client
for data fetching.

**Step 7: Run tests and type check**

Run: `cd /Users/pontus/GitHub/fubbik && bun run --filter @fubbik/api test && bun run check-types`

**Step 8: Commit**

```bash
git add packages/db/src/repository/connection.ts packages/db/src/repository/index.ts \
  packages/api/src/connections/ packages/api/src/index.ts \
  apps/web/src/routes/chunks.\$chunkId.tsx
git commit -m "feat: add connection CRUD — API endpoints and UI"
```

---

### Task 3: Search UI on Dashboard

The API supports `?search=` but the web app has no search interface. Add a command palette using the existing `command.tsx` shadcn
component.

**Files:**

- Create: `apps/web/src/features/search/chunk-search.tsx`
- Modify: `apps/web/src/routes/__root.tsx` (or layout — add global keyboard shortcut)

**Step 1: Create the search component**

Create `apps/web/src/features/search/chunk-search.tsx`. This is a command palette that:

- Opens with `Cmd+K` / `Ctrl+K`
- Has a search input that debounces and calls `GET /api/chunks?search=...`
- Shows matching chunks in a list
- Clicking a result navigates to `/chunks/$chunkId`

Use the `CommandDialog`, `CommandDialogTrigger`, `CommandInput`, `CommandList`, `CommandItem` etc. from `@/components/ui/command`.

```typescript
import { useQuery } from "@tanstack/react-query";
import { useNavigate } from "@tanstack/react-router";
import { Blocks, Search } from "lucide-react";
import { useEffect, useState } from "react";

import { Badge } from "@/components/ui/badge";
import {
    CommandDialog,
    CommandDialogBackdrop,
    CommandDialogPopup,
    CommandDialogPortal,
    CommandEmpty,
    CommandGroup,
    CommandInput,
    CommandItem,
    CommandList,
    CommandSeparator
} from "@/components/ui/command";
import { api } from "@/utils/api";

export function ChunkSearch() {
    const [open, setOpen] = useState(false);
    const [search, setSearch] = useState("");
    const navigate = useNavigate();

    // Cmd+K / Ctrl+K global shortcut
    useEffect(() => {
        const down = (e: KeyboardEvent) => {
            if (e.key === "k" && (e.metaKey || e.ctrlKey)) {
                e.preventDefault();
                setOpen(prev => !prev);
            }
        };
        document.addEventListener("keydown", down);
        return () => document.removeEventListener("keydown", down);
    }, []);

    const searchQuery = useQuery({
        queryKey: ["chunk-search", search],
        queryFn: async () => {
            if (!search.trim()) return { chunks: [], total: 0 };
            const { data, error } = await api.api.chunks.get({ query: { search, limit: "10" } });
            if (error) return { chunks: [], total: 0 };
            return data as Exclude<typeof data, { message: string }>;
        },
        enabled: search.length > 0
    });

    const chunks = searchQuery.data?.chunks ?? [];

    return (
        <CommandDialog open={open} onOpenChange={setOpen}>
            {/* ... command palette UI with input, list, items */}
            {/* Each item navigates to /chunks/$chunkId on select */}
            {/* Show chunk title, type badge, tags */}
        </CommandDialog>
    );
}
```

Note: The exact component API depends on the shadcn-ui command variant installed. Read `apps/web/src/components/ui/command.tsx` to
understand the available sub-components and adapt accordingly. The command component uses `@base-ui/react` Dialog + the Autocomplete
component.

**Step 2: Add to root layout**

In the root layout (find the layout component that wraps all routes — likely `__root.tsx`), add `<ChunkSearch />` so it's globally
available.

**Step 3: Add a search trigger button**

Add a search button to the dashboard header or nav bar that also opens the command palette. Show `Cmd+K` hint in the button.

**Step 4: Run type check**

Run: `cd /Users/pontus/GitHub/fubbik && bun run check-types`

**Step 5: Commit**

```bash
git add apps/web/src/features/search/ apps/web/src/routes/__root.tsx
git commit -m "feat: add global chunk search command palette (Cmd+K)"
```

---

### Task 4: Markdown Content Editor

Replace the plain textarea on create/edit pages with a markdown editor with preview.

**Files:**

- Create: `apps/web/src/features/editor/markdown-editor.tsx`
- Modify: `apps/web/src/routes/chunks.new.tsx` (use editor)
- Modify: `apps/web/src/routes/chunks.$chunkId.edit.tsx` (use editor)

**Step 1: Install a lightweight markdown editor**

Run: `cd apps/web && bun add @uiw/react-md-editor`

Alternative: Use a simple split-pane approach with `react-markdown` for preview and a plain textarea for editing. This avoids a heavy
dependency.

Simpler approach — build a tab-based editor:

```typescript
// apps/web/src/features/editor/markdown-editor.tsx
import { useState } from "react";

import { Tabs, TabsContent, TabsList, TabsTab } from "@/components/ui/tabs";

interface MarkdownEditorProps {
    value: string;
    onChange: (value: string) => void;
    placeholder?: string;
    rows?: number;
}

export function MarkdownEditor({ value, onChange, placeholder, rows = 10 }: MarkdownEditorProps) {
    return (
        <Tabs defaultValue="write">
            <TabsList>
                <TabsTab value="write">Write</TabsTab>
                <TabsTab value="preview">Preview</TabsTab>
            </TabsList>
            <TabsContent value="write">
                <textarea
                    value={value}
                    onChange={e => onChange(e.target.value)}
                    placeholder={placeholder}
                    rows={rows}
                    className="bg-background focus:ring-ring w-full resize-y rounded-md border px-3 py-2 text-sm focus:ring-2 focus:outline-none"
                />
            </TabsContent>
            <TabsContent value="preview">
                <div className="prose prose-invert prose-sm min-h-[200px] max-w-none rounded-md border p-3">
                    {/* Render markdown — reuse the same block parser from chunks.$chunkId.tsx */}
                    {/* Or install react-markdown for proper rendering */}
                    {value.split("\n\n").map((block, i) => {
                        if (block.startsWith("## ")) return <h2 key={i} className="mt-4 mb-2 text-base font-semibold">{block.replace("## ", "")}</h2>;
                        if (block.startsWith("- ")) return (
                            <ul key={i} className="list-disc space-y-1 pl-5 text-sm">
                                {block.split("\n").map((line, j) => <li key={j}>{line.replace(/^- /, "")}</li>)}
                            </ul>
                        );
                        return <p key={i} className="text-muted-foreground text-sm">{block}</p>;
                    })}
                </div>
            </TabsContent>
        </Tabs>
    );
}
```

**Step 2: Replace textarea in chunks.new.tsx and chunks.$chunkId.edit.tsx**

Replace the content `<textarea>` with `<MarkdownEditor value={content} onChange={setContent} />`.

**Step 3: Run type check and test manually**

**Step 4: Commit**

```bash
git add apps/web/src/features/editor/ apps/web/src/routes/chunks.new.tsx apps/web/src/routes/chunks.\$chunkId.edit.tsx
git commit -m "feat: add markdown editor with write/preview tabs"
```

---

### Task 5: Graph Visualization

The landing page promises "Knowledge Graphs." Add a `/graph` route that renders chunks as nodes and connections as edges using
`@xyflow/react`.

**Files:**

- Create: `apps/web/src/routes/graph.tsx`
- Modify: `apps/web/src/routes/dashboard.tsx` (add link to graph)

**Step 1: Install @xyflow/react**

Run: `cd apps/web && bun add @xyflow/react`

**Step 2: Add graph API endpoint**

Add a new endpoint `GET /api/graph` that returns all chunks (id, title, type) and all connections for the user. This is needed because the
existing endpoints are paginated.

Create `packages/api/src/graph/routes.ts`:

```typescript
import { Effect } from "effect";
import { Elysia } from "elysia";

import { requireSession } from "../require-session";
import * as graphService from "./service";

export const graphRoutes = new Elysia().get("/graph", ctx =>
    Effect.runPromise(requireSession(ctx).pipe(Effect.flatMap(session => graphService.getUserGraph(session.user.id))))
);
```

Create `packages/api/src/graph/service.ts` that fetches all chunks (id, title, type, tags) and all connections for the user. Add a
`getAllChunksMeta` repository function that returns lightweight chunk data (no content) and a `getAllConnectionsForUser` function.

**Step 3: Create the graph route**

Create `apps/web/src/routes/graph.tsx`:

```typescript
import { useQuery } from "@tanstack/react-query";
import { createFileRoute, redirect } from "@tanstack/react-router";
import { ReactFlow, Background, Controls, type Node, type Edge } from "@xyflow/react";
import "@xyflow/react/dist/style.css";

import { getUser } from "@/functions/get-user";
import { api } from "@/utils/api";

export const Route = createFileRoute("/graph")({
    component: GraphView,
    beforeLoad: async () => {
        try {
            const session = await getUser();
            return { session };
        } catch {
            throw redirect({ to: "/login" });
        }
    }
});

function GraphView() {
    const { data, isLoading } = useQuery({
        queryKey: ["graph"],
        queryFn: async () => {
            const { data, error } = await api.api.graph.get();
            if (error) throw new Error("Failed to load graph");
            return data;
        }
    });

    // Convert chunks to nodes, connections to edges
    // Use force-directed layout or dagre for auto-layout
    // Each node shows chunk title and type badge
    // Clicking a node navigates to /chunks/$chunkId
    // Edges show relation label

    const nodes: Node[] = (data?.chunks ?? []).map((chunk, i) => ({
        id: chunk.id,
        data: { label: chunk.title },
        position: { x: (i % 5) * 250, y: Math.floor(i / 5) * 150 }
    }));

    const edges: Edge[] = (data?.connections ?? []).map(conn => ({
        id: conn.id,
        source: conn.sourceId,
        target: conn.targetId,
        label: conn.relation
    }));

    if (isLoading) return <div className="flex h-[calc(100vh-4rem)] items-center justify-center"><p>Loading graph...</p></div>;

    return (
        <div className="h-[calc(100vh-4rem)]">
            <ReactFlow nodes={nodes} edges={edges} fitView>
                <Background />
                <Controls />
            </ReactFlow>
        </div>
    );
}
```

**Step 4: Add nav link from dashboard**

Add a "View Graph" button to the dashboard header next to "New Chunk".

**Step 5: Register graph routes in API**

Add `import { graphRoutes } from "./graph/routes"` and `.use(graphRoutes)` in `packages/api/src/index.ts`.

**Step 6: Run type check and manual test**

**Step 7: Commit**

```bash
git add apps/web/src/routes/graph.tsx packages/api/src/graph/ packages/api/src/index.ts \
  packages/db/src/repository/ apps/web/src/routes/dashboard.tsx
git commit -m "feat: add knowledge graph visualization with @xyflow/react"
```

---

### Task 6: AI-Powered Features

The Vercel AI SDK (`@ai-sdk/react`) is already installed in the web app. Add AI features: summarize a chunk, suggest connections, and
generate a chunk from a prompt.

**Files:**

- Create: `packages/api/src/ai/routes.ts`
- Create: `packages/api/src/ai/service.ts`
- Modify: `packages/api/src/index.ts` (use aiRoutes)
- Modify: `apps/server/package.json` (add `@ai-sdk/openai` or chosen provider)
- Modify: `packages/env/src/server.ts` (add `OPENAI_API_KEY`)
- Modify: `apps/web/src/routes/chunks.$chunkId.tsx` (add AI buttons)

**Step 1: Install AI provider on server**

Run: `cd apps/server && bun add ai @ai-sdk/openai`

Add `OPENAI_API_KEY` to `packages/env/src/server.ts` as an optional env var (not all users will have it).

**Step 2: Create AI service**

Create `packages/api/src/ai/service.ts`:

```typescript
import { generateText } from "ai";
import { openai } from "@ai-sdk/openai";
import { Effect } from "effect";
import { DatabaseError } from "@fubbik/db/errors";

export function summarizeChunk(title: string, content: string) {
    return Effect.tryPromise({
        try: async () => {
            const result = await generateText({
                model: openai("gpt-4o-mini"),
                prompt: `Summarize this knowledge chunk concisely:\n\nTitle: ${title}\n\nContent: ${content}`,
                maxTokens: 200
            });
            return { summary: result.text };
        },
        catch: cause => new DatabaseError({ cause }) // Consider a new AiError tagged error
    });
}

export function suggestConnections(chunkTitle: string, chunkContent: string, otherChunks: { id: string; title: string }[]) {
    return Effect.tryPromise({
        try: async () => {
            const chunkList = otherChunks.map(c => `- ${c.id}: ${c.title}`).join("\n");
            const result = await generateText({
                model: openai("gpt-4o-mini"),
                prompt: `Given this chunk:\nTitle: ${chunkTitle}\nContent: ${chunkContent}\n\nSuggest which of these chunks it should be connected to and why:\n${chunkList}\n\nReturn JSON array: [{"id": "...", "relation": "..."}]`,
                maxTokens: 500
            });
            return JSON.parse(result.text) as { id: string; relation: string }[];
        },
        catch: cause => new DatabaseError({ cause })
    });
}

export function generateChunk(prompt: string) {
    return Effect.tryPromise({
        try: async () => {
            const result = await generateText({
                model: openai("gpt-4o-mini"),
                prompt: `Generate a knowledge chunk based on this prompt. Return JSON with title, content, type (one of: note, document, reference, schema, checklist), and tags (array of strings):\n\n${prompt}`,
                maxTokens: 1000
            });
            return JSON.parse(result.text) as { title: string; content: string; type: string; tags: string[] };
        },
        catch: cause => new DatabaseError({ cause })
    });
}
```

**Step 3: Create AI routes**

Create `packages/api/src/ai/routes.ts` with:

- `POST /ai/summarize` — takes `chunkId`, returns summary
- `POST /ai/suggest-connections` — takes `chunkId`, returns suggested connections
- `POST /ai/generate` — takes `prompt`, returns generated chunk fields

**Step 4: Add AI buttons to chunk detail page**

Add a "Summarize" button and "Suggest Connections" button to the chunk detail page. Show results in a card below the content.

**Step 5: Add "Generate with AI" option to new chunk page**

Add a text input + button above the form: "Describe what you want and AI will generate a chunk."

**Step 6: Register routes, run tests, commit**

```bash
git add packages/api/src/ai/ packages/api/src/index.ts apps/server/package.json \
  packages/env/src/server.ts apps/web/src/routes/chunks.\$chunkId.tsx \
  apps/web/src/routes/chunks.new.tsx
git commit -m "feat: add AI features — summarize, suggest connections, generate chunks"
```

---

### Task 7: Full-Text Search with pg_trgm

Replace the simple ILIKE search with PostgreSQL trigram-based fuzzy search for better results on typos and partial matches.

**Files:**

- Create: `packages/db/src/migrations/add-trgm-index.sql` (or use drizzle push)
- Modify: `packages/db/src/repository/chunk.ts` (update search query)

**Step 1: Enable pg_trgm extension**

Add a migration or drizzle custom SQL to enable the extension:

```sql
CREATE EXTENSION IF NOT EXISTS pg_trgm;
```

**Step 2: Add GIN trigram indexes**

```sql
CREATE INDEX IF NOT EXISTS chunk_title_trgm_idx ON chunk USING gin (title gin_trgm_ops);
CREATE INDEX IF NOT EXISTS chunk_content_trgm_idx ON chunk USING gin (content gin_trgm_ops);
```

These can be added to the Drizzle schema using `sql` in the table indexes, or applied via `db:push` after adding the extension manually.

**Step 3: Update search query in repository**

In `packages/db/src/repository/chunk.ts`, update the search condition in `listChunks`:

```typescript
// Before:
if (params.search) {
    conditions.push(or(ilike(chunk.title, `%${params.search}%`), ilike(chunk.content, `%${params.search}%`))!);
}

// After — use similarity for ranking + trigram matching:
if (params.search) {
    conditions.push(
        sql`(${chunk.title} % ${params.search} OR ${chunk.content} % ${params.search} OR ${chunk.title} ILIKE ${"%" + params.search + "%"})`
    );
    // Also add ORDER BY similarity for better ranking
}
```

The `%` operator is the trigram similarity operator. Combine with ILIKE as fallback for exact substring matches.

**Step 4: Test with the search API**

Run: `bun dev`, create some chunks, test search with typos.

**Step 5: Commit**

```bash
git add packages/db/
git commit -m "feat: add pg_trgm fuzzy search with GIN indexes"
```

---

### Task 8: Chunk Filtering & Sorting UI

The dashboard shows recent chunks but has no filter/sort controls.

**Files:**

- Create: `apps/web/src/routes/chunks.tsx` (dedicated chunks list page with filters)
- Modify: `apps/web/src/routes/dashboard.tsx` (add "View All" link)

**Step 1: Create a dedicated chunks list page**

Create `apps/web/src/routes/chunks.tsx` with:

- Search input at the top
- Type filter dropdown (All, Note, Document, Reference, Schema, Checklist)
- Tag filter (show popular tags as badges, click to filter)
- Sort options (Newest, Oldest, A-Z)
- Paginated chunk list using the existing `GET /api/chunks` endpoint with query params

Use `useSearch` from TanStack Router to read/write URL search params so filters are bookmarkable.

```typescript
import { createFileRoute, redirect } from "@tanstack/react-router";

export const Route = createFileRoute("/chunks")({
    component: ChunksList,
    validateSearch: (search: Record<string, unknown>) => ({
        type: (search.type as string) || undefined,
        search: (search.search as string) || undefined,
        sort: (search.sort as string) || "newest",
        page: Number(search.page) || 1
    }),
    beforeLoad: async () => {
        try {
            const session = await getUser();
            return { session };
        } catch {
            throw redirect({ to: "/login" });
        }
    }
});
```

**Step 2: Add "View All" link to dashboard**

In dashboard's "Recent Chunks" header, add a link to `/chunks`.

**Step 3: Run type check and manual test**

**Step 4: Commit**

```bash
git add apps/web/src/routes/chunks.tsx apps/web/src/routes/dashboard.tsx
git commit -m "feat: add chunks list page with type/search filters and sorting"
```

---

### Task 9: Bulk Import/Export

Add JSON export of all chunks and import from JSON. Useful for backup and CLI<->server sync.

**Files:**

- Create: `packages/api/src/chunks/export.ts` (export service)
- Modify: `packages/api/src/chunks/routes.ts` (add export/import endpoints)
- Modify: `apps/web/src/routes/dashboard.tsx` or settings page (add export/import buttons)

**Step 1: Add export endpoint**

Add `GET /api/chunks/export` that returns all chunks as a JSON array (no pagination):

```typescript
// In chunks/routes.ts
.get("/chunks/export", ctx =>
    Effect.runPromise(
        requireSession(ctx).pipe(
            Effect.flatMap(session => chunkService.exportChunks(session.user.id))
        )
    )
)
```

Add `exportChunks` to the service that fetches all chunks without limit.

**Step 2: Add import endpoint**

Add `POST /api/chunks/import` that accepts an array of chunks and creates them:

```typescript
.post("/chunks/import", ctx =>
    Effect.runPromise(
        requireSession(ctx).pipe(
            Effect.flatMap(session => chunkService.importChunks(session.user.id, ctx.body.chunks))
        )
    ),
    {
        body: t.Object({
            chunks: t.Array(t.Object({
                title: t.String(),
                content: t.Optional(t.String()),
                type: t.Optional(t.String()),
                tags: t.Optional(t.Array(t.String()))
            }))
        })
    }
)
```

The import service assigns new IDs to avoid conflicts and skips chunks with duplicate titles.

**Step 3: Add UI buttons**

Add "Export" and "Import" buttons to the dashboard. Export triggers a file download. Import opens a file picker.

**Step 4: Commit**

```bash
git add packages/api/src/chunks/ apps/web/src/routes/dashboard.tsx
git commit -m "feat: add bulk import/export for chunks as JSON"
```

---

### Task 10: Version History

Track content changes per chunk with a `chunk_version` table.

**Files:**

- Create: `packages/db/src/schema/chunk-version.ts`
- Modify: `packages/db/src/schema/index.ts` (export)
- Create: `packages/db/src/repository/chunk-version.ts`
- Modify: `packages/db/src/repository/index.ts` (export)
- Modify: `packages/api/src/chunks/service.ts` (snapshot on update)
- Modify: `packages/api/src/chunks/routes.ts` (add history endpoint)
- Modify: `apps/web/src/routes/chunks.$chunkId.tsx` (show history)

**Step 1: Create version schema**

Create `packages/db/src/schema/chunk-version.ts`:

```typescript
import { pgTable, text, timestamp, integer, jsonb } from "drizzle-orm/pg-core";
import { chunk } from "./chunk";

export const chunkVersion = pgTable("chunk_version", {
    id: text("id").primaryKey(),
    chunkId: text("chunk_id")
        .notNull()
        .references(() => chunk.id, { onDelete: "cascade" }),
    version: integer("version").notNull(),
    title: text("title").notNull(),
    content: text("content").notNull(),
    type: text("type").notNull(),
    tags: jsonb("tags").$type<string[]>().notNull(),
    createdAt: timestamp("created_at").defaultNow().notNull()
});
```

**Step 2: Add repository functions**

- `createVersion(params)` — insert a snapshot
- `getVersions(chunkId)` — list versions ordered by version desc

**Step 3: Snapshot on update**

In `packages/api/src/chunks/service.ts`, modify `updateChunk` to first snapshot the current state before applying the update:

```typescript
export function updateChunk(chunkId: string, userId: string, body: UpdateBody) {
    return getChunkById(chunkId, userId).pipe(
        Effect.flatMap(existing => existing ? Effect.succeed(existing) : Effect.fail(new NotFoundError({ resource: "Chunk" }))),
        Effect.flatMap(existing =>
            // Snapshot current state
            createVersionRepo({
                id: crypto.randomUUID(),
                chunkId,
                version: /* get next version number */,
                title: existing.title,
                content: existing.content,
                type: existing.type,
                tags: existing.tags as string[]
            }).pipe(Effect.flatMap(() => updateChunkRepo(chunkId, body)))
        )
    );
}
```

**Step 4: Add history endpoint**

Add `GET /api/chunks/:id/history` that returns version list.

**Step 5: Add history UI**

Add a collapsible "History" section on the chunk detail page showing version timestamps. Click to view a previous version.

**Step 6: Run db:push, test, commit**

```bash
bun db:push  # creates chunk_version table
git add packages/db/src/schema/ packages/db/src/repository/ \
  packages/api/src/chunks/ apps/web/src/routes/chunks.\$chunkId.tsx
git commit -m "feat: add version history — snapshot on update, history view"
```

---

### Task 11: CLI <-> Server Sync

The CLI uses a local JSON store, completely separate from the server DB. Add a `fubbik sync` command.

**Files:**

- Create: `apps/cli/src/commands/sync.ts`
- Modify: `apps/cli/src/index.ts` (register command)
- Modify: `apps/cli/src/lib/store.ts` (add sync metadata)

**Step 1: Add sync metadata to store**

Update the store schema to track `serverId` per chunk and `lastSync` timestamp:

```typescript
interface Chunk {
    id: string;
    serverId?: string; // ID on the server, if synced
    title: string;
    content: string;
    type: string;
    tags: string[];
    createdAt: string;
    updatedAt: string;
}

interface Store {
    name: string;
    chunks: Chunk[];
    serverUrl?: string;
    lastSync?: string;
}
```

**Step 2: Create sync command**

Create `apps/cli/src/commands/sync.ts`:

```typescript
// fubbik sync [--url <server-url>] [--push | --pull]
// Default: bidirectional — push local-only chunks, pull server-only chunks
// --push: only push local to server
// --pull: only pull server to local

// Uses the same API endpoints:
// GET /api/chunks — list server chunks
// POST /api/chunks — create chunk on server
// POST /api/chunks/import — bulk import (from Task 9)
```

The sync logic:

1. Fetch all server chunks
2. Compare by title (since IDs differ between local and server)
3. Push local-only chunks to server
4. Pull server-only chunks to local store
5. For conflicts (same title, different content), use `updatedAt` to pick the newer version

**Step 3: Register command**

Add `program.addCommand(syncCommand)` in `apps/cli/src/index.ts`.

**Step 4: Test manually**

**Step 5: Commit**

```bash
git add apps/cli/src/commands/sync.ts apps/cli/src/index.ts apps/cli/src/lib/store.ts
git commit -m "feat: add CLI sync command for local <-> server chunk sync"
```

---

### Task 12: Tags Page

Add a `/tags` route that shows all tags with chunk counts and allows filtering.

**Files:**

- Create: `packages/db/src/repository/tags.ts`
- Modify: `packages/db/src/repository/index.ts` (export)
- Create: `packages/api/src/tags/routes.ts`
- Create: `packages/api/src/tags/service.ts`
- Modify: `packages/api/src/index.ts` (use tagRoutes)
- Create: `apps/web/src/routes/tags.tsx`
- Modify: `apps/web/src/routes/dashboard.tsx` (link to tags)

**Step 1: Add tags repository**

Create `packages/db/src/repository/tags.ts`:

```typescript
import { eq, sql } from "drizzle-orm";
import { Effect } from "effect";

import { DatabaseError } from "../errors";
import { db } from "../index";
import { chunk } from "../schema/chunk";

export function getTagsWithCounts(userId: string) {
    return Effect.tryPromise({
        try: async () => {
            // Unnest jsonb tags array and count occurrences
            const result = await db.execute(sql`
                SELECT tag, COUNT(*) as count
                FROM ${chunk}, jsonb_array_elements_text(${chunk.tags}) AS tag
                WHERE ${chunk.userId} = ${userId}
                GROUP BY tag
                ORDER BY count DESC
            `);
            return result.rows as { tag: string; count: number }[];
        },
        catch: cause => new DatabaseError({ cause })
    });
}
```

**Step 2: Add tags service and routes**

Create `packages/api/src/tags/service.ts` and `routes.ts` following the same pattern:

```typescript
// routes.ts
export const tagRoutes = new Elysia().get("/tags", ctx =>
    Effect.runPromise(requireSession(ctx).pipe(Effect.flatMap(session => tagService.getUserTags(session.user.id))))
);
```

**Step 3: Create the tags page**

Create `apps/web/src/routes/tags.tsx`:

- Grid of tag badges with counts
- Clicking a tag navigates to `/chunks?tag=<tag>` (from Task 8)
- Shows total unique tags

**Step 4: Add link from dashboard**

Make the "Tags" stat card on the dashboard clickable, linking to `/tags`.

**Step 5: Register routes, run type check, commit**

```bash
git add packages/db/src/repository/tags.ts packages/db/src/repository/index.ts \
  packages/api/src/tags/ packages/api/src/index.ts \
  apps/web/src/routes/tags.tsx apps/web/src/routes/dashboard.tsx
git commit -m "feat: add tags page with counts and filtering"
```
