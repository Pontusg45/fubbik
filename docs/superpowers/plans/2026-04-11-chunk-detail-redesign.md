# Chunk Detail Redesign Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Rewrite `/chunks/$chunkId` into a reading-first three-pane layout (sibling navigator, content, metadata/ToC) with a "More context" drawer for all supporting data.

**Architecture:** Extract current inline sub-components into dedicated files under `apps/web/src/features/chunks/detail/`. Reuse all existing feature components (ChunkLinkRenderer, ChunkToc, AiSection, DependencyTree, etc.) unchanged. The route component becomes a thin composition. Use `Sheet` component for the drawer.

**Tech Stack:** React, TanStack Router, shadcn-ui (base-ui `render` prop pattern), Tailwind CSS, Sheet component, existing hooks

---

## File Structure

| File | Action | Responsibility |
|------|--------|---------------|
| `apps/web/src/features/chunks/chunk-comments.tsx` | Create | Extracted from inline definition in current chunks.$chunkId.tsx |
| `apps/web/src/features/chunks/detail/chunk-detail-top-bar.tsx` | Create | Top bar with Back / Favorite / Export / Edit / ⋯ menu |
| `apps/web/src/features/chunks/detail/chunk-sibling-navigator.tsx` | Create | Left pane with sibling chunks + prev/next |
| `apps/web/src/features/chunks/detail/chunk-metadata-panel.tsx` | Create | Right pane with ToC + compact details |
| `apps/web/src/features/chunks/detail/chunk-detail-content.tsx` | Create | Center content (meta row, title, summary, markdown, decision callout) |
| `apps/web/src/features/chunks/detail/more-context-drawer.tsx` | Create | Sheet-based drawer with 4 tabs |
| `apps/web/src/features/chunks/detail/more-context-links-tab.tsx` | Create | Links tab (outgoing, incoming, deps, suggested, related) |
| `apps/web/src/features/chunks/detail/more-context-context-tab.tsx` | Create | Context tab (applies-to, file-refs, AI, decision edit) |
| `apps/web/src/routes/chunks.$chunkId.tsx` | Rewrite | Thin composition of the above |

---

### Task 1: Extract ChunkComments to its own file

**Files:**
- Create: `apps/web/src/features/chunks/chunk-comments.tsx`

**Context:** The current `chunks.$chunkId.tsx` file defines `ChunkComments` as an inline function at the bottom of the file (around line 657-818). Extract it unchanged into its own file so we can import it from the new drawer tab components.

- [ ] **Step 1: Find the ChunkComments definition**

Run: `grep -n "function ChunkComments" /Users/pontus/projects/fubbik/apps/web/src/routes/chunks.\$chunkId.tsx`

Note the line number where the function starts.

- [ ] **Step 2: Read the ChunkComments function and its imports**

Read the current chunks.$chunkId.tsx from that line to the end of the function. Note all external identifiers it references: API calls, hooks, types, icons, UI components.

- [ ] **Step 3: Create the new file**

Create `apps/web/src/features/chunks/chunk-comments.tsx` and paste the `ChunkComments` function verbatim. Add all imports the function needs at the top:

```typescript
// Example imports — verify against the original function body
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { Loader2, MessageSquare, Pencil, Trash2, User } from "lucide-react";
import { useState } from "react";
import { toast } from "sonner";

import { Button } from "@/components/ui/button";
import { api } from "@/utils/api";
```

Export the component:
```typescript
export function ChunkComments({ chunkId }: { chunkId: string }) {
    // ... existing body ...
}
```

Keep the function signature and body identical to the original. Do not modify the logic.

- [ ] **Step 4: Remove the inline definition from chunks.$chunkId.tsx**

Delete the `ChunkComments` function from `chunks.$chunkId.tsx`. Add an import at the top:
```typescript
import { ChunkComments } from "@/features/chunks/chunk-comments";
```

- [ ] **Step 5: Verify type check passes**

Run: `pnpm --filter web run check-types 2>&1 | grep -E "chunk-comments|chunks.\\\$chunkId"`

Expected: No errors in the new file or the route file.

- [ ] **Step 6: Commit**

```bash
git add apps/web/src/features/chunks/chunk-comments.tsx apps/web/src/routes/chunks.\$chunkId.tsx
git commit -m "refactor(chunks): extract ChunkComments to its own file"
```

---

### Task 2: Create the Top Bar component

**Files:**
- Create: `apps/web/src/features/chunks/detail/chunk-detail-top-bar.tsx`

**Context:** The new top bar replaces the 10-button horizontal row with a clean Back button on the left and 4 action buttons on the right (Favorite, Export, Edit, ⋯ menu).

- [ ] **Step 1: Create the component file**

```typescript
// apps/web/src/features/chunks/detail/chunk-detail-top-bar.tsx
import { Link, useNavigate } from "@tanstack/react-router";
import {
    Archive,
    ArrowLeft,
    Check,
    Copy,
    Download,
    Edit,
    Eye,
    Flag,
    Focus,
    MoreHorizontal,
    Network,
    Scissors,
    Sparkles,
    Star,
    Trash2,
    Type,
} from "lucide-react";
import { useState } from "react";

import { ConfirmDialog } from "@/components/confirm-dialog";
import { Button } from "@/components/ui/button";
import {
    DropdownMenu,
    DropdownMenuContent,
    DropdownMenuItem,
    DropdownMenuSeparator,
    DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { useFavorites } from "@/features/chunks/use-favorites";
import { useFocusMode } from "@/hooks/use-focus-mode";

export interface ChunkDetailTopBarProps {
    chunkId: string;
    title: string;
    content: string;
    type: string;
    isEntryPoint?: boolean;
    isAi?: boolean;
    reviewStatus?: string;
    onArchive: () => void;
    onDelete: () => void;
    onSplit: () => void;
    onToggleEntryPoint: () => void;
    onReview?: (status: "reviewed" | "approved") => void;
    archivePending?: boolean;
    deletePending?: boolean;
}

function buildMarkdown(title: string, type: string, content: string): string {
    return `# ${title}\n\n**Type:** ${type}\n\n${content}`;
}

function downloadMarkdown(title: string, type: string, content: string) {
    const md = buildMarkdown(title, type, content);
    const blob = new Blob([md], { type: "text/markdown" });
    const url = URL.createObjectURL(blob);
    const a = document.createElement("a");
    a.href = url;
    a.download = `${title.replace(/[^a-z0-9]/gi, "-").toLowerCase()}.md`;
    a.click();
    URL.revokeObjectURL(url);
}

