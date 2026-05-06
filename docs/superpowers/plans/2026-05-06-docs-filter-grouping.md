# Docs Page Filter & Grouping Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add tag/type filtering, folder/tag grouping, and saveable filter presets to the existing `/docs` document browser page.

**Architecture:** Enrich the `listDocuments` repository query to include tags and chunk type per document. Add a pure client-side filter/group module (`filter-documents.ts`). Build a compact filter bar component stacked above the existing folder tree sidebar. Modify `document-browser.tsx` to wire filter state through URL params, apply filtering/grouping via `useMemo`, and render tag groups when selected.

**Tech Stack:** Drizzle (repository query joins), Effect (service layer), React (filter bar component, useMemo), TanStack Router (URL search params), localStorage (presets)

---

### Task 1: Enrich `listDocuments` with tags and type

**Files:**
- Modify: `packages/db/src/repository/document.ts:45-70`
- Test: `packages/api/src/documents/service.test.ts` (existing, add test)

- [ ] **Step 1: Write a failing test**

Create or append to `packages/api/src/documents/service.test.ts`. Since the repository is hard to unit test (needs DB), we'll test the enrichment at the service level with a simple integration-style check. Actually — since this project uses `vi.mock` at the repository boundary, add a test for the new service function:

Add to the end of the existing `packages/api/src/documents/service.test.ts`:

```typescript
describe("listDocumentsEnriched", () => {
    it("returns splitMarkdown sections with tags from the content", () => {
        const md = `---\ntags:\n  - guide\n---\n\n# Getting Started\n\n## Setup\n\nSteps here.\n`;
        const result = splitMarkdown(md, "docs/getting-started.md");
        expect(result.tags).toContain("guide");
        expect(result.tags).toContain("docs");
        expect(result.tags).toContain("getting started");
    });
});
```

Wait — the enrichment is a repository-level SQL change. We can't easily unit test the SQL join. Instead, let's write the implementation first and verify with the full test suite + type-check. Skip TDD for this database query change.

- [ ] **Step 2: Add `listDocumentsWithTags` to the document repository**

In `packages/db/src/repository/document.ts`, add these imports at the top:

```typescript
import { chunk } from "../schema/chunk";
import { chunkTag } from "../schema/tag";
import { tag } from "../schema/tag";
```

Check which are already imported — `chunk` is already imported. Add `chunkTag` and `tag` if missing.

Then add this new function after `listDocuments`:

```typescript
export function listDocumentsWithTags(userId: string, codebaseId?: string) {
    return dbEffect(async () => {
        const conditions = [eq(document.userId, userId)];
        if (codebaseId) conditions.push(eq(document.codebaseId, codebaseId));

        const docs = await db
            .select({
                id: document.id,
                title: document.title,
                sourcePath: document.sourcePath,
                contentHash: document.contentHash,
                description: document.description,
                codebaseId: document.codebaseId,
                createdAt: document.createdAt,
                updatedAt: document.updatedAt,
                chunkCount: sql<number>`count(distinct ${chunk.id})`.as("chunk_count"),
                lastChunkUpdatedAt: sql<Date>`max(${chunk.updatedAt})`.as("last_chunk_updated_at"),
                oldestChunkUpdatedAt: sql<Date>`min(${chunk.updatedAt})`.as("oldest_chunk_updated_at"),
                type: sql<string>`min(case when ${chunk.documentOrder} = 0 then ${chunk.type} end)`.as("type"),
                tagsRaw: sql<string>`string_agg(distinct ${tag.name}, ',')`.as("tags_raw"),
            })
            .from(document)
            .leftJoin(chunk, eq(chunk.documentId, document.id))
            .leftJoin(chunkTag, eq(chunkTag.chunkId, chunk.id))
            .leftJoin(tag, eq(tag.id, chunkTag.tagId))
            .where(and(...conditions))
            .groupBy(document.id)
            .orderBy(document.title);

        return docs.map(d => ({
            ...d,
            type: d.type ?? "document",
            tags: d.tagsRaw ? d.tagsRaw.split(",").filter(Boolean) : [],
            tagsRaw: undefined,
        }));
    });
}
```

- [ ] **Step 3: Export the new function**

The repository file's exports are picked up automatically by `packages/db/src/repository/index.ts` via `export * from "./document"`. Verify the function is accessible.

- [ ] **Step 4: Add a service wrapper and update the route**

In `packages/api/src/documents/service.ts`, the existing `listDocuments` function delegates to `listDocumentsRepo`. Add an import for the new function:

```typescript
import {
    // ... existing imports ...
    listDocumentsWithTags as listDocumentsWithTagsRepo,
} from "@fubbik/db/repository";
```

Add a new service function:

```typescript
export function listDocumentsWithTags(userId: string, codebaseId?: string) {
    return listDocumentsWithTagsRepo(userId, codebaseId);
}
```

In `packages/api/src/documents/routes.ts`, update the `GET /documents` handler to use the enriched version:

```typescript
    .get(
        "/documents",
        ctx =>
            Effect.runPromise(
                requireSession(ctx).pipe(
                    Effect.flatMap(session =>
                        documentService.listDocumentsWithTags(session.user.id, ctx.query.codebaseId)
                    )
                )
            ),
        {
            query: t.Object({
                codebaseId: t.Optional(t.String())
            })
        }
    )
