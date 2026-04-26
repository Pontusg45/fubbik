# Smart Link Renderer Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Extend `MarkdownRenderer` with a `SmartLinkProvider` context so all markdown surfaces auto-link backticked terms to chunks/file-refs and highlight vocabulary terms with hover popovers — without callsite changes.

**Architecture:** A `SmartLinkProvider` near the app root fetches chunk titles, vocabulary entries, and file refs into cached lookup maps. `MarkdownRenderer` consumes this context via `useContext` and does matching inside component overrides (`components.code` for backticked terms, a rehype text-node plugin for vocabulary in prose). `ChunkLinkRenderer` is deleted; its sole callsite switches to plain `<MarkdownRenderer>`.

**Tech Stack:** React context, TanStack Query, react-markdown component overrides, custom rehype plugin, Tailwind CSS

---

## File Structure

| File | Action | Responsibility |
|---|---|---|
| `apps/web/src/components/smart-link-provider.tsx` | **Create** | Context + provider + three cached queries + matching functions |
| `apps/web/src/components/smart-link-provider.test.ts` | **Create** | Unit tests for matching logic (pure functions) |
| `apps/web/src/components/vocabulary-popover.tsx` | **Create** | Hover card for vocabulary terms |
| `apps/web/src/components/markdown-renderer.tsx` | **Modify** | Consume context, update `code` override, add rehype plugin for vocabulary text matching |
| `apps/web/src/features/chunks/chunk-link-renderer.tsx` | **Delete** | Replaced by MarkdownRenderer internals |
| `apps/web/src/features/chunks/detail/chunk-detail-content.tsx` | **Modify** | Replace `ChunkLinkRenderer` with `MarkdownRenderer` |
| `apps/web/src/features/documents/document-browser.tsx` | **Modify** | Remove `handleContentClick`, `docTitleMap`, `allDocsQuery` |
| `apps/web/src/routes/__root.tsx` | **Modify** | Add `<SmartLinkProvider>` |
| `packages/api/src/file-refs/routes.ts` | **Modify** | Add `GET /api/file-refs` list-all endpoint |
| `packages/api/src/file-refs/service.ts` | **Modify** | Add `listAllFileRefs` function |
| `packages/db/src/repository/file-ref.ts` | **Modify** | Add `listAllFileRefsForUser` query |

---

### Task 1: Add list-all file refs API endpoint

The `SmartLinkProvider` needs all file refs in a single query. Currently only per-chunk and per-path lookups exist. Add a lightweight `GET /api/file-refs` endpoint.

**Files:**
- Modify: `packages/db/src/repository/file-ref.ts`
- Modify: `packages/api/src/file-refs/service.ts`
- Modify: `packages/api/src/file-refs/routes.ts`

- [ ] **Step 1: Add repository function**

In `packages/db/src/repository/file-ref.ts`, add this function after the existing `lookupChunksByFilePath`:

```ts
export function listAllFileRefs(userId: string) {
    return dbEffect(() =>
        db
            .select({
                chunkId: chunk.id,
                chunkTitle: chunk.title,
                path: chunkFileRef.path,
                anchor: chunkFileRef.anchor
            })
            .from(chunkFileRef)
            .innerJoin(chunk, eq(chunkFileRef.chunkId, chunk.id))
            .where(eq(chunk.userId, userId))
    );
}
```

- [ ] **Step 2: Export from repository index**

In `packages/db/src/repository/index.ts`, find the file-ref exports and add `listAllFileRefs`:

```ts
export { getFileRefsForChunk, getFileRefsForChunks, setFileRefsForChunk, lookupChunksByFilePath, listAllFileRefs } from "./file-ref";
```

- [ ] **Step 3: Add service function**

In `packages/api/src/file-refs/service.ts`, add:

```ts
export function listAll(userId: string) {
    return listAllFileRefs(userId);
}
```

And update the import at the top:

```ts
import { getChunkById, getFileRefsForChunk, lookupChunksByFilePath, setFileRefsForChunk, listAllFileRefs } from "@fubbik/db/repository";
```

- [ ] **Step 4: Add route**

In `packages/api/src/file-refs/routes.ts`, add a new GET route before the existing `/file-refs/lookup` route:

```ts
    .get(
        "/file-refs",
        ctx =>
            Effect.runPromise(
                requireSession(ctx).pipe(Effect.flatMap(session => fileRefService.listAll(session.user.id)))
            )
    )
```

- [ ] **Step 5: Verify the endpoint works**

