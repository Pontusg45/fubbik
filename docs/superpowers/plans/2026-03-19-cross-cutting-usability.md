# Cross-Cutting Usability Improvements Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement cross-cutting usability improvements: offline indicator, consistent empty states, and export enhancements — features that span multiple surfaces.

**Architecture:** Offline indicator is a web UI component using navigator.onLine + server health polling. Export improvements add new API query params and UI options. These are small, focused changes.

**Tech Stack:** React, TanStack Query, sonner, Elysia API

---

## File Structure

### New files to create:
- `apps/web/src/features/nav/connection-status.tsx` — Offline/online indicator component

### Files to modify:
- `apps/web/src/routes/__root.tsx` — Add connection status indicator
- `apps/web/src/routes/dashboard.tsx` — Improve export with format options
- `apps/web/src/routes/chunks.$chunkId.tsx` — Add "Export as Markdown" action
- `packages/api/src/chunks/routes.ts` — Add markdown export format option to export endpoint

---

## Task 1: Connection Status Indicator

Show a subtle banner when the server is unreachable, and recover gracefully when it comes back.

**Files:**
- Create: `apps/web/src/features/nav/connection-status.tsx`
- Modify: `apps/web/src/routes/__root.tsx`

- [ ] **Step 1: Write test for ConnectionStatus**

```tsx
// apps/web/src/__tests__/connection-status.test.tsx
import { render, screen } from "@testing-library/react";
import { describe, it, expect, vi, beforeEach } from "vitest";

describe("ConnectionStatus", () => {
  it("shows nothing when online", () => {
    // Mock fetch to resolve
    vi.spyOn(globalThis, "fetch").mockResolvedValue(new Response("ok"));
    // Component should render null or empty
  });

  it("shows offline banner when server unreachable", () => {
    vi.spyOn(globalThis, "fetch").mockRejectedValue(new Error("Network error"));
    // Component should show "Server unreachable" message
  });
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cd apps/web && pnpm vitest run src/__tests__/connection-status.test.tsx`
Expected: FAIL

- [ ] **Step 3: Implement ConnectionStatus component**

```tsx
// apps/web/src/features/nav/connection-status.tsx
import { useQuery } from "@tanstack/react-query";
import { WifiOff } from "lucide-react";

export function ConnectionStatus() {
  const { isError } = useQuery({
    queryKey: ["health"],
    queryFn: async () => {
      const res = await fetch("/api/health");
      if (!res.ok) throw new Error("Server error");
      return res.json();
    },
    refetchInterval: 30_000, // check every 30s
    retry: false,
  });

  if (!isError) return null;

  return (
    <div className="bg-destructive/10 text-destructive text-xs px-3 py-1.5 text-center flex items-center justify-center gap-1.5">
      <WifiOff className="h-3 w-3" />
      Server unreachable — some features may be unavailable
    </div>
  );
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cd apps/web && pnpm vitest run src/__tests__/connection-status.test.tsx`
Expected: PASS

- [ ] **Step 5: Add to root layout**

In `__root.tsx`, add `<ConnectionStatus />` just below the header nav (before `<main>`).

- [ ] **Step 6: Commit**

```bash
git add apps/web/src/features/nav/connection-status.tsx apps/web/src/__tests__/connection-status.test.tsx apps/web/src/routes/__root.tsx
git commit -m "feat(web): add connection status indicator for server health"
```

---

## Task 2: Export Improvements

Add markdown export for individual chunks and filtered subsets.

**Files:**
- Modify: `packages/api/src/chunks/routes.ts` — Add `format=markdown` query param to export
- Modify: `apps/web/src/routes/chunks.$chunkId.tsx` — Add "Export as Markdown" button
- Modify: `apps/web/src/routes/dashboard.tsx` — Add format selector to export

- [ ] **Step 1: Add markdown format to export endpoint**