export function ChunkDetailTopBar({
    chunkId,
    title,
    content,
    type,
    isEntryPoint,
    isAi,
    reviewStatus,
    onArchive,
    onDelete,
    onSplit,
    onToggleEntryPoint,
    onReview,
    archivePending,
    deletePending,
}: ChunkDetailTopBarProps) {
    const navigate = useNavigate();
    const { toggleFavorite, isFavorite } = useFavorites();
    const { enabled: focusMode, toggle: toggleFocus } = useFocusMode();
    const [showDeleteDialog, setShowDeleteDialog] = useState(false);
    const [exportCopied, setExportCopied] = useState(false);

    const favorited = isFavorite(chunkId);

    function handleCopy() {
        void navigator.clipboard.writeText(buildMarkdown(title, type, content));
        setExportCopied(true);
        setTimeout(() => setExportCopied(false), 2000);
    }

    return (
        <div className="mb-6 flex items-center justify-between print:hidden" data-focus-hide="true">
            <Button variant="ghost" size="sm" render={<Link to="/dashboard" />}>
                <ArrowLeft className="size-4" />
                Back
            </Button>

            <div className="flex items-center gap-1">
                <Button
                    variant="ghost"
                    size="sm"
                    onClick={() => toggleFavorite(chunkId)}
                    className="gap-1.5"
                    title={favorited ? "Remove from favorites" : "Add to favorites"}
                >
                    <Star className={`size-3.5 ${favorited ? "fill-yellow-500 text-yellow-500" : ""}`} />
                </Button>

                <DropdownMenu>
                    <DropdownMenuTrigger
                        render={
                            <Button variant="ghost" size="sm" className="gap-1.5">
                                <Download className="size-3.5" />
                                Export
                            </Button>
                        }
                    />
                    <DropdownMenuContent align="end">
                        <DropdownMenuItem onClick={handleCopy}>
                            {exportCopied ? <Check className="size-3.5" /> : <Copy className="size-3.5" />}
                            {exportCopied ? "Copied" : "Copy as markdown"}
                        </DropdownMenuItem>
                        <DropdownMenuItem onClick={() => downloadMarkdown(title, type, content)}>
                            <Download className="size-3.5" />
                            Download .md
                        </DropdownMenuItem>
                    </DropdownMenuContent>
                </DropdownMenu>

                <Button
                    variant="outline"
                    size="sm"
                    render={<Link to="/chunks/$chunkId/edit" params={{ chunkId }} />}
                    className="gap-1.5"
                >
                    <Edit className="size-3.5" />
                    Edit
                </Button>

                <DropdownMenu>
                    <DropdownMenuTrigger
                        render={
                            <Button variant="ghost" size="sm" aria-label="More actions">
                                <MoreHorizontal className="size-4" />
                            </Button>
                        }
                    />
                    <DropdownMenuContent align="end" className="w-56">
                        <DropdownMenuItem onClick={toggleFocus}>
                            <Focus className="size-3.5" />
                            {focusMode ? "Exit focus mode" : "Focus mode"}
                            <span className="ml-auto text-[10px] text-muted-foreground font-mono">f</span>
                        </DropdownMenuItem>
                        <DropdownMenuItem
                            onClick={() => void navigate({ to: "/graph", search: { pathFrom: chunkId } as any })}
                        >
                            <Network className="size-3.5" />
                            Find path in graph
                        </DropdownMenuItem>
                        <DropdownMenuItem
                            onClick={() =>
                                void navigate({
                                    to: "/search",
                                    search: { q: `similar-to:"${title}"` } as any,
                                })
                            }
                        >
                            <Sparkles className="size-3.5" />
                            Show similar chunks
                        </DropdownMenuItem>

                        <DropdownMenuSeparator />

                        <DropdownMenuItem onClick={onSplit}>
                            <Scissors className="size-3.5" />
                            Split chunk
                        </DropdownMenuItem>
                        <DropdownMenuItem onClick={onToggleEntryPoint}>
                            <Flag className={`size-3.5 ${isEntryPoint ? "fill-emerald-500 text-emerald-500" : ""}`} />
                            {isEntryPoint ? "Unmark entry point" : "Mark as entry point"}
                        </DropdownMenuItem>

                        {isAi && reviewStatus !== "approved" && onReview && (
                            <>
                                <DropdownMenuSeparator />
                                {reviewStatus === "draft" && (
                                    <DropdownMenuItem onClick={() => onReview("reviewed")}>
                                        <Eye className="size-3.5" />
                                        Mark as reviewed
                                    </DropdownMenuItem>
                                )}
                                <DropdownMenuItem onClick={() => onReview("approved")}>
                                    <Check className="size-3.5" />
                                    Approve
                                </DropdownMenuItem>
                            </>
                        )}

                        <DropdownMenuSeparator />

                        <DropdownMenuItem onClick={onArchive} disabled={archivePending}>
                            <Archive className="size-3.5" />
                            {archivePending ? "Archiving…" : "Archive"}
                        </DropdownMenuItem>
                        <DropdownMenuItem
                            onClick={() => setShowDeleteDialog(true)}
                            disabled={deletePending}
                            className="text-destructive focus:text-destructive"
                        >
                            <Trash2 className="size-3.5" />
                            Delete
                        </DropdownMenuItem>
                    </DropdownMenuContent>
                </DropdownMenu>
            </div>

            <ConfirmDialog
                open={showDeleteDialog}
                onOpenChange={setShowDeleteDialog}
                title="Delete chunk"
                description="Permanently delete this chunk? This cannot be undone."
                confirmLabel="Delete"
                confirmVariant="destructive"
                onConfirm={() => {
                    setShowDeleteDialog(false);
                    onDelete();
                }}
                loading={deletePending}
            />
        </div>
    );
}
```

- [ ] **Step 2: Verify type check**

Run: `pnpm --filter web run check-types 2>&1 | grep chunk-detail-top-bar`

Expected: No errors. If `DropdownMenuTrigger` doesn't accept `render` prop, check the actual API of `@/components/ui/dropdown-menu` and adapt.

- [ ] **Step 3: Commit**

```bash
git add apps/web/src/features/chunks/detail/chunk-detail-top-bar.tsx
git commit -m "feat(chunks): add ChunkDetailTopBar component"
```

---

### Task 3: Create the Sibling Navigator

**Files:**
- Create: `apps/web/src/features/chunks/detail/chunk-sibling-navigator.tsx`

**Context:** Slim left pane with a sliding window of 10 sibling chunks within the same codebase, plus prev/next buttons.

- [ ] **Step 1: Create the component**

```typescript
// apps/web/src/features/chunks/detail/chunk-sibling-navigator.tsx
import { useQuery } from "@tanstack/react-query";
import { Link, useNavigate } from "@tanstack/react-router";
import { ChevronLeft, ChevronRight } from "lucide-react";
import { useEffect, useMemo } from "react";