Run: `curl -s http://localhost:3000/api/file-refs -b <cookie> | jq '.[:2]'`

Expected: JSON array of objects with `chunkId`, `chunkTitle`, `path`, `anchor`.

- [ ] **Step 6: Commit**

```bash
git add packages/db/src/repository/file-ref.ts packages/db/src/repository/index.ts packages/api/src/file-refs/service.ts packages/api/src/file-refs/routes.ts
git commit -m "feat: add GET /api/file-refs list-all endpoint for smart link provider"
```

---

### Task 2: Create SmartLinkProvider — matching logic and tests

Build the pure matching functions first, test them, then wire up the provider.

**Files:**
- Create: `apps/web/src/components/smart-link-provider.tsx`
- Create: `apps/web/src/components/smart-link-provider.test.ts`

- [ ] **Step 1: Write tests for matching logic**

Create `apps/web/src/components/smart-link-provider.test.ts`:

```ts
import { describe, expect, it } from "vitest";
import { buildChunkIndex, buildVocabularyIndex, buildFileRefIndex, matchInCode, matchVocabularyInText } from "./smart-link-provider";

describe("buildChunkIndex", () => {
    it("indexes by lowercase title", () => {
        const index = buildChunkIndex([
            { id: "c1", title: "UserService", aliases: [] },
            { id: "c2", title: "Auth Flow", aliases: ["authentication"] }
        ]);
        expect(index.get("userservice")).toEqual({ id: "c1", title: "UserService" });
        expect(index.get("auth flow")).toEqual({ id: "c2", title: "Auth Flow" });
    });

    it("indexes aliases", () => {
        const index = buildChunkIndex([
            { id: "c1", title: "UserService", aliases: ["UserSvc", "user-service"] }
        ]);
        expect(index.get("usersvc")).toEqual({ id: "c1", title: "UserService" });
        expect(index.get("user-service")).toEqual({ id: "c1", title: "UserService" });
    });

    it("skips short titles (< 4 chars)", () => {
        const index = buildChunkIndex([
            { id: "c1", title: "API", aliases: [] }
        ]);
        expect(index.get("api")).toBeUndefined();
    });

    it("excludes specified chunk id", () => {
        const index = buildChunkIndex([
            { id: "c1", title: "UserService", aliases: [] },
            { id: "c2", title: "Auth Flow", aliases: [] }
        ]);
        const match = matchInCode("UserService", index, new Map(), new Map(), "c1");
        expect(match).toBeNull();
    });
});

describe("buildVocabularyIndex", () => {
    it("indexes by lowercase word", () => {
        const index = buildVocabularyIndex([
            { word: "UserService", category: "actor", expects: ["class"] }
        ]);
        expect(index.get("userservice")).toEqual({
            word: "UserService",
            category: "actor",
            expects: ["class"]
        });
    });
});

describe("buildFileRefIndex", () => {
    it("indexes by filename and full path", () => {
        const index = buildFileRefIndex([
            { chunkId: "c1", chunkTitle: "Auth Module", path: "src/auth/service.ts", anchor: null }
        ]);
        expect(index.get("src/auth/service.ts")).toEqual({ chunkId: "c1", chunkTitle: "Auth Module", path: "src/auth/service.ts" });
        expect(index.get("service.ts")).toEqual({ chunkId: "c1", chunkTitle: "Auth Module", path: "src/auth/service.ts" });
    });
});

describe("matchInCode", () => {
    const chunks = buildChunkIndex([
        { id: "c1", title: "UserService", aliases: [] },
        { id: "c2", title: "AuthFlow", aliases: [] }
    ]);
    const vocab = buildVocabularyIndex([
        { word: "Repository", category: "actor", expects: ["class"] }
    ]);
    const fileRefs = buildFileRefIndex([
        { chunkId: "c3", chunkTitle: "Config", path: "src/config.ts", anchor: null }
    ]);

    it("returns chunk match (highest priority)", () => {
        const result = matchInCode("UserService", chunks, fileRefs, vocab);
        expect(result).toEqual({ type: "chunk", id: "c1", title: "UserService" });
    });

    it("returns file ref match when no chunk matches", () => {
        const result = matchInCode("src/config.ts", chunks, fileRefs, vocab);
        expect(result).toEqual({ type: "fileRef", chunkId: "c3", chunkTitle: "Config", path: "src/config.ts" });
    });

    it("returns vocabulary match as fallback", () => {
        const result = matchInCode("Repository", chunks, fileRefs, vocab);
        expect(result).toEqual({ type: "vocabulary", word: "Repository", category: "actor", expects: ["class"] });
    });

    it("returns null when nothing matches", () => {
        const result = matchInCode("UnknownThing", chunks, fileRefs, vocab);
        expect(result).toBeNull();
    });
});

describe("matchVocabularyInText", () => {
    const vocab = buildVocabularyIndex([
        { word: "UserService", category: "actor", expects: ["class"] },
        { word: "deploy", category: "action", expects: null }
    ]);

    it("finds vocabulary terms in plain text", () => {
        const matches = matchVocabularyInText("The UserService handles deploy requests", vocab);
        expect(matches).toHaveLength(2);
        expect(matches[0]).toEqual({ start: 4, end: 15, word: "UserService", category: "actor", expects: ["class"] });
        expect(matches[1]).toEqual({ start: 24, end: 30, word: "deploy", category: "action", expects: null });
    });

    it("returns empty for no matches", () => {
        const matches = matchVocabularyInText("nothing relevant here", vocab);
        expect(matches).toHaveLength(0);
    });

    it("matches case-insensitively", () => {
        const matches = matchVocabularyInText("the userservice is important", vocab);
        expect(matches).toHaveLength(1);
        expect(matches[0]?.word).toBe("UserService");
    });
});
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cd apps/web && pnpm vitest run src/components/smart-link-provider.test.ts`