```

- [ ] **Step 5: Run type-check and tests**

Run: `pnpm run check-types && pnpm test`
Expected: All pass. The enriched response is a superset of the old one (adds `type` and `tags` fields).

- [ ] **Step 6: Commit**

```bash
git add packages/db/src/repository/document.ts packages/api/src/documents/service.ts packages/api/src/documents/routes.ts
git commit -m "feat: enrich listDocuments with tags and chunk type"
```

---

### Task 2: Pure filter and group functions

**Files:**
- Create: `apps/web/src/features/documents/filter-documents.ts`
- Create: `apps/web/src/features/documents/filter-documents.test.ts`

- [ ] **Step 1: Write failing tests**

Create `apps/web/src/features/documents/filter-documents.test.ts`:

```typescript
import { describe, expect, it } from "vitest";
import { filterDocuments, groupDocuments, type EnrichedDocument } from "./filter-documents";

const docs: EnrichedDocument[] = [
    { id: "1", title: "Auth Guide", sourcePath: "docs/guides/auth.md", tags: ["auth", "guides"], type: "document", chunkCount: 3 },
    { id: "2", title: "API Endpoints", sourcePath: "docs/api/endpoints.md", tags: ["api", "reference"], type: "reference", chunkCount: 5 },
    { id: "3", title: "Architecture", sourcePath: "docs/architecture.md", tags: ["backend"], type: "document", chunkCount: 4 },
    { id: "4", title: "Errors", sourcePath: "docs/api/errors.md", tags: [], type: "document", chunkCount: 2 },
];

describe("filterDocuments", () => {
    it("returns all documents when no filters active", () => {
        const result = filterDocuments(docs, { activeTags: [], activeTypes: [] });
        expect(result).toHaveLength(4);
    });

    it("filters by tags (OR within tags)", () => {
        const result = filterDocuments(docs, { activeTags: ["auth", "api"], activeTypes: [] });
        expect(result.map(d => d.id)).toEqual(["1", "2"]);
    });

    it("filters by types (OR within types)", () => {
        const result = filterDocuments(docs, { activeTags: [], activeTypes: ["reference"] });
        expect(result.map(d => d.id)).toEqual(["2"]);
    });

    it("AND between dimensions", () => {
        const result = filterDocuments(docs, { activeTags: ["auth", "api"], activeTypes: ["document"] });
        expect(result.map(d => d.id)).toEqual(["1"]);
    });

    it("empty result when no match", () => {
        const result = filterDocuments(docs, { activeTags: ["nonexistent"], activeTypes: [] });
        expect(result).toHaveLength(0);
    });
});

describe("groupDocuments", () => {
    it("groups by folder", () => {
        const groups = groupDocuments(docs, "folder");
        expect(groups.get("docs/guides")).toHaveLength(1);
        expect(groups.get("docs/api")).toHaveLength(2);
        expect(groups.get("docs")).toHaveLength(1);
    });

    it("groups by tag", () => {
        const groups = groupDocuments(docs, "tag");
        expect(groups.get("auth")).toHaveLength(1);
        expect(groups.get("api")).toHaveLength(1);
        expect(groups.get("reference")).toHaveLength(1);
        expect(groups.get("backend")).toHaveLength(1);
        expect(groups.get("guides")).toHaveLength(1);
        expect(groups.get("Untagged")).toHaveLength(1);
    });

    it("duplicates multi-tagged docs across tag groups", () => {
        const groups = groupDocuments([docs[0]!], "tag");
        expect(groups.get("auth")).toHaveLength(1);
        expect(groups.get("guides")).toHaveLength(1);
    });
});
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `pnpm --filter web test -- --reporter verbose src/features/documents/filter-documents.test.ts`
Expected: Fails — module not found.