import { Button } from "@/components/ui/button";
import { api } from "@/utils/api";

export interface ChunkSiblingNavigatorProps {
    currentChunkId: string;
    codebaseId?: string;
    codebaseName?: string;
}

interface SiblingChunk {
    id: string;
    title: string;
    type: string;
}

const WINDOW_SIZE = 10;
const BEFORE = 4;

export function ChunkSiblingNavigator({
    currentChunkId,
    codebaseId,
    codebaseName,
}: ChunkSiblingNavigatorProps) {
    const navigate = useNavigate();

    const { data } = useQuery({
        queryKey: ["chunk-siblings", codebaseId],
        queryFn: async () => {
            const { data, error } = await api.api.chunks.get({
                query: {
                    ...(codebaseId ? { codebaseId } : {}),
                    sort: "updated",
                    limit: "50",
                } as any,
            });
            if (error) return { chunks: [] as SiblingChunk[], total: 0 };
            return data as unknown as { chunks: SiblingChunk[]; total: number };
        },
        enabled: !!codebaseId,
    });

    const allSiblings: SiblingChunk[] = (data?.chunks ?? []).filter(
        (c: SiblingChunk) => c.id !== currentChunkId,
    );

    // Compute sliding window
    const { windowed, currentIndex, total } = useMemo(() => {
        const total = allSiblings.length;
        if (total === 0) return { windowed: [], currentIndex: -1, total: 0 };

        // Find where "current" would fit in the sorted list (we don't include current chunk itself,
        // so just pick a window of WINDOW_SIZE from the top)
        const windowed = allSiblings.slice(0, WINDOW_SIZE);
        return { windowed, currentIndex: -1, total };
    }, [allSiblings]);

    const prev = windowed[0];
    const next = windowed[1];

    // Keyboard: h / l for prev/next sibling
    useEffect(() => {
        function handleKey(e: KeyboardEvent) {
            const tag = document.activeElement?.tagName;
            if (tag === "INPUT" || tag === "TEXTAREA" || tag === "SELECT") return;
            if (e.metaKey || e.ctrlKey || e.altKey) return;
            if (e.key === "h" && prev) {
                e.preventDefault();
                void navigate({ to: "/chunks/$chunkId", params: { chunkId: prev.id } });
            } else if (e.key === "l" && next) {
                e.preventDefault();
                void navigate({ to: "/chunks/$chunkId", params: { chunkId: next.id } });
            }
        }
        window.addEventListener("keydown", handleKey);
        return () => window.removeEventListener("keydown", handleKey);
    }, [prev, next, navigate]);

    if (!codebaseId || total === 0) {
        return null;
    }

    return (
        <aside
            className="hidden xl:block w-[180px] shrink-0 print:hidden"
            data-focus-hide="true"
        >
            <div className="sticky top-8">
                <div className="text-[9px] font-semibold uppercase tracking-wider text-muted-foreground mb-2">
                    In this codebase
                </div>
                {codebaseName && (
                    <Link
                        to="/codebases/$codebaseId"
                        params={{ codebaseId }}
                        className="text-xs font-medium hover:text-primary transition-colors mb-3 block truncate"
                    >
                        {codebaseName}
                    </Link>
                )}
                <nav className="flex flex-col gap-0.5">
                    {windowed.map(sibling => (
                        <Link
                            key={sibling.id}
                            to="/chunks/$chunkId"
                            params={{ chunkId: sibling.id }}
                            className="rounded px-2 py-1.5 text-[11px] text-muted-foreground hover:bg-muted hover:text-foreground transition-colors truncate"
                            title={sibling.title}
                        >
                            {sibling.title}
                        </Link>
                    ))}
                </nav>
                {total > WINDOW_SIZE && (
                    <Link
                        to="/chunks"
                        search={{ codebaseId } as any}
                        className="mt-2 block text-[10px] text-muted-foreground hover:text-foreground transition-colors px-2"
                    >
                        See all ({total}) →
                    </Link>
                )}
                <div className="mt-3 flex gap-1 border-t pt-3">
                    <Button
                        variant="ghost"
                        size="sm"
                        disabled={!prev}
                        onClick={() => prev && void navigate({ to: "/chunks/$chunkId", params: { chunkId: prev.id } })}
                        className="flex-1 gap-1 text-[10px]"
                        title="Previous (h)"
                    >
                        <ChevronLeft className="size-3" />
                        Prev
                    </Button>
                    <Button
                        variant="ghost"
                        size="sm"
                        disabled={!next}
                        onClick={() => next && void navigate({ to: "/chunks/$chunkId", params: { chunkId: next.id } })}
                        className="flex-1 gap-1 text-[10px]"
                        title="Next (l)"
                    >
                        Next
                        <ChevronRight className="size-3" />
                    </Button>
                </div>
            </div>
        </aside>
    );
}
```

Note: The sliding window logic here is simplified because the current chunk is filtered out; in practice we just show the top N by update time. The prev/next buttons navigate to the first two items in the window.

- [ ] **Step 2: Verify type check**

Run: `pnpm --filter web run check-types 2>&1 | grep sibling-navigator`

Expected: No errors.

- [ ] **Step 3: Commit**

```bash
git add apps/web/src/features/chunks/detail/chunk-sibling-navigator.tsx
git commit -m "feat(chunks): add ChunkSiblingNavigator for detail page left pane"
```

---

### Task 4: Create the Metadata Panel (Right Pane)

**Files:**
- Create: `apps/web/src/features/chunks/detail/chunk-metadata-panel.tsx`

**Context:** Right pane with the existing `ChunkToc` (from content) and compact details (health, tags, type, created, updated, size, connections count, codebase, origin, review).

- [ ] **Step 1: Create the component**

```typescript
// apps/web/src/features/chunks/detail/chunk-metadata-panel.tsx
import { Link } from "@tanstack/react-router";

import { Badge } from "@/components/ui/badge";
import { ChunkHealthBadge } from "@/features/chunks/chunk-health-badge";
import { ChunkToc } from "@/features/chunks/chunk-toc";
import { getChunkSize } from "@/features/chunks/chunk-size";
import { InlineTagEditor } from "@/features/chunks/inline-tag-editor";