Expected: FAIL — module not found.

- [ ] **Step 3: Create `smart-link-provider.tsx` with matching logic and provider**

Create `apps/web/src/components/smart-link-provider.tsx`:

```tsx
import { useQuery } from "@tanstack/react-query";
import { createContext, useContext, useMemo, type ReactNode } from "react";

import { api } from "@/utils/api";
import { unwrapEden } from "@/utils/eden";

/* ─── Types ─── */

export interface ChunkMatch {
    id: string;
    title: string;
}

export interface VocabularyMatch {
    word: string;
    category: string;
    expects: string[] | null;
}

export interface FileRefMatch {
    chunkId: string;
    chunkTitle: string;
    path: string;
}

export type CodeMatch =
    | ({ type: "chunk" } & ChunkMatch)
    | ({ type: "fileRef" } & FileRefMatch)
    | ({ type: "vocabulary" } & VocabularyMatch);

export interface VocabularyTextMatch extends VocabularyMatch {
    start: number;
    end: number;
}

/* ─── Index builders (exported for testing) ─── */

export function buildChunkIndex(
    chunks: { id: string; title: string; aliases: string[] }[]
): Map<string, ChunkMatch> {
    const map = new Map<string, ChunkMatch>();
    for (const c of chunks) {
        if (c.title.length >= 4) {
            map.set(c.title.toLowerCase(), { id: c.id, title: c.title });
        }
        for (const alias of c.aliases) {
            if (alias.length >= 4) {
                map.set(alias.toLowerCase(), { id: c.id, title: c.title });
            }
        }
    }
    return map;
}

export function buildVocabularyIndex(
    entries: { word: string; category: string; expects: string[] | null }[]
): Map<string, VocabularyMatch> {
    const map = new Map<string, VocabularyMatch>();
    for (const e of entries) {
        map.set(e.word.toLowerCase(), { word: e.word, category: e.category, expects: e.expects ?? null });
    }
    return map;
}

export function buildFileRefIndex(
    refs: { chunkId: string; chunkTitle: string; path: string; anchor: string | null }[]
): Map<string, FileRefMatch> {
    const map = new Map<string, FileRefMatch>();
    for (const r of refs) {
        const entry = { chunkId: r.chunkId, chunkTitle: r.chunkTitle, path: r.path };
        map.set(r.path.toLowerCase(), entry);
        // Also index by filename alone for shorter backtick references
        const filename = r.path.split("/").pop();
        if (filename && !map.has(filename.toLowerCase())) {
            map.set(filename.toLowerCase(), entry);
        }
    }
    return map;
}

/* ─── Matching functions (exported for testing) ─── */

/**
 * Match a backticked term. Priority: chunk > fileRef > vocabulary.
 * Optionally exclude a chunk ID (for self-link prevention on detail pages).
 */
export function matchInCode(
    text: string,
    chunkIndex: Map<string, ChunkMatch>,
    fileRefIndex: Map<string, FileRefMatch>,
    vocabIndex: Map<string, VocabularyMatch>,
    excludeChunkId?: string
): CodeMatch | null {
    const lower = text.toLowerCase();

    const chunk = chunkIndex.get(lower);
    if (chunk && chunk.id !== excludeChunkId) {
        return { type: "chunk", ...chunk };
    }

    const fileRef = fileRefIndex.get(lower);
    if (fileRef && fileRef.chunkId !== excludeChunkId) {
        return { type: "fileRef", ...fileRef };
    }

    const vocab = vocabIndex.get(lower);
    if (vocab) {
        return { type: "vocabulary", ...vocab };
    }

    return null;
}

/**
 * Find all vocabulary term matches in a plain text string.
 * Returns matches sorted by position, longest-first for overlapping matches.
 */
export function matchVocabularyInText(
    text: string,
    vocabIndex: Map<string, VocabularyMatch>
): VocabularyTextMatch[] {
    if (vocabIndex.size === 0) return [];

    const words = Array.from(vocabIndex.keys()).sort((a, b) => b.length - a.length);
    const escaped = words.map(w => w.replace(/[.*+?^${}()|[\]\\]/g, "\\$&"));
    const pattern = new RegExp(`\\b(${escaped.join("|")})\\b`, "gi");

    const matches: VocabularyTextMatch[] = [];
    let m: RegExpExecArray | null;
    while ((m = pattern.exec(text)) !== null) {
        const vocab = vocabIndex.get(m[1]!.toLowerCase());
        if (vocab) {
            matches.push({ start: m.index, end: m.index + m[0].length, ...vocab });
        }
    }

    return matches;
}

/* ─── Context ─── */

interface SmartLinkContextValue {
    chunkIndex: Map<string, ChunkMatch>;
    vocabIndex: Map<string, VocabularyMatch>;
    fileRefIndex: Map<string, FileRefMatch>;
}

const SmartLinkContext = createContext<SmartLinkContextValue>({
    chunkIndex: new Map(),
    vocabIndex: new Map(),
    fileRefIndex: new Map()
});

export function useSmartLinks() {
    return useContext(SmartLinkContext);
}

/* ─── Provider ─── */

const STALE_TIME = 5 * 60 * 1000; // 5 minutes

export function SmartLinkProvider({ children }: { children: ReactNode }) {
    const chunksQuery = useQuery({
        queryKey: ["smart-link-chunks"],
        queryFn: async () => {
            const result = unwrapEden(await api.api.chunks.get({ query: { limit: "2000" } as any }));
            const chunks = (result as any)?.chunks ?? [];
            return chunks.map((c: any) => ({ id: c.id, title: c.title, aliases: c.aliases ?? [] }));
        },
        staleTime: STALE_TIME
    });

    const vocabQuery = useQuery({
        queryKey: ["smart-link-vocab"],
        queryFn: async () => {
            try {
                // Vocabulary requires a codebaseId — fetch without to get all.
                // If this fails (no codebase selected), return empty.
                const result = unwrapEden(await api.api.vocabulary.get({ query: {} as any }));
                return (result as any[]) ?? [];
            } catch {
                return [];
            }
        },
        staleTime: STALE_TIME
    });

    const fileRefsQuery = useQuery({
        queryKey: ["smart-link-file-refs"],
        queryFn: async () => {
            try {
                const result = unwrapEden(await (api.api as any)["file-refs"].get());
                return (result as any[]) ?? [];
            } catch {
                return [];
            }
        },
        staleTime: STALE_TIME
    });

    const value = useMemo<SmartLinkContextValue>(() => {
        const chunkIndex = buildChunkIndex(chunksQuery.data ?? []);
        const vocabIndex = buildVocabularyIndex(vocabQuery.data ?? []);
        const fileRefIndex = buildFileRefIndex(fileRefsQuery.data ?? []);
        return { chunkIndex, vocabIndex, fileRefIndex };
    }, [chunksQuery.data, vocabQuery.data, fileRefsQuery.data]);

    return (
        <SmartLinkContext.Provider value={value}>
            {children}
        </SmartLinkContext.Provider>
    );
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cd apps/web && pnpm vitest run src/components/smart-link-provider.test.ts`