In `packages/api/src/chunks/routes.ts`, modify the GET `/chunks/export` handler. **Important:** Add an Elysia query schema to validate the `format` param:
```ts
// Add query param: format ("json" | "markdown")
// Must add query schema: { query: t.Object({ format: t.Optional(t.String()) }) }
.get("/export", async ({ query }) => {
  const chunks = await Effect.runPromise(/* existing logic */);

  if (query.format === "markdown") {
    const md = chunks.map((c) => [
      `# ${c.title}`,
      "",
      `**Type:** ${c.type}`,
      c.tags?.length ? `**Tags:** ${c.tags.join(", ")}` : "",
      "",
      c.content,
      "",
      "---",
      "",
    ].filter(Boolean).join("\n")).join("\n");
    return new Response(md, {
      headers: { "Content-Type": "text/markdown", "Content-Disposition": "attachment; filename=fubbik-export.md" },
    });
  }

  return chunks; // default JSON
})
```

- [ ] **Step 2: Add "Export as Markdown" to chunk detail**

In `chunks.$chunkId.tsx`, add a button in the actions area (~line 160-217):
```tsx
<Button variant="outline" size="sm" onClick={() => {
  const md = `# ${chunk.title}\n\n**Type:** ${chunk.type}\n**Tags:** ${(chunk.tags || []).join(", ")}\n\n${chunk.content}`;
  const blob = new Blob([md], { type: "text/markdown" });
  const url = URL.createObjectURL(blob);
  const a = document.createElement("a");
  a.href = url;
  a.download = `${chunk.title.replace(/[^a-z0-9]/gi, "-").toLowerCase()}.md`;
  a.click();
  URL.revokeObjectURL(url);
}}>
  Export MD
</Button>
```

- [ ] **Step 3: Add format toggle to dashboard export**

In `dashboard.tsx`, modify the export button (~line 148-169) to offer JSON or Markdown via a dropdown:
```tsx
<DropdownMenu>
  <DropdownMenuTrigger asChild>
    <Button variant="outline" size="sm">Export</Button>
  </DropdownMenuTrigger>
  <DropdownMenuContent>
    <DropdownMenuItem onClick={exportJson}>Export as JSON</DropdownMenuItem>
    <DropdownMenuItem onClick={exportMarkdown}>Export as Markdown</DropdownMenuItem>
  </DropdownMenuContent>
</DropdownMenu>
```

- [ ] **Step 4: Test manually**

- Chunk detail → "Export MD" → downloads .md file with chunk content
- Dashboard → Export → Markdown → downloads full knowledge base as markdown
- Dashboard → Export → JSON → existing behavior unchanged

- [ ] **Step 5: Commit**

```bash
git add packages/api/src/chunks/routes.ts apps/web/src/routes/chunks.\$chunkId.tsx apps/web/src/routes/dashboard.tsx
git commit -m "feat: add markdown export for chunks and knowledge base"
```

---

## Task 3: Consistent Empty States Across All Pages

Audit and standardize empty states for every list page using the existing Empty component system.

**Files:**
- Modify: `apps/web/src/routes/tags.tsx`
- Modify: `apps/web/src/routes/codebases.tsx`
- Modify: `apps/web/src/routes/templates.tsx`
- Modify: `apps/web/src/routes/vocabulary.tsx`
- Modify: `apps/web/src/routes/activity.tsx`
- Modify: `apps/web/src/routes/chunks.archived.tsx`

- [ ] **Step 1: Audit current empty states**

Check each list page for how it handles zero items. Document which ones have proper empty states and which just show blank or "Loading...".

- [ ] **Step 2: Create a mapping of empty states**

| Page | Icon | Title | Description | Action |
|------|------|-------|-------------|--------|
| Tags | `Tag` | No tags yet | Tags help categorize and filter chunks. | Create Tag |
| Codebases | `GitBranch` | No codebases | Add a codebase to scope chunks to specific projects. | Add Codebase |
| Templates | `LayoutTemplate` | No custom templates | Templates pre-fill chunk forms for common patterns. | Create Template |
| Vocabulary | `BookOpen` | No vocabulary | Define domain terms to standardize your knowledge base. | Add Term |
| Activity | `Activity` | No activity yet | Actions on chunks and connections will appear here. | — |
| Archived | `Archive` | No archived chunks | Archived chunks can be restored at any time. | — |

- [ ] **Step 3: Implement empty states**

**Note:** `EmptyAction` does not exist yet in `empty.tsx`. First add it:
```tsx
// In apps/web/src/components/ui/empty.tsx
export function EmptyAction({ children }: { children: React.ReactNode }) {
  return <div className="mt-4">{children}</div>;
}
```

Then, for each page, wrap the "no items" case with:
```tsx
<Empty>
  <EmptyMedia variant="icon"><IconComponent className="h-10 w-10" /></EmptyMedia>
  <EmptyTitle>{title}</EmptyTitle>
  <EmptyDescription>{description}</EmptyDescription>
  {action && (
    <EmptyAction>
      <Button asChild><Link to={actionUrl}>{actionLabel}</Link></Button>
    </EmptyAction>
  )}
</Empty>
```

- [ ] **Step 4: Verify visually**

Navigate to each page with zero items. Verify the empty state renders with icon, text, and action button.

- [ ] **Step 5: Commit**

```bash
git add apps/web/src/routes/ apps/web/src/components/ui/empty.tsx
git commit -m "feat(web): standardize empty states across all list pages"
```