export interface ChunkMetadataPanelProps {
    content: string;
    tags: Array<{ id: string; name: string }>;
    onTagsUpdate: (tags: string[]) => void;
    tagsLoading?: boolean;
    type: string;
    createdAt: string | Date;
    updatedAt: string | Date;
    connectionCount: number;
    codebases?: Array<{ id: string; name: string }>;
    origin?: string;
    reviewStatus?: string;
    healthScore?: {
        total: number;
        breakdown: { freshness: number; completeness: number; richness: number; connectivity: number };
        issues: string[];
    };
    onShowConnections: () => void;
}

export function ChunkMetadataPanel({
    content,
    tags,
    onTagsUpdate,
    tagsLoading,
    type,
    createdAt,
    updatedAt,
    connectionCount,
    codebases,
    origin,
    reviewStatus,
    healthScore,
    onShowConnections,
}: ChunkMetadataPanelProps) {
    const size = getChunkSize(content);
    const created = new Date(createdAt);
    const updated = new Date(updatedAt);
    const primaryCodebase = codebases && codebases.length > 0 ? codebases[0] : undefined;

    return (
        <aside
            className="hidden lg:block w-[220px] shrink-0 print:hidden"
            data-focus-hide="true"
        >
            <div className="sticky top-8 space-y-6">
                <ChunkToc content={content} />

                <div className="border-t pt-4">
                    <div className="text-[9px] font-semibold uppercase tracking-wider text-muted-foreground mb-3">
                        Details
                    </div>

                    {healthScore && (
                        <div className="mb-3 flex items-center justify-between">
                            <span className="text-[11px] text-muted-foreground">Health</span>
                            <ChunkHealthBadge healthScore={healthScore} />
                        </div>
                    )}

                    <div className="mb-4">
                        <div className="text-[11px] text-muted-foreground mb-1.5">Tags</div>
                        <InlineTagEditor
                            tags={tags}
                            onUpdate={onTagsUpdate}
                            loading={tagsLoading}
                        />
                    </div>

                    <dl className="space-y-1.5 text-[11px]">
                        <MetaRow label="Type">
                            <Link to="/chunks" search={{ type } as any} className="hover:text-foreground transition-colors">
                                {type}
                            </Link>
                        </MetaRow>
                        <MetaRow label="Created">
                            <span title={created.toLocaleString()}>{created.toLocaleDateString()}</span>
                        </MetaRow>
                        <MetaRow label="Updated">
                            <span title={updated.toLocaleString()}>{updated.toLocaleDateString()}</span>
                        </MetaRow>
                        <MetaRow label="Size">
                            <span style={{ color: size.color }}>{size.lines} ln</span>
                        </MetaRow>
                        <MetaRow label="Connections">
                            <button
                                type="button"
                                onClick={onShowConnections}
                                className="hover:text-foreground transition-colors underline-offset-2 hover:underline"
                            >
                                {connectionCount}
                            </button>
                        </MetaRow>
                        {primaryCodebase && (
                            <MetaRow label="Codebase">
                                <Link
                                    to="/codebases/$codebaseId"
                                    params={{ codebaseId: primaryCodebase.id }}
                                    className="hover:text-foreground transition-colors truncate"
                                >
                                    {primaryCodebase.name}
                                </Link>
                            </MetaRow>
                        )}
                        {origin && (
                            <MetaRow label="Origin">
                                <Badge variant="secondary" size="sm" className="font-mono text-[9px]">
                                    {origin}
                                </Badge>
                            </MetaRow>
                        )}
                        {reviewStatus && (
                            <MetaRow label="Review">
                                <Badge variant="secondary" size="sm" className="font-mono text-[9px]">
                                    {reviewStatus}
                                </Badge>
                            </MetaRow>
                        )}
                    </dl>
                </div>
            </div>
        </aside>
    );
}

function MetaRow({ label, children }: { label: string; children: React.ReactNode }) {
    return (
        <div className="flex items-center justify-between gap-2">
            <dt className="text-muted-foreground shrink-0">{label}</dt>
            <dd className="text-right min-w-0 truncate">{children}</dd>
        </div>
    );
}
```

- [ ] **Step 2: Verify type check**

Run: `pnpm --filter web run check-types 2>&1 | grep metadata-panel`

Expected: No errors. If `ChunkHealthBadge`, `InlineTagEditor`, or `getChunkSize` have different signatures than assumed, adjust the calls.

- [ ] **Step 3: Commit**

```bash
git add apps/web/src/features/chunks/detail/chunk-metadata-panel.tsx
git commit -m "feat(chunks): add ChunkMetadataPanel for detail page right pane"
```

---

### Task 5: Create the Detail Content component (center pane)

**Files:**
- Create: `apps/web/src/features/chunks/detail/chunk-detail-content.tsx`

**Context:** The center pane. Renders meta row, title, summary, markdown content, and decision context callout.

- [ ] **Step 1: Create the component**

```typescript
// apps/web/src/features/chunks/detail/chunk-detail-content.tsx
import { Bot, Clock, Scale, Star } from "lucide-react";

import { Badge } from "@/components/ui/badge";
import { ChunkLinkRenderer } from "@/features/chunks/chunk-link-renderer";
import { ChunkTypeIcon } from "@/features/chunks/chunk-type-icon";
import { estimateReadingTime } from "@/features/chunks/reading-time";
import { MarkdownRenderer } from "@/components/markdown-renderer";
import { StalenessBanner } from "@/features/staleness/staleness-banner";

export interface ChunkDetailContentProps {
    chunkId: string;
    type: string;
    title: string;
    content: string;
    summary?: string | null;
    updatedAt: string | Date;
    isAi?: boolean;
    reviewStatus?: string;
    isFavorite?: boolean;
    onToggleFavorite?: () => void;
    rationale?: string | null;
    alternatives?: string[] | null;
    consequences?: string | null;
    readerClasses: string;
}

function relativeDate(date: Date): string {
    const diff = Date.now() - date.getTime();
    const days = Math.floor(diff / (1000 * 60 * 60 * 24));
    if (days === 0) return "today";
    if (days === 1) return "yesterday";
    if (days < 7) return `${days} days ago`;
    if (days < 30) return `${Math.floor(days / 7)}w ago`;
    if (days < 365) return `${Math.floor(days / 30)}mo ago`;
    return `${Math.floor(days / 365)}y ago`;
}