Expected: All 10 tests PASS.

- [ ] **Step 5: Commit**

```bash
git add apps/web/src/components/smart-link-provider.tsx apps/web/src/components/smart-link-provider.test.ts
git commit -m "feat: add SmartLinkProvider with chunk, vocabulary, and file-ref matching"
```

---

### Task 3: Create VocabularyPopover component

A custom hover card that shows vocabulary term metadata.

**Files:**
- Create: `apps/web/src/components/vocabulary-popover.tsx`

- [ ] **Step 1: Create the component**

Create `apps/web/src/components/vocabulary-popover.tsx`:

```tsx
import { Link } from "@tanstack/react-router";
import { useEffect, useRef, useState, type ReactNode } from "react";
import { createPortal } from "react-dom";

import { Badge } from "@/components/ui/badge";

interface VocabularyPopoverProps {
    word: string;
    category: string;
    expects: string[] | null;
    children: ReactNode;
}

export function VocabularyPopover({ word, category, expects, children }: VocabularyPopoverProps) {
    const triggerRef = useRef<HTMLSpanElement>(null);
    const popoverRef = useRef<HTMLDivElement>(null);
    const [open, setOpen] = useState(false);
    const openTimer = useRef<ReturnType<typeof setTimeout>>();
    const closeTimer = useRef<ReturnType<typeof setTimeout>>();
    const [position, setPosition] = useState<{ top: number; left: number } | null>(null);

    const show = () => {
        clearTimeout(closeTimer.current);
        openTimer.current = setTimeout(() => {
            if (!triggerRef.current) return;
            const rect = triggerRef.current.getBoundingClientRect();
            const popoverHeight = 100; // estimate
            const spaceAbove = rect.top;
            const placeAbove = spaceAbove > popoverHeight + 8;

            setPosition({
                top: placeAbove ? rect.top + window.scrollY - 8 : rect.bottom + window.scrollY + 8,
                left: rect.left + window.scrollX + rect.width / 2
            });
            setOpen(true);
        }, 200);
    };

    const hide = () => {
        clearTimeout(openTimer.current);
        closeTimer.current = setTimeout(() => setOpen(false), 100);
    };

    const keepOpen = () => {
        clearTimeout(closeTimer.current);
    };

    useEffect(() => {
        return () => {
            clearTimeout(openTimer.current);
            clearTimeout(closeTimer.current);
        };
    }, []);

    return (
        <>
            <span
                ref={triggerRef}
                onMouseEnter={show}
                onMouseLeave={hide}
                className="cursor-help underline decoration-dotted underline-offset-2 decoration-muted-foreground/50"
            >
                {children}
            </span>
            {open && position && createPortal(
                <div
                    ref={popoverRef}
                    onMouseEnter={keepOpen}
                    onMouseLeave={hide}
                    className="fixed z-50 w-64 rounded-lg border border-border bg-popover p-3 shadow-md animate-in fade-in-0 zoom-in-95 duration-100"
                    style={{
                        top: position.top,
                        left: position.left,
                        transform: "translateX(-50%)"
                    }}
                >
                    <div className="flex items-center justify-between gap-2 mb-1">
                        <span className="text-sm font-medium text-foreground">{word}</span>
                        <Badge variant="secondary" size="sm" className="text-[10px] shrink-0">
                            {category}
                        </Badge>
                    </div>
                    {expects && expects.length > 0 && (
                        <p className="text-xs text-muted-foreground mb-2">
                            Expects: {expects.join(", ")}
                        </p>
                    )}
                    <Link
                        to="/vocabulary"
                        className="text-xs text-primary hover:text-primary/80 transition-colors"
                    >
                        View in vocabulary →
                    </Link>
                </div>,
                document.body
            )}
        </>
    );
}
```