- [ ] **Step 3: Implement the pure functions**

Create `apps/web/src/features/documents/filter-documents.ts`:

```typescript
export interface EnrichedDocument {
    id: string;
    title: string;
    sourcePath: string;
    tags: string[];
    type: string;
    chunkCount: number;
    [key: string]: unknown;
}

export interface DocFilters {
    activeTags: string[];
    activeTypes: string[];
}

export function filterDocuments(
    documents: EnrichedDocument[],
    filters: DocFilters
): EnrichedDocument[] {
    const { activeTags, activeTypes } = filters;

    return documents.filter(doc => {
        if (activeTags.length > 0) {
            const hasTag = activeTags.some(t => doc.tags.includes(t));
            if (!hasTag) return false;
        }
        if (activeTypes.length > 0) {
            const hasType = activeTypes.includes(doc.type);
            if (!hasType) return false;
        }
        return true;
    });
}

function folderFromPath(sourcePath: string): string {
    const parts = sourcePath.split("/");
    if (parts.length <= 1) return "/";
    return parts.slice(0, -1).join("/");
}

export function groupDocuments(
    documents: EnrichedDocument[],
    groupBy: "folder" | "tag"
): Map<string, EnrichedDocument[]> {
    const groups = new Map<string, EnrichedDocument[]>();

    if (groupBy === "folder") {
        for (const doc of documents) {
            const folder = folderFromPath(doc.sourcePath);
            const existing = groups.get(folder) ?? [];
            existing.push(doc);
            groups.set(folder, existing);
        }
    } else {
        for (const doc of documents) {
            if (doc.tags.length === 0) {
                const existing = groups.get("Untagged") ?? [];
                existing.push(doc);
                groups.set("Untagged", existing);
            } else {
                for (const tag of doc.tags) {
                    const existing = groups.get(tag) ?? [];
                    existing.push(doc);
                    groups.set(tag, existing);
                }
            }
        }
    }

    return new Map([...groups.entries()].sort((a, b) => {
        if (a[0] === "Untagged") return 1;
        if (b[0] === "Untagged") return -1;
        return a[0].localeCompare(b[0]);
    }));
}

export function collectAllTags(documents: EnrichedDocument[]): string[] {
    const tags = new Set<string>();
    for (const doc of documents) {
        for (const tag of doc.tags) tags.add(tag);
    }
    return [...tags].sort();
}

export function collectAllTypes(documents: EnrichedDocument[]): string[] {
    const types = new Set<string>();
    for (const doc of documents) types.add(doc.type);
    return [...types].sort();
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `pnpm --filter web test -- --reporter verbose src/features/documents/filter-documents.test.ts`
Expected: All 8 tests pass.

- [ ] **Step 5: Commit**

```bash
git add apps/web/src/features/documents/filter-documents.ts apps/web/src/features/documents/filter-documents.test.ts
git commit -m "feat: pure filter and group functions for document browser"
```

---

### Task 3: Filter presets component

**Files:**
- Create: `apps/web/src/features/documents/document-filter-presets.tsx`

- [ ] **Step 1: Create the presets component**

Create `apps/web/src/features/documents/document-filter-presets.tsx`:

```typescript
import { Bookmark, Save, Trash2, X } from "lucide-react";
import { useState } from "react";