export function ChunkDetailContent({
    chunkId,
    type,
    title,
    content,
    summary,
    updatedAt,
    isAi,
    reviewStatus,
    isFavorite,
    onToggleFavorite,
    rationale,
    alternatives,
    consequences,
    readerClasses,
}: ChunkDetailContentProps) {
    const updated = new Date(updatedAt);
    const reading = estimateReadingTime(content);
    const hasDecisionContext = !!rationale || (alternatives && alternatives.length > 0) || !!consequences;

    return (
        <div className="flex-1 min-w-0 max-w-[760px] mx-auto" data-focus-main="true">
            {/* Meta row */}
            <div className="mb-4 flex flex-wrap items-center gap-x-3 gap-y-1 text-xs text-muted-foreground">
                <span className="inline-flex items-center gap-1.5">
                    <ChunkTypeIcon type={type} className="size-3.5" />
                    <span className="font-mono">{type}</span>
                </span>
                <span>·</span>
                <span className="inline-flex items-center gap-1">
                    <Clock className="size-3" />
                    {reading.label}
                </span>
                <span>·</span>
                <span>Updated {relativeDate(updated)}</span>
                {isAi && (
                    <>
                        <span>·</span>
                        <Badge
                            variant="outline"
                            size="sm"
                            className={
                                reviewStatus === "draft"
                                    ? "border-yellow-500/30 bg-yellow-500/10 text-yellow-600"
                                    : reviewStatus === "reviewed"
                                      ? "border-blue-500/30 bg-blue-500/10 text-blue-600"
                                      : "border-green-500/30 bg-green-500/10 text-green-600"
                            }
                        >
                            <Bot className="mr-1 size-3" />
                            AI {reviewStatus === "draft" ? "Draft" : reviewStatus === "reviewed" ? "Reviewed" : "Approved"}
                        </Badge>
                    </>
                )}
            </div>

            {/* Title with favorite */}
            <div className="mb-3 flex items-start gap-3">
                <h1 className="text-3xl font-bold tracking-tight leading-tight flex-1">{title}</h1>
                {onToggleFavorite && (
                    <button
                        type="button"
                        onClick={onToggleFavorite}
                        className="mt-1.5 text-muted-foreground hover:text-yellow-500 transition-colors"
                        title={isFavorite ? "Remove from favorites" : "Add to favorites"}
                    >
                        <Star className={`size-5 ${isFavorite ? "fill-yellow-500 text-yellow-500" : ""}`} />
                    </button>
                )}
            </div>

            {/* Summary */}
            {summary && (
                <p className="mb-6 text-lg italic text-muted-foreground leading-relaxed">{summary}</p>
            )}

            {/* Staleness banner */}
            <StalenessBanner chunkId={chunkId} />

            {/* Content */}
            <div className={`prose dark:prose-invert max-w-none ${readerClasses}`}>
                <ChunkLinkRenderer content={content} currentChunkId={chunkId} />
            </div>

            {/* Decision context callout */}
            {hasDecisionContext && (
                <aside className="mt-10 rounded-md border-l-2 border-amber-500/40 bg-amber-500/5 px-5 py-4">
                    <div className="mb-3 flex items-center gap-1.5 text-xs font-semibold uppercase tracking-wider text-amber-600 dark:text-amber-400">
                        <Scale className="size-3.5" />
                        Decision context
                    </div>
                    {rationale && (
                        <div className="mb-4">
                            <div className="mb-1 text-xs font-semibold text-muted-foreground">Rationale</div>
                            <div className="prose prose-sm dark:prose-invert max-w-none text-muted-foreground">
                                <MarkdownRenderer>{rationale}</MarkdownRenderer>
                            </div>
                        </div>
                    )}
                    {alternatives && alternatives.length > 0 && (
                        <div className="mb-4">
                            <div className="mb-1 text-xs font-semibold text-muted-foreground">Alternatives considered</div>
                            <ul className="text-sm text-muted-foreground list-disc pl-5 space-y-1">
                                {alternatives.map((alt, i) => (
                                    <li key={i}>{alt}</li>
                                ))}
                            </ul>
                        </div>
                    )}
                    {consequences && (
                        <div>
                            <div className="mb-1 text-xs font-semibold text-muted-foreground">Consequences</div>
                            <div className="prose prose-sm dark:prose-invert max-w-none text-muted-foreground">
                                <MarkdownRenderer>{consequences}</MarkdownRenderer>
                            </div>
                        </div>
                    )}
                </aside>
            )}
        </div>
    );
}
```

- [ ] **Step 2: Verify type check**

Run: `pnpm --filter web run check-types 2>&1 | grep chunk-detail-content`

Expected: No errors.

- [ ] **Step 3: Commit**

```bash
git add apps/web/src/features/chunks/detail/chunk-detail-content.tsx
git commit -m "feat(chunks): add ChunkDetailContent for center reading pane"
```

---

### Task 6: Create the More Context Drawer

**Files:**
- Create: `apps/web/src/features/chunks/detail/more-context-drawer.tsx`
- Create: `apps/web/src/features/chunks/detail/more-context-links-tab.tsx`
- Create: `apps/web/src/features/chunks/detail/more-context-context-tab.tsx`

**Context:** The drawer uses the existing `Sheet` component (`apps/web/src/components/ui/sheet.tsx`). Four tabs: Links, Context, Comments, History.

- [ ] **Step 1: Read the existing Sheet component**

Read `apps/web/src/components/ui/sheet.tsx` to understand the API. Look at the exported components (`Sheet`, `SheetTrigger`, `SheetContent`, `SheetHeader`, etc.) and how side is specified ("right" / "left" / etc.).

- [ ] **Step 2: Create the Links tab**

```typescript
// apps/web/src/features/chunks/detail/more-context-links-tab.tsx
import { ChevronRight } from "lucide-react";

import { Badge } from "@/components/ui/badge";
import { ChunkLink } from "@/features/chunks/chunk-link";
import { DeleteConnectionButton } from "@/features/chunks/delete-connection-button";
import { DependencyTree } from "@/features/chunks/dependency-tree";
import { RelatedChunks } from "@/features/chunks/related-chunks";
import { RelatedSuggestions } from "@/features/chunks/related-suggestions";
import { relationColor } from "@/features/chunks/relation-colors";
import { SuggestedConnections } from "@/features/chunks/suggested-connections";

export interface MoreContextLinksTabProps {
    chunkId: string;
    outgoing: Array<{ id: string; sourceId: string; targetId: string; targetTitle?: string; targetType?: string; relation: string }>;
    incoming: Array<{ id: string; sourceId: string; targetId: string; sourceTitle?: string; sourceType?: string; relation: string }>;
}