- [ ] **Step 2: Verify it renders without errors**

This will be integration-tested when wired into `MarkdownRenderer`. For now, verify no TypeScript errors:

Run: `cd apps/web && pnpm tsc --noEmit --pretty 2>&1 | head -20`

Expected: No errors related to `vocabulary-popover.tsx`.

- [ ] **Step 3: Commit**

```bash
git add apps/web/src/components/vocabulary-popover.tsx
git commit -m "feat: add VocabularyPopover hover card component"
```

---

### Task 4: Update MarkdownRenderer to consume SmartLinkProvider context

Wire the matching logic into the markdown component overrides.

**Files:**
- Modify: `apps/web/src/components/markdown-renderer.tsx`

- [ ] **Step 1: Add imports and smart code component**

In `apps/web/src/components/markdown-renderer.tsx`, add imports at the top (after existing imports):

```tsx
import { Link } from "@tanstack/react-router";
import { matchInCode, matchVocabularyInText, useSmartLinks } from "./smart-link-provider";
import { VocabularyPopover } from "./vocabulary-popover";
```

- [ ] **Step 2: Create SmartCode component**

Add this component before the `/* ─── Component overrides ─── */` comment:

```tsx
/* ─── Smart inline code ─── */

function SmartCode({ children, className, ...props }: {
    children: string;
    className?: string;
    [key: string]: unknown;
}) {
    const { chunkIndex, fileRefIndex, vocabIndex } = useSmartLinks();
    const excludeChunkId = useContext(ExcludeChunkContext);
    const text = String(children);
    const isInline = !className && !text.includes("\n");

    if (!isInline) {
        return <CodeBlock className={className}>{text}</CodeBlock>;
    }

    const match = matchInCode(text, chunkIndex, fileRefIndex, vocabIndex, excludeChunkId);

    if (match?.type === "chunk") {
        return (
            <Link
                to="/chunks/$chunkId"
                params={{ chunkId: match.id }}
                className="rounded bg-primary/10 px-1.5 py-0.5 text-sm font-mono text-primary hover:bg-primary/20 transition-colors"
            >
                {children}
            </Link>
        );
    }

    if (match?.type === "fileRef") {
        return (
            <Link
                to="/chunks/$chunkId"
                params={{ chunkId: match.chunkId }}
                className="rounded bg-primary/10 px-1.5 py-0.5 text-sm font-mono text-primary hover:bg-primary/20 transition-colors"
                title={match.path}
            >
                {children}
            </Link>
        );
    }

    if (match?.type === "vocabulary") {
        return (
            <VocabularyPopover word={match.word} category={match.category} expects={match.expects}>
                <code className="rounded bg-muted px-1.5 py-0.5 text-sm font-mono" {...props}>
                    {children}
                </code>
            </VocabularyPopover>
        );
    }

    return (
        <code className="rounded bg-muted px-1.5 py-0.5 text-sm font-mono" {...props}>
            {children}
        </code>
    );
}
```