import {
    DropdownMenu,
    DropdownMenuContent,
    DropdownMenuItem,
    DropdownMenuLabel,
    DropdownMenuSeparator,
    DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { useLocalStorage } from "@/hooks/use-local-storage";

export interface DocPresetFilters {
    activeTags: string[];
    activeTypes: string[];
    groupBy: "folder" | "tag";
}

interface DocFilterPreset {
    name: string;
    filters: DocPresetFilters;
}

interface DocFilterPresetsProps {
    currentFilters: DocPresetFilters;
    onApplyPreset: (filters: DocPresetFilters) => void;
}

export function DocFilterPresets({ currentFilters, onApplyPreset }: DocFilterPresetsProps) {
    const [presets, setPresets] = useLocalStorage<DocFilterPreset[]>("fubbik-docs-filter-presets", []);
    const [isSaving, setIsSaving] = useState(false);
    const [presetName, setPresetName] = useState("");

    function handleSave() {
        const trimmed = presetName.trim();
        if (!trimmed) return;
        setPresets(prev => [...prev, { name: trimmed, filters: currentFilters }]);
        setPresetName("");
        setIsSaving(false);
    }

    function handleDelete(index: number, e: React.MouseEvent) {
        e.stopPropagation();
        e.preventDefault();
        setPresets(prev => prev.filter((_, i) => i !== index));
    }

    return (
        <div className="flex items-center gap-2 text-[11px]">
            <DropdownMenu>
                <DropdownMenuTrigger className="text-muted-foreground hover:text-foreground flex cursor-pointer items-center gap-1 transition-colors">
                    <Bookmark className="size-3" />
                    Presets
                </DropdownMenuTrigger>
                <DropdownMenuContent side="bottom" align="start" sideOffset={8}>
                    <DropdownMenuLabel>Saved Presets</DropdownMenuLabel>
                    <DropdownMenuSeparator />
                    {presets.length === 0 ? (
                        <div className="text-muted-foreground px-2 py-3 text-center text-xs">
                            No saved presets yet
                        </div>
                    ) : (
                        presets.map((preset, i) => (
                            <DropdownMenuItem
                                key={`${preset.name}-${i}`}
                                className="flex items-center justify-between gap-2"
                                onSelect={() => onApplyPreset(preset.filters)}
                            >
                                <span className="truncate">{preset.name}</span>
                                <button
                                    type="button"
                                    className="text-muted-foreground hover:text-destructive shrink-0 rounded p-0.5 transition-colors"
                                    onClick={(e) => handleDelete(i, e)}
                                    aria-label={`Delete preset ${preset.name}`}
                                >
                                    <Trash2 className="size-3" />
                                </button>
                            </DropdownMenuItem>
                        ))
                    )}
                </DropdownMenuContent>
            </DropdownMenu>

            {isSaving ? (
                <div className="flex items-center gap-1">
                    <input
                        type="text"
                        value={presetName}
                        onChange={e => setPresetName(e.target.value)}
                        onKeyDown={e => {
                            if (e.key === "Enter") handleSave();
                            if (e.key === "Escape") { setIsSaving(false); setPresetName(""); }
                        }}
                        placeholder="Name..."
                        className="bg-background border-input min-w-0 flex-1 rounded border px-1.5 py-0.5 text-[10px] outline-none focus:ring-1 focus:ring-ring"
                        autoFocus
                    />
                    <button type="button" onClick={handleSave} disabled={!presetName.trim()} className="text-muted-foreground hover:text-foreground disabled:opacity-40">
                        <Save className="size-3" />
                    </button>
                    <button type="button" onClick={() => { setIsSaving(false); setPresetName(""); }} className="text-muted-foreground hover:text-foreground">
                        <X className="size-3" />
                    </button>
                </div>
            ) : (
                <button
                    type="button"
                    onClick={() => setIsSaving(true)}
                    className="text-muted-foreground hover:text-foreground flex cursor-pointer items-center gap-1 transition-colors"
                >
                    <Save className="size-3" />
                    Save
                </button>
            )}
        </div>
    );
}
```

- [ ] **Step 2: Run type-check**

Run: `pnpm run check-types`
Expected: Pass.

- [ ] **Step 3: Commit**

```bash
git add apps/web/src/features/documents/document-filter-presets.tsx
git commit -m "feat: filter presets component for docs page"
```

---

### Task 4: Filter bar component

**Files:**
- Create: `apps/web/src/features/documents/document-filter-bar.tsx`

- [ ] **Step 1: Create the filter bar component**

Create `apps/web/src/features/documents/document-filter-bar.tsx`:

```typescript
import { ChevronDown, ChevronRight, FolderOpen, Tag, X } from "lucide-react";
import { useState } from "react";

import { DocFilterPresets, type DocPresetFilters } from "./document-filter-presets";

interface DocumentFilterBarProps {
    allTags: string[];
    allTypes: string[];
    activeTags: string[];
    activeTypes: string[];
    groupBy: "folder" | "tag";
    totalCount: number;
    filteredCount: number;
    onToggleTag: (tag: string) => void;
    onToggleType: (type: string) => void;
    onSetGroupBy: (groupBy: "folder" | "tag") => void;
    onClearAll: () => void;
    onApplyPreset: (filters: DocPresetFilters) => void;
}

export function DocumentFilterBar({
    allTags,
    allTypes,
    activeTags,
    activeTypes,
    groupBy,
    totalCount,
    filteredCount,
    onToggleTag,
    onToggleType,
    onSetGroupBy,
    onClearAll,
    onApplyPreset,
}: DocumentFilterBarProps) {
    const [expanded, setExpanded] = useState(false);
    const hasActiveFilters = activeTags.length > 0 || activeTypes.length > 0;

    return (
        <div className="border-b border-border/50 px-3 py-2">
            {/* Group-by toggle */}
            <div className="mb-2 flex items-center gap-1.5">
                <button
                    type="button"
                    onClick={() => onSetGroupBy("folder")}
                    className={`flex items-center gap-1 rounded px-2 py-0.5 text-[11px] font-medium transition-colors ${
                        groupBy === "folder"
                            ? "bg-muted text-foreground"
                            : "text-muted-foreground hover:text-foreground"
                    }`}
                >
                    <FolderOpen className="size-3" />
                    Folder
                </button>
                <button
                    type="button"
                    onClick={() => onSetGroupBy("tag")}
                    className={`flex items-center gap-1 rounded px-2 py-0.5 text-[11px] font-medium transition-colors ${
                        groupBy === "tag"
                            ? "bg-muted text-foreground"
                            : "text-muted-foreground hover:text-foreground"
                    }`}
                >
                    <Tag className="size-3" />
                    Tag
                </button>
                <button
                    type="button"
                    onClick={() => setExpanded(e => !e)}
                    className="ml-auto flex items-center gap-0.5 text-[11px] text-muted-foreground hover:text-foreground transition-colors"
                >
                    Filters
                    {hasActiveFilters && (
                        <span className="ml-0.5 rounded-full bg-primary/20 px-1 text-[9px] text-primary">
                            {activeTags.length + activeTypes.length}
                        </span>
                    )}
                    {expanded ? <ChevronDown className="size-3" /> : <ChevronRight className="size-3" />}
                </button>
            </div>

            {/* Expanded filter area */}
            {expanded && (
                <div className="space-y-2 border-t border-border/30 pt-2">
                    {/* Tags */}
                    {allTags.length > 0 && (
                        <div>
                            <div className="mb-1 text-[10px] font-semibold uppercase tracking-wider text-muted-foreground">Tags</div>
                            <div className="flex flex-wrap gap-1">
                                {allTags.map(tag => {
                                    const active = activeTags.includes(tag);
                                    return (
                                        <button
                                            key={tag}
                                            type="button"
                                            onClick={() => onToggleTag(tag)}
                                            className={`flex items-center gap-0.5 rounded px-1.5 py-0.5 text-[11px] transition-colors ${
                                                active
                                                    ? "bg-primary/20 text-primary border border-primary/40"
                                                    : "bg-muted/50 text-muted-foreground hover:text-foreground border border-transparent"
                                            }`}
                                        >
                                            {tag}
                                            {active && <X className="size-2.5" />}
                                        </button>
                                    );
                                })}
                            </div>
                        </div>
                    )}

                    {/* Types */}
                    {allTypes.length > 0 && (
                        <div>
                            <div className="mb-1 text-[10px] font-semibold uppercase tracking-wider text-muted-foreground">Type</div>
                            <div className="flex flex-wrap gap-1">
                                {allTypes.map(type => {
                                    const active = activeTypes.includes(type);
                                    return (
                                        <button
                                            key={type}
                                            type="button"
                                            onClick={() => onToggleType(type)}
                                            className={`flex items-center gap-0.5 rounded px-1.5 py-0.5 text-[11px] transition-colors ${
                                                active
                                                    ? "bg-primary/20 text-primary border border-primary/40"
                                                    : "bg-muted/50 text-muted-foreground hover:text-foreground border border-transparent"
                                            }`}
                                        >
                                            {type}
                                            {active && <X className="size-2.5" />}
                                        </button>
                                    );
                                })}
                            </div>
                        </div>
                    )}

                    {/* Presets + clear */}
                    <div className="flex items-center justify-between pt-1">
                        <DocFilterPresets
                            currentFilters={{ activeTags, activeTypes, groupBy }}
                            onApplyPreset={onApplyPreset}
                        />
                        {hasActiveFilters && (
                            <button
                                type="button"
                                onClick={onClearAll}
                                className="text-[11px] text-muted-foreground hover:text-foreground transition-colors"
                            >
                                Clear all
                            </button>
                        )}
                    </div>

                    {/* Result count */}
                    {hasActiveFilters && (
                        <div className="text-[11px] text-muted-foreground">
                            Showing {filteredCount} of {totalCount} documents
                        </div>
                    )}
                </div>
            )}
        </div>
    );
}
```

- [ ] **Step 2: Run type-check**

Run: `pnpm run check-types`
Expected: Pass.

- [ ] **Step 3: Commit**

```bash
git add apps/web/src/features/documents/document-filter-bar.tsx
git commit -m "feat: filter bar component for docs page"
```

---

### Task 5: Integrate filtering and grouping into document browser

**Files:**
- Modify: `apps/web/src/features/documents/document-browser.tsx`
- Modify: `apps/web/src/routes/docs.tsx`

This is the largest task — wiring the filter bar, filter logic, tag grouping, and URL state into the existing 1112-line document browser. The changes are additive.

- [ ] **Step 1: Add URL search params for filters**

In `apps/web/src/routes/docs.tsx`, extend the `validateSearch` to include filter params:

```typescript
    validateSearch: (search: Record<string, unknown>): {
        tab?: string;
        id?: string;
        section?: string;
        groupBy?: string;
        tags?: string;
        types?: string;
    } => ({
        tab: (search.tab as string) ?? undefined,
        id: (search.id as string) ?? undefined,
        section: (search.section as string) ?? undefined,
        groupBy: (search.groupBy as string) ?? undefined,
        tags: (search.tags as string) ?? undefined,
        types: (search.types as string) ?? undefined,
    }),
```

Pass the new params to `DocumentBrowser`:

```typescript
{tab === "docs" && (
    <DocumentBrowser
        initialDocId={search.id}
        initialSection={search.section}
        initialGroupBy={search.groupBy as "folder" | "tag" | undefined}
        initialTags={search.tags?.split(",").filter(Boolean)}
        initialTypes={search.types?.split(",").filter(Boolean)}
    />
)}
```

- [ ] **Step 2: Update `DocumentBrowserProps` and add filter state**

In `apps/web/src/features/documents/document-browser.tsx`, update the props interface:

```typescript
interface DocumentBrowserProps {
    initialDocId?: string;
    initialSection?: string;
    initialGroupBy?: "folder" | "tag";
    initialTags?: string[];
    initialTypes?: string[];
}
```

Update the function signature:

```typescript
export function DocumentBrowser({ initialDocId, initialSection, initialGroupBy, initialTags, initialTypes }: DocumentBrowserProps) {
```

Add imports at the top of the file:

```typescript
import { DocumentFilterBar } from "./document-filter-bar";
import { filterDocuments, groupDocuments, collectAllTags, collectAllTypes, type EnrichedDocument } from "./filter-documents";
import type { DocPresetFilters } from "./document-filter-presets";
```

Add the `Tag` icon to the lucide imports:

```typescript
import { Check, ChevronDown, ChevronLeft, ChevronRight, Eye, FileText, FolderOpen, Link2, Menu, Pencil, Plus, Printer, Search, Tag, X } from "lucide-react";
```

Add filter state after the existing state declarations (around line 308):

```typescript
    const [activeTags, setActiveTags] = useState<string[]>(initialTags ?? []);
    const [activeTypes, setActiveTypes] = useState<string[]>(initialTypes ?? []);
    const [groupBy, setGroupBy] = useState<"folder" | "tag">(initialGroupBy ?? "folder");
```

- [ ] **Step 3: Update `DocumentListItem` type to include tags and type**

Update the existing `DocumentListItem` interface at the top of the file:

```typescript
interface DocumentListItem {
    id: string;
    title: string;
    sourcePath: string;
    description: string | null;
    chunkCount: number;
    updatedAt: Date;
    lastChunkUpdatedAt: Date | null;
    oldestChunkUpdatedAt: Date | null;
    tags: string[];
    type: string;
}
```

- [ ] **Step 4: Add filter/group memos and URL sync**

After the existing `const documents = listQuery.data ?? [];` line, add the filter/group computations:

```typescript
    const allTags = useMemo(() => collectAllTags(documents as EnrichedDocument[]), [documents]);
    const allTypes = useMemo(() => collectAllTypes(documents as EnrichedDocument[]), [documents]);

    const filteredDocuments = useMemo(
        () => filterDocuments(documents as EnrichedDocument[], { activeTags, activeTypes }),
        [documents, activeTags, activeTypes]
    );

    const groupedDocuments = useMemo(
        () => groupDocuments(filteredDocuments, groupBy),
        [filteredDocuments, groupBy]
    );
```

Add URL sync — after the filter state declarations, add a `useEffect` that syncs filter state to URL:

```typescript
    useEffect(() => {
        navigate({
            to: "/docs",
            search: (prev: Record<string, unknown>) => ({
                ...prev,
                groupBy: groupBy !== "folder" ? groupBy : undefined,
                tags: activeTags.length > 0 ? activeTags.join(",") : undefined,
                types: activeTypes.length > 0 ? activeTypes.join(",") : undefined,
            }),
            replace: true,
        });
    }, [activeTags, activeTypes, groupBy]);
```

Add filter action handlers:

```typescript
    const toggleTag = (tag: string) => {
        setActiveTags(prev => prev.includes(tag) ? prev.filter(t => t !== tag) : [...prev, tag]);
    };
    const toggleType = (type: string) => {
        setActiveTypes(prev => prev.includes(type) ? prev.filter(t => t !== type) : [...prev, type]);
    };
    const clearFilters = () => {
        setActiveTags([]);
        setActiveTypes([]);
    };
    const applyPreset = (preset: DocPresetFilters) => {
        setActiveTags(preset.activeTags);
        setActiveTypes(preset.activeTypes);
        setGroupBy(preset.groupBy);
    };
```

- [ ] **Step 5: Update `sidebarFiltered` and `folderTree` to use filtered documents**

Replace the existing `sidebarFiltered` and `folderTree` memos:

```typescript
    const sidebarFiltered = useMemo(() => {
        let docs = filteredDocuments as DocumentListItem[];
        if (searchQuery && !isSearching) {
            const q = searchQuery.toLowerCase();
            docs = docs.filter(
                d => d.title.toLowerCase().includes(q) || d.sourcePath.toLowerCase().includes(q)
            );
        }
        return docs;
    }, [filteredDocuments, searchQuery, isSearching]);

    const folderTree = useMemo(() => buildFolderTree(sidebarFiltered), [sidebarFiltered]);
```

- [ ] **Step 6: Render the filter bar in the sidebar**

Find the sidebar rendering section in the JSX. The sidebar currently has the search input and the folder tree. Add the `DocumentFilterBar` between the search input and the folder tree/tag groups.

Look for the sidebar `<div>` that contains the search input (`data-docs-search`). After the search input's container `<div>`, add:

```tsx
<DocumentFilterBar
    allTags={allTags}
    allTypes={allTypes}
    activeTags={activeTags}
    activeTypes={activeTypes}
    groupBy={groupBy}
    totalCount={documents.length}
    filteredCount={filteredDocuments.length}
    onToggleTag={toggleTag}
    onToggleType={toggleType}
    onSetGroupBy={setGroupBy}
    onClearAll={clearFilters}
    onApplyPreset={applyPreset}
/>
```

- [ ] **Step 7: Add tag group sidebar rendering**

Below the `FolderTreeNode` rendering in the sidebar, add a conditional for tag grouping. Find where the folder tree nodes are rendered (the `{folderTree.children.map(child => (...FolderTreeNode...))}` section) and wrap it:

```tsx
{groupBy === "folder" ? (
    <>
        {folderTree.children.map(child => (
            <FolderTreeNode
                key={child.fullPath}
                node={child}
                depth={1}
                selectedId={selectedId}
                onSelect={setSelectedId}
                defaultOpen={true}
            />
        ))}
        {folderTree.docs.map(doc => (
            /* ... existing root-level doc buttons ... */
        ))}
    </>
) : (
    <div className="px-1 py-1">
        {[...groupedDocuments.entries()].map(([groupName, groupDocs]) => (
            <TagGroupNode
                key={groupName}
                name={groupName}
                docs={groupDocs as DocumentListItem[]}
                selectedId={selectedId}
                onSelect={setSelectedId}
            />
        ))}
    </div>
)}
```

Add the `TagGroupNode` component before the `DocumentBrowser` function:

```typescript
function TagGroupNode({
    name,
    docs,
    selectedId,
    onSelect,
}: {
    name: string;
    docs: DocumentListItem[];
    selectedId: string | null;
    onSelect: (id: string) => void;
}) {
    const [open, setOpen] = useState(true);

    return (
        <div>
            <button
                type="button"
                onClick={() => setOpen(o => !o)}
                className="text-muted-foreground hover:text-foreground mb-0.5 flex w-full items-center gap-1 px-2 py-1 text-xs font-medium transition-colors"
            >
                {open ? <ChevronDown className="size-3" /> : <ChevronRight className="size-3" />}
                <Tag className="size-3.5" />
                <span className="truncate">{name}</span>
                <Badge variant="secondary" size="sm" className="ml-auto shrink-0 font-mono text-[9px]">
                    {docs.length}
                </Badge>
            </button>
            {open && (
                <div>
                    {docs.map(doc => (
                        <button
                            key={doc.id}
                            onClick={() => onSelect(doc.id)}
                            className={`flex w-full items-center gap-2 rounded-md py-1.5 text-left text-sm transition-colors ${
                                selectedId === doc.id
                                    ? "bg-muted text-foreground font-medium"
                                    : "text-muted-foreground hover:text-foreground hover:bg-muted/50"
                            }`}
                            style={{ paddingLeft: 32 }}
                        >
                            <FileText className="size-3.5 shrink-0" />
                            <span className="min-w-0 flex-1 truncate">{doc.title || doc.sourcePath.split("/").pop()}</span>
                            <Badge variant="secondary" size="sm" className="shrink-0 font-mono text-[9px] mr-2">
                                {doc.chunkCount}
                            </Badge>
                        </button>
                    ))}
                </div>
            )}
        </div>
    );
}
```

- [ ] **Step 8: Update the "All Documents" index view for tag grouping**

Find the `IndexTree` usage in the main content area (the view when no document is selected). Wrap it with a conditional:

```tsx
{!selectedId && !isSearching && (
    groupBy === "folder" ? (
        <IndexTree node={folderTree} depth={0} onSelect={setSelectedId} />
    ) : (
        <div className="space-y-4">
            {[...groupedDocuments.entries()].map(([groupName, groupDocs]) => (
                <div key={groupName}>
                    <h3 className="text-sm font-semibold text-muted-foreground mb-2 flex items-center gap-1.5">
                        <Tag className="size-3.5" />
                        {groupName}
                    </h3>
                    <div className="space-y-1 pl-5">
                        {(groupDocs as DocumentListItem[]).map(doc => {
                            const staleness = getStaleness(doc);
                            return (
                                <button
                                    key={doc.id}
                                    onClick={() => setSelectedId(doc.id)}
                                    className="text-foreground hover:text-foreground/80 flex items-center gap-2 text-sm w-full text-left"
                                >
                                    <FileText className="size-3.5 text-muted-foreground shrink-0" />
                                    <span>{doc.title}</span>
                                    {doc.description && (
                                        <span className="text-muted-foreground text-xs truncate">— {doc.description}</span>
                                    )}
                                    <span className={`text-xs ml-auto shrink-0 ${staleness.color}`} title={staleness.tooltip}>
                                        {staleness.label}
                                    </span>
                                </button>
                            );
                        })}
                    </div>
                </div>
            ))}
        </div>
    )
)}
```

- [ ] **Step 9: Run type-check**

Run: `pnpm run check-types`
Expected: All packages pass.

- [ ] **Step 10: Run full test suite**

Run: `pnpm test`
Expected: All tests pass.

- [ ] **Step 11: Commit**

```bash
git add apps/web/src/features/documents/document-browser.tsx apps/web/src/routes/docs.tsx
git commit -m "feat: integrate filter bar, tag grouping, and URL state into document browser"
```

---

### Task 6: Final verification

**Files:** None (verification only)

- [ ] **Step 1: Run full CI pipeline**

Run: `pnpm ci`
Expected: type-check, lint, test, build, format check, sherif all pass.

- [ ] **Step 2: Manual browser verification**

Run: `pnpm dev`

Verify:
1. Navigate to `/docs` — filter bar appears above folder tree in sidebar
2. Click "Filters ▸" — expands to show tag and type pills
3. Click a tag pill — sidebar narrows to matching documents, result count updates
4. Click a type pill — further narrows (AND between tag + type)
5. Click "Tag" group-by — sidebar switches from folder tree to tag groups
6. Main content "All Documents" view also switches to tag grouping
7. Click "Clear all" — removes all filters
8. Save a preset — name it, verify it appears in presets dropdown
9. Apply a preset — filters + groupBy restored
10. Delete a preset — removed from dropdown
11. URL params update as filters change — copy URL, paste in new tab, same view restored
12. Search still works — type in search box, results appear, navigate to result

- [ ] **Step 3: Commit any final fixes**

If browser testing reveals issues, fix and commit individually.