export function MoreContextLinksTab({ chunkId, outgoing, incoming }: MoreContextLinksTabProps) {
    return (
        <div className="space-y-6 px-1 pb-4">
            {/* Outgoing */}
            {outgoing.length > 0 && (
                <section>
                    <h3 className="mb-2 text-xs font-semibold uppercase tracking-wider text-muted-foreground">
                        Outgoing ({outgoing.length})
                    </h3>
                    <div className="space-y-1">
                        {outgoing.map(conn => (
                            <div key={conn.id} className="flex items-center gap-2 rounded border px-3 py-2 text-sm">
                                <Badge variant="outline" size="sm" style={{ borderColor: relationColor(conn.relation) }}>
                                    {conn.relation}
                                </Badge>
                                <ChunkLink
                                    chunkId={conn.targetId}
                                    title={conn.targetTitle ?? conn.targetId}
                                    type={conn.targetType ?? "note"}
                                    className="flex-1 truncate"
                                />
                                <DeleteConnectionButton connectionId={conn.id} />
                            </div>
                        ))}
                    </div>
                </section>
            )}

            {/* Incoming */}
            {incoming.length > 0 && (
                <section>
                    <h3 className="mb-2 text-xs font-semibold uppercase tracking-wider text-muted-foreground">
                        Incoming ({incoming.length})
                    </h3>
                    <div className="space-y-1">
                        {incoming.map(conn => (
                            <div key={conn.id} className="flex items-center gap-2 rounded border px-3 py-2 text-sm">
                                <Badge variant="outline" size="sm" style={{ borderColor: relationColor(conn.relation) }}>
                                    {conn.relation}
                                </Badge>
                                <ChunkLink
                                    chunkId={conn.sourceId}
                                    title={conn.sourceTitle ?? conn.sourceId}
                                    type={conn.sourceType ?? "note"}
                                    className="flex-1 truncate"
                                />
                            </div>
                        ))}
                    </div>
                </section>
            )}

            {/* Dependency tree */}
            <section>
                <h3 className="mb-2 text-xs font-semibold uppercase tracking-wider text-muted-foreground">
                    Dependency tree
                </h3>
                <DependencyTree chunkId={chunkId} />
            </section>

            {/* Suggested connections */}
            <section>
                <h3 className="mb-2 text-xs font-semibold uppercase tracking-wider text-muted-foreground">
                    Suggested connections
                </h3>
                <SuggestedConnections chunkId={chunkId} />
            </section>

            {/* Related chunks */}
            <section>
                <h3 className="mb-2 text-xs font-semibold uppercase tracking-wider text-muted-foreground">
                    Related chunks
                </h3>
                <RelatedChunks chunkId={chunkId} />
            </section>

            {/* Related suggestions */}
            <section>
                <h3 className="mb-2 text-xs font-semibold uppercase tracking-wider text-muted-foreground">
                    Semantic suggestions
                </h3>
                <RelatedSuggestions chunkId={chunkId} />
            </section>
        </div>
    );
}
```

- [ ] **Step 3: Create the Context tab**

```typescript
// apps/web/src/features/chunks/detail/more-context-context-tab.tsx
import { Code, FileCode } from "lucide-react";

import { AiSection } from "@/features/chunks/ai-section";

export interface MoreContextContextTabProps {
    chunkId: string;
    chunk: any;
    appliesTo?: Array<{ id: string; pattern: string; note?: string | null }>;
    fileReferences?: Array<{ id: string; path: string; anchor?: string | null; relation: string }>;
}

export function MoreContextContextTab({
    chunkId,
    chunk,
    appliesTo,
    fileReferences,
}: MoreContextContextTabProps) {
    return (
        <div className="space-y-6 px-1 pb-4">
            {/* Applies to */}
            <section>
                <h3 className="mb-2 flex items-center gap-1.5 text-xs font-semibold uppercase tracking-wider text-muted-foreground">
                    <Code className="size-3.5" />
                    Applies to
                </h3>
                {appliesTo && appliesTo.length > 0 ? (
                    <div className="space-y-1">
                        {appliesTo.map(applies => (
                            <div key={applies.id} className="rounded border px-3 py-2 text-sm">
                                <code className="font-mono text-xs">{applies.pattern}</code>
                                {applies.note && (
                                    <p className="mt-1 text-xs text-muted-foreground">{applies.note}</p>
                                )}
                            </div>
                        ))}
                    </div>
                ) : (
                    <p className="text-xs text-muted-foreground">No file patterns associated.</p>
                )}
            </section>

            {/* File references */}
            <section>
                <h3 className="mb-2 flex items-center gap-1.5 text-xs font-semibold uppercase tracking-wider text-muted-foreground">
                    <FileCode className="size-3.5" />
                    File references
                </h3>
                {fileReferences && fileReferences.length > 0 ? (
                    <div className="space-y-1">
                        {fileReferences.map(ref => (
                            <div key={ref.id} className="rounded border px-3 py-2 text-sm">
                                <code className="font-mono text-xs">{ref.path}</code>
                                {ref.anchor && (
                                    <span className="ml-2 text-xs text-muted-foreground">@ {ref.anchor}</span>
                                )}
                                <span className="ml-2 text-xs text-muted-foreground">({ref.relation})</span>
                            </div>
                        ))}
                    </div>
                ) : (
                    <p className="text-xs text-muted-foreground">No file references.</p>
                )}
            </section>

            {/* AI enrichment */}
            <section>
                <h3 className="mb-2 text-xs font-semibold uppercase tracking-wider text-muted-foreground">
                    AI enrichment
                </h3>
                <AiSection chunk={chunk} />
            </section>
        </div>
    );
}
```

- [ ] **Step 4: Create the drawer shell**

```typescript
// apps/web/src/features/chunks/detail/more-context-drawer.tsx
import { Layers, MessageSquare, History as HistoryIcon, Network } from "lucide-react";
import { useEffect, useState } from "react";

import { Sheet, SheetContent, SheetHeader, SheetTitle } from "@/components/ui/sheet";
import { Badge } from "@/components/ui/badge";
import { ChunkComments } from "@/features/chunks/chunk-comments";
import { VersionHistory } from "@/features/chunks/version-history";

import { MoreContextContextTab } from "./more-context-context-tab";
import { MoreContextLinksTab } from "./more-context-links-tab";

export type DrawerTab = "links" | "context" | "comments" | "history";

export interface MoreContextDrawerProps {
    open: boolean;
    onOpenChange: (open: boolean) => void;
    chunkId: string;
    chunk: any;
    outgoing: any[];
    incoming: any[];
    appliesTo?: any[];
    fileReferences?: any[];
    initialTab?: DrawerTab;
}

const STORAGE_KEY = "chunk-detail-drawer-tab";