- [ ] **Step 3: Create SmartText component for vocabulary matching in prose**

Add this component after `SmartCode`:

```tsx
/* ─── Smart text (vocabulary matching in prose) ─── */

function SmartText({ children }: { children: string }) {
    const { vocabIndex } = useSmartLinks();
    const matches = matchVocabularyInText(children, vocabIndex);

    if (matches.length === 0) return <>{children}</>;

    const parts: React.ReactNode[] = [];
    let lastEnd = 0;

    for (const match of matches) {
        if (match.start > lastEnd) {
            parts.push(children.slice(lastEnd, match.start));
        }
        parts.push(
            <VocabularyPopover
                key={match.start}
                word={match.word}
                category={match.category}
                expects={match.expects}
            >
                {children.slice(match.start, match.end)}
            </VocabularyPopover>
        );
        lastEnd = match.end;
    }

    if (lastEnd < children.length) {
        parts.push(children.slice(lastEnd));
    }

    return <>{parts}</>;
}
```

- [ ] **Step 4: Create wrapper components for text-containing elements**

Add these components to wrap elements whose text children should be scanned for vocabulary matches. These skip matching inside headings, links, and code (which are already interactive):

```tsx
/* ─── Smart paragraph (wraps text children with vocab matching) ─── */

function SmartParagraph({ children }: { children: React.ReactNode }) {
    return <p>{processChildren(children)}</p>;
}

function SmartListItem({ children }: { children: React.ReactNode }) {
    return <li>{processChildren(children)}</li>;
}

function processChildren(children: React.ReactNode): React.ReactNode {
    if (typeof children === "string") {
        return <SmartText>{children}</SmartText>;
    }
    if (Array.isArray(children)) {
        return children.map((child, i) => {
            if (typeof child === "string") {
                return <SmartText key={i}>{child}</SmartText>;
            }
            return child;
        });
    }
    return children;
}
```

- [ ] **Step 5: Update the components object**

Replace the existing `code` override and add `p` and `li` overrides. Update the `components` object:

Replace the existing `code` entry:

```ts
    code({ className, children, ...props }) {
        const isInline = !className && typeof children === "string" && !children.includes("\n");
        if (isInline) {
            return (
                <code className="rounded bg-muted px-1.5 py-0.5 text-sm font-mono" {...props}>
                    {children}
                </code>
            );
        }
        return <CodeBlock className={className}>{String(children)}</CodeBlock>;
    },
```

With:

```ts
    code({ className, children, ...props }) {
        const text = String(children);
        const isInline = !className && !text.includes("\n");
        if (isInline) {
            return <SmartCode className={className} {...props}>{text}</SmartCode>;
        }
        return <CodeBlock className={className}>{text}</CodeBlock>;
    },
```

And add these two entries to the components object (after the existing `hr` entry is fine):

```ts
    p({ children }) {
        return <SmartParagraph>{children}</SmartParagraph>;
    },
    li({ children }) {
        return <SmartListItem>{children}</SmartListItem>;
    },
```

- [ ] **Step 6: Update MarkdownRenderer to accept excludeChunkId**

Change the `MarkdownRenderer` export to accept and thread through the `excludeChunkId` prop. Replace:

```tsx
export function MarkdownRenderer({ children }: { children: string }) {
```

With:

```tsx
export function MarkdownRenderer({ children, excludeChunkId }: { children: string; excludeChunkId?: string }) {
```

The `excludeChunkId` needs to reach `SmartCode`. Since react-markdown's `components` object is static, use a React context for this. Add before the `components` object:

```tsx
const ExcludeChunkContext = createContext<string | undefined>(undefined);
```

Update `SmartCode` to read from it:

```tsx
function SmartCode({ children, className, ...props }: {
    children: string;
    className?: string;
    [key: string]: unknown;
}) {
    const { chunkIndex, fileRefIndex, vocabIndex } = useSmartLinks();
    const excludeChunkId = useContext(ExcludeChunkContext);
    const text = String(children);
    const isInline = !className && !text.includes("\n");
    // ... rest unchanged
```

And wrap the Markdown component with it:

```tsx
export function MarkdownRenderer({ children, excludeChunkId }: { children: string; excludeChunkId?: string }) {
    return (
        <ExcludeChunkContext.Provider value={excludeChunkId}>
            <Markdown
                remarkPlugins={remarkPlugins}
                rehypePlugins={rehypePlugins}
                components={components}
            >
                {children}
            </Markdown>
        </ExcludeChunkContext.Provider>
    );
}
```

Add `createContext, useContext` to the React import at the top of the file.

- [ ] **Step 7: Verify no TypeScript errors**

Run: `cd apps/web && pnpm tsc --noEmit --pretty 2>&1 | head -20`

Expected: No errors.

- [ ] **Step 8: Commit**

```bash
git add apps/web/src/components/markdown-renderer.tsx
git commit -m "feat: wire SmartLinkProvider into MarkdownRenderer component overrides"
```

---

### Task 5: Mount SmartLinkProvider in root layout

**Files:**
- Modify: `apps/web/src/routes/__root.tsx`

- [ ] **Step 1: Add import**

In `apps/web/src/routes/__root.tsx`, add this import alongside the existing component imports:

```ts
import { SmartLinkProvider } from "@/components/smart-link-provider";
```

- [ ] **Step 2: Wrap with SmartLinkProvider**

In the `RootDocument` function, wrap the content inside `<ThemeProvider>`. Find:

```tsx
                <ThemeProvider>
                    <VocabularyPrimer />
```

Replace with:

```tsx
                <ThemeProvider>
                    <SmartLinkProvider>
                    <VocabularyPrimer />
```

And find the closing `</ThemeProvider>`:

```tsx
                </ThemeProvider>
```

Add `</SmartLinkProvider>` before it:

```tsx
                    </SmartLinkProvider>
                </ThemeProvider>
```

- [ ] **Step 3: Verify the app still loads**

Run: Open `http://localhost:3001` in the browser, navigate to a chunk detail page. Verify the page renders without errors. Check the browser console for any React errors.

- [ ] **Step 4: Commit**

```bash
git add apps/web/src/routes/__root.tsx
git commit -m "feat: mount SmartLinkProvider in root layout"
```

---

### Task 6: Migrate ChunkLinkRenderer callsite and delete wrapper

**Files:**
- Modify: `apps/web/src/features/chunks/detail/chunk-detail-content.tsx`
- Delete: `apps/web/src/features/chunks/chunk-link-renderer.tsx`

- [ ] **Step 1: Update chunk detail content**

In `apps/web/src/features/chunks/detail/chunk-detail-content.tsx`:

Remove the `ChunkLinkRenderer` import (line 5):

```ts
import { ChunkLinkRenderer } from "@/features/chunks/chunk-link-renderer";
```

The `MarkdownRenderer` import on line 4 is already there. Find the usage on line 119:

```tsx
                <ChunkLinkRenderer content={content} currentChunkId={chunkId} />
```

Replace with:

```tsx
                <MarkdownRenderer excludeChunkId={chunkId}>{content}</MarkdownRenderer>
```