export function MoreContextDrawer({
    open,
    onOpenChange,
    chunkId,
    chunk,
    outgoing,
    incoming,
    appliesTo,
    fileReferences,
    initialTab,
}: MoreContextDrawerProps) {
    const [tab, setTab] = useState<DrawerTab>(initialTab ?? "links");

    useEffect(() => {
        if (initialTab) {
            setTab(initialTab);
            return;
        }
        try {
            const stored = sessionStorage.getItem(STORAGE_KEY) as DrawerTab | null;
            if (stored && ["links", "context", "comments", "history"].includes(stored)) {
                setTab(stored);
            }
        } catch {
            // ignore
        }
    }, [initialTab]);

    function changeTab(next: DrawerTab) {
        setTab(next);
        try {
            sessionStorage.setItem(STORAGE_KEY, next);
        } catch {
            // ignore
        }
    }

    const linkCount = outgoing.length + incoming.length;
    const contextCount = (appliesTo?.length ?? 0) + (fileReferences?.length ?? 0);

    return (
        <Sheet open={open} onOpenChange={onOpenChange}>
            <SheetContent side="right" className="w-full sm:max-w-[480px] flex flex-col">
                <SheetHeader>
                    <SheetTitle>More context</SheetTitle>
                </SheetHeader>

                {/* Tab strip */}
                <div className="flex gap-1 border-b pb-2 mt-2">
                    <TabButton active={tab === "links"} onClick={() => changeTab("links")}>
                        <Network className="size-3.5" />
                        Links
                        {linkCount > 0 && <Badge variant="secondary" size="sm">{linkCount}</Badge>}
                    </TabButton>
                    <TabButton active={tab === "context"} onClick={() => changeTab("context")}>
                        <Layers className="size-3.5" />
                        Context
                        {contextCount > 0 && <Badge variant="secondary" size="sm">{contextCount}</Badge>}
                    </TabButton>
                    <TabButton active={tab === "comments"} onClick={() => changeTab("comments")}>
                        <MessageSquare className="size-3.5" />
                        Comments
                    </TabButton>
                    <TabButton active={tab === "history"} onClick={() => changeTab("history")}>
                        <HistoryIcon className="size-3.5" />
                        History
                    </TabButton>
                </div>

                {/* Tab content */}
                <div className="flex-1 overflow-y-auto mt-4">
                    {tab === "links" && (
                        <MoreContextLinksTab chunkId={chunkId} outgoing={outgoing} incoming={incoming} />
                    )}
                    {tab === "context" && (
                        <MoreContextContextTab
                            chunkId={chunkId}
                            chunk={chunk}
                            appliesTo={appliesTo}
                            fileReferences={fileReferences}
                        />
                    )}
                    {tab === "comments" && <ChunkComments chunkId={chunkId} />}
                    {tab === "history" && <VersionHistory chunkId={chunkId} />}
                </div>
            </SheetContent>
        </Sheet>
    );
}

function TabButton({ active, onClick, children }: { active: boolean; onClick: () => void; children: React.ReactNode }) {
    return (
        <button
            type="button"
            onClick={onClick}
            className={`flex items-center gap-1.5 rounded px-3 py-1.5 text-xs font-medium transition-colors ${
                active ? "bg-muted text-foreground" : "text-muted-foreground hover:bg-muted/50"
            }`}
        >
            {children}
        </button>
    );
}
```

- [ ] **Step 5: Verify type check**

Run: `pnpm --filter web run check-types 2>&1 | grep -E "more-context"`

Expected: No new errors in these three files. Adapt component props if `AiSection`, `RelatedChunks`, etc. have different signatures.

- [ ] **Step 6: Commit**

```bash
git add apps/web/src/features/chunks/detail/more-context-drawer.tsx apps/web/src/features/chunks/detail/more-context-links-tab.tsx apps/web/src/features/chunks/detail/more-context-context-tab.tsx
git commit -m "feat(chunks): add More Context drawer with Links, Context, Comments, History tabs"
```

---

### Task 7: Rewrite the route component

**Files:**
- Modify: `apps/web/src/routes/chunks.$chunkId.tsx` (full rewrite)

**Context:** The route becomes a thin composition. All the old inline UI gets removed. The new components are composed into a three-pane layout with the drawer and floating trigger button.

- [ ] **Step 1: Replace the file contents**

Read the current file to preserve the route definition (`createFileRoute`, `beforeLoad`, auth logic) and the data fetch/mutation setup. Keep:
- The `Route` export with `createFileRoute` and `beforeLoad`
- All `useQuery` / `useMutation` logic
- Loading skeleton
- Error state
- Hooks: `useRecentChunks`, `useRecentlyViewed`, `useReadingTrail`, `useFavorites`, `useFocusMode`, `useReaderSettings`
- Scroll progress state and effect

Delete:
- The inline JSX for action bar (replaced by `ChunkDetailTopBar`)
- The old title/metadata block (replaced by `ChunkDetailContent`)
- The `InlineTagEditor` inline (moves to `ChunkMetadataPanel`)
- The collapsible sections (all move to `MoreContextDrawer`)
- The `ChunkComments` inline function (already extracted in Task 1)
- The old `lg:grid` content+sidebar layout (replaced by new layout)

Here's the new return statement structure:

```typescript
return (
    <>
        {/* Scroll progress bar */}
        <div className="fixed top-0 left-0 right-0 z-50 h-0.5 bg-transparent print:hidden">
            <div
                className="h-full bg-primary transition-[width] duration-100 ease-out"
                style={{ width: `${scrollProgress}%` }}
            />
        </div>

        <div className="container mx-auto max-w-[1400px] px-4 py-8">
            <ChunkDetailTopBar
                chunkId={chunkId}
                title={chunk.title}
                content={chunk.content}
                type={chunk.type}
                isEntryPoint={isEntryPoint}
                isAi={isAi}
                reviewStatus={reviewStatus}
                onArchive={() => archiveMutation.mutate()}
                onDelete={() => deleteMutation.mutate()}
                onSplit={() => setShowSplitDialog(true)}
                onToggleEntryPoint={() => toggleEntryPointMutation.mutate()}
                onReview={(status) => reviewMutation.mutate(status)}
                archivePending={archiveMutation.isPending}
                deletePending={deleteMutation.isPending}
            />

            <div className="flex gap-8">
                <ChunkSiblingNavigator
                    currentChunkId={chunkId}
                    codebaseId={currentCodebases?.[0]?.id}
                    codebaseName={currentCodebases?.[0]?.name}
                />

                <ChunkDetailContent
                    chunkId={chunkId}
                    type={chunk.type}
                    title={chunk.title}
                    content={chunk.content}
                    summary={chunk.summary}
                    updatedAt={chunk.updatedAt}
                    isAi={isAi}
                    reviewStatus={reviewStatus}
                    isFavorite={isFavorite(chunkId)}
                    onToggleFavorite={() => toggleFavorite(chunkId)}
                    rationale={rationale}
                    alternatives={alternatives}
                    consequences={consequences}
                    readerClasses={readerClasses}
                />

                <ChunkMetadataPanel
                    content={chunk.content}
                    tags={tags}
                    onTagsUpdate={(newTags) => tagMutation.mutate(newTags)}
                    tagsLoading={tagMutation.isPending}
                    type={chunk.type}
                    createdAt={chunk.createdAt}
                    updatedAt={chunk.updatedAt}
                    connectionCount={connections.length}
                    codebases={currentCodebases}
                    origin={origin}
                    reviewStatus={reviewStatus}
                    healthScore={healthScore}
                    onShowConnections={() => {
                        setDrawerTab("links");
                        setDrawerOpen(true);
                    }}
                />
            </div>

            {/* Floating More Context button */}
            <button
                type="button"
                onClick={() => setDrawerOpen(true)}
                className="fixed bottom-6 right-6 z-30 inline-flex items-center gap-2 rounded-full border border-indigo-500/30 bg-indigo-500/15 px-4 py-2 text-xs font-semibold text-indigo-400 shadow-lg backdrop-blur hover:bg-indigo-500/25 transition-colors print:hidden"
                data-focus-hide="true"
                title="More context (m)"
            >
                ▸ More context
                {totalSignals > 0 && (
                    <span className="text-[10px] text-indigo-400/60 font-mono">
                        ({totalSignals})
                    </span>
                )}
            </button>

            <MoreContextDrawer
                open={drawerOpen}
                onOpenChange={setDrawerOpen}
                chunkId={chunkId}
                chunk={chunk}
                outgoing={outgoing}
                incoming={incoming}
                appliesTo={appliesTo}
                fileReferences={fileReferences}
                initialTab={drawerTab}
            />

            <SplitChunkDialog
                open={showSplitDialog}
                onOpenChange={setShowSplitDialog}
                chunkId={chunkId}
                title={chunk.title}
                content={chunk.content}
                type={chunk.type}
                tags={[]}
            />
        </div>
    </>
);
```

Add state declarations at the top:
```typescript
const [drawerOpen, setDrawerOpen] = useState(false);
const [drawerTab, setDrawerTab] = useState<DrawerTab>("links");
const [showSplitDialog, setShowSplitDialog] = useState(false);

const totalSignals = connections.length + (appliesTo?.length ?? 0) + (fileReferences?.length ?? 0);
```

Add keyboard shortcut for `m`:
```typescript
useEffect(() => {
    function handleKey(e: KeyboardEvent) {
        const tag = document.activeElement?.tagName;
        if (tag === "INPUT" || tag === "TEXTAREA" || tag === "SELECT") return;
        if (e.metaKey || e.ctrlKey || e.altKey) return;
        if (e.key === "m") {
            e.preventDefault();
            setDrawerOpen(prev => !prev);
        }
    }
    window.addEventListener("keydown", handleKey);
    return () => window.removeEventListener("keydown", handleKey);
}, []);
```

Add imports at the top:
```typescript
import { ChunkDetailTopBar } from "@/features/chunks/detail/chunk-detail-top-bar";
import { ChunkDetailContent } from "@/features/chunks/detail/chunk-detail-content";
import { ChunkSiblingNavigator } from "@/features/chunks/detail/chunk-sibling-navigator";
import { ChunkMetadataPanel } from "@/features/chunks/detail/chunk-metadata-panel";
import { MoreContextDrawer, type DrawerTab } from "@/features/chunks/detail/more-context-drawer";
```

Remove imports that are no longer used (Archive, ArrowLeft, etc. — keep only what's still referenced).

- [ ] **Step 2: Type check and fix**

Run: `pnpm --filter web run check-types 2>&1 | grep chunks.\\\$chunkId`

Fix any import errors, missing props, or type mismatches. The route file should drop from ~818 lines to under 300.

- [ ] **Step 3: Smoke test**

Run `pnpm dev` and navigate to a chunk detail page:
1. Three panes visible on wide screens
2. Top bar shows Back + Favorite + Export + Edit + ⋯
3. Left pane shows sibling chunks (if the chunk has a codebase)
4. Center shows title + content + decision context (if present)
5. Right pane shows ToC + Details with tags + metadata
6. Floating "More context" button bottom-right
7. Clicking it opens the drawer with 4 tabs
8. Each tab shows its content (Links, Context, Comments, History)
9. Press `m` to toggle drawer
10. Press `f` to toggle focus mode → side panes hide
11. All existing actions still work (edit, archive, delete, favorite, entry point)

- [ ] **Step 4: Commit**

```bash
git add apps/web/src/routes/chunks.\$chunkId.tsx
git commit -m "feat(chunks): rewrite detail page with three-pane layout and More Context drawer"
```

---

### Task 8: Final Verification

**Files:** (verification only)

- [ ] **Step 1: Full type check**

Run: `pnpm --filter web run check-types 2>&1 | head -30`

Expected: No new errors in `chunk-detail-*` or `chunks.$chunkId.tsx`.

- [ ] **Step 2: Build**

Run: `pnpm build --filter=web`

Expected: Success.

- [ ] **Step 3: Full manual checklist**

Load a chunk with plenty of connections, comments, file refs, and a rationale. Verify:

| Feature | Expected |
|---------|----------|
| Top bar | 4 buttons + ⋯ menu visible |
| ⋯ menu | All old actions accessible (Focus, Find path, Similar, Split, Entry point, Review, Archive, Delete) |
| Left pane | Siblings visible if in a codebase, prev/next work |
| Content pane | Type + reading time + updated row, title, summary, staleness banner, markdown with auto-links |
| Decision context | Amber callout appears when rationale/alternatives/consequences exist |
| Right pane | ToC generated from headings, Details with health, tags, type, etc. |
| Drawer trigger | Floating button bottom-right shows signal count |
| Drawer → Links | Outgoing + incoming + deps + suggested + related all shown |
| Drawer → Context | Applies-to, file-refs, AI enrichment |
| Drawer → Comments | Full CRUD works |
| Drawer → History | Version history renders |
| Keyboard: m | Opens/closes drawer |
| Keyboard: f | Toggles focus mode; side panes hide |
| Keyboard: h / l | Navigates prev/next sibling |
| Favorite star | Toggles correctly |
| Export dropdown | Copy and Download both work |
| Mobile (resize window) | Layout degrades gracefully, top bar compact |

- [ ] **Step 4: Commit fixes if needed**

If any smoke tests fail, fix inline and commit:
```bash
git commit -am "fix(chunks): resolve smoke test issues in detail redesign"
```