- [ ] **Step 2: Delete ChunkLinkRenderer**

Delete the file `apps/web/src/features/chunks/chunk-link-renderer.tsx`.

Run: `rm apps/web/src/features/chunks/chunk-link-renderer.tsx`

- [ ] **Step 3: Verify no remaining imports**

Run: `grep -r "chunk-link-renderer" apps/web/src/`

Expected: No results.

- [ ] **Step 4: Verify the chunk detail page works**

Open a chunk detail page in the browser. Verify:
- Markdown content renders correctly
- Backticked terms that match other chunk titles show as clickable links
- No console errors

- [ ] **Step 5: Commit**

```bash
git add apps/web/src/features/chunks/detail/chunk-detail-content.tsx
git rm apps/web/src/features/chunks/chunk-link-renderer.tsx
git commit -m "refactor: replace ChunkLinkRenderer with SmartLinkProvider-powered MarkdownRenderer"
```

---

### Task 7: Clean up document-browser inter-doc link workarounds

Remove the click interception and `allDocsQuery` from the document browser. Smart linking now handles this via the provider.

**Files:**
- Modify: `apps/web/src/features/documents/document-browser.tsx`

- [ ] **Step 1: Remove allDocsQuery, docTitleMap, and handleContentClick**

In `apps/web/src/features/documents/document-browser.tsx`, remove these blocks:

1. Remove the `allDocsQuery` (lines ~387–406):

```ts
    // Fetch all document details for search and inter-document linking
    const allDocsQuery = useQuery({
        queryKey: ["documents-all-details"],
        ...
    });
```

**Wait** — `allDocsQuery` is also used for search (`searchResults` and `groupedSearchResults` depend on `allDocsQuery.data`). Only remove the parts related to inter-document linking, not the query itself.

Remove:

a) The `docTitleMap` memo (lines ~409–419):
```ts
    // Build title-to-ID map for inter-document link navigation
    const docTitleMap = useMemo(() => {
        ...
    }, [allDocsQuery.data]);
```

b) The `handleContentClick` function (lines ~421–437):
```ts
    const handleContentClick = (e: React.MouseEvent) => {
        ...
    };
```

c) The `onClick={handleContentClick}` on the content div (line ~951). Find:
```tsx
                        <div className="space-y-2" data-doc-content onClick={handleContentClick}>
```

Replace with:
```tsx
                        <div className="space-y-2" data-doc-content>
```

- [ ] **Step 2: Verify docs page still works**

Open `http://localhost:3001/docs`, select a document. Verify:
- Document content renders
- Search still works
- No console errors

- [ ] **Step 3: Commit**

```bash
git add apps/web/src/features/documents/document-browser.tsx
git commit -m "refactor: remove document-browser inter-doc link workarounds (handled by SmartLinkProvider)"
```

---

### Task 8: Manual integration verification

Verify the full feature works end-to-end.

- [ ] **Step 1: Test backtick chunk linking**

Create or find a chunk whose content includes a backticked reference to another chunk's title. For example, if you have a chunk titled "Authentication Flow", create content with `` `Authentication Flow` ``.

Verify: The backticked text renders as a clickable link that navigates to the referenced chunk.

- [ ] **Step 2: Test vocabulary popover**

Ensure you have vocabulary entries in the database (the seed data includes 10). Navigate to a chunk or document whose content mentions a vocabulary term.

Verify: The term has a dotted underline, hovering shows a popover with the word, category badge, expects list, and "View in vocabulary →" link.

- [ ] **Step 3: Test self-link exclusion**

On a chunk detail page, verify that the chunk's own title (if backticked in its own content) does NOT render as a link.

- [ ] **Step 4: Test docs page**

Navigate to `/docs`, select a document. Verify:
- Content renders with smart links
- Search still works
- Previous/next navigation works
- No console errors

- [ ] **Step 5: Test other markdown surfaces**

Check that smart linking works on:
- Graph detail panel (click a node, see the detail)
- Plan descriptions
- Any other page that uses `<MarkdownRenderer>`

- [ ] **Step 6: Run the full test suite**

Run: `pnpm test`

Expected: All tests pass including the new `smart-link-provider.test.ts`.

- [ ] **Step 7: Run type check**

Run: `pnpm run check-types`

Expected: No type errors.

- [ ] **Step 8: Final commit (if any fixups needed)**

```bash
git add -A
git commit -m "fix: integration fixups for smart link renderer"
```
