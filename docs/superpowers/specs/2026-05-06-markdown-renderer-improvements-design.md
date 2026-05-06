# Markdown Renderer Improvements

**Date:** 2026-05-06
**Scope:** Symmetric markdown codec (parse/render round-trip), flexible heading-level splitting, frontend renderer UX and performance

## Overview

Three areas of improvement to the markdown pipeline:

1. **Symmetric codec** — `splitMarkdown` and `renderDocument` become two halves of a round-trip codec. Export a document's chunks to markdown, edit externally, re-import without metadata loss.
2. **Flexible heading-level splitting** — `splitMarkdown` gains configurable and auto-detected split levels instead of hardcoded H2.
3. **Frontend `MarkdownRenderer` improvements** — promise-based mermaid loading, copy buttons on code blocks, auto-generated TOC, vocabulary matching performance.

## Approach

**Symmetric codec (Approach A):** Treat `splitMarkdown` and `renderDocument` as inverse functions sharing heading-level detection logic. A round-trip (`split → render → split`) produces identical structured output. No new abstraction layer — just make the existing functions aware of each other's format.

## Design

### 1. Symmetric Markdown Codec

#### Heading-level detection

`splitMarkdown` gains a `splitLevel` parameter:

```typescript
function splitMarkdown(
    raw: string,
    filePath: string,
    splitLevel?: 2 | 3 | 4 | "auto"
): SplitResult
```

Default: `"auto"`.

Auto-detection logic: after stripping frontmatter and the title heading, scan for the first heading encountered. That heading's level becomes the split level. If no headings are found, the entire body becomes a single "Introduction" section (preserving current behavior).

The detected split level is returned in the `SplitResult` and persisted on the document record (new `splitLevel` column, nullable integer). `renderDocument` reads this to emit the correct heading prefix (e.g., `###` for level 3 instead of the current hardcoded `##`):

```typescript
interface SplitResult {
    title: string;
    description?: string;
    tags: string[];
    sections: MarkdownSection[];
    splitLevel: number; // new — the level that was used for splitting
}
```

#### Frontmatter round-trip in `renderDocument`

`renderDocument` reconstructs a full frontmatter block from chunk + document metadata:

```yaml
---
title: Getting Started
type: reference
tags:
  - guides
  - getting started
scope:
  env: production
---
```

Fields emitted:
- `title` — always
- `type` — only if not `"document"` (the default)
- `tags` — from chunk tags, excluding path-derived tags (folder segments and filename stem) to avoid duplication on re-import
- `scope` — only if present on the chunk

To retrieve tags, `renderDocument` calls `getTagsForChunk` for the first chunk (tags are shared across a document's chunks). Tags that match folder segments from `document.sourcePath` are excluded since `tagsFromPath` will re-derive them on import. Scope is read from `chunk.scope` (JSONB column already on the chunk table).

#### Decision context fields

`renderDocument` emits `rationale`, `alternatives`, and `consequences` as labeled blockquote sections after the chunk content:

```markdown
## Authentication

Content here...

> **Rationale:** We chose JWT because...

> **Alternatives:**
> - Session cookies
> - OAuth tokens

> **Consequences:** Requires token refresh logic...
```

`splitMarkdown` learns to parse these back out. At the end of each section's content, it scans the final consecutive blockquote group for lines matching `> **Rationale:**`, `> **Alternatives:**`, and `> **Consequences:**`. Only trailing blockquotes are examined — blockquotes earlier in the section are treated as regular content. Matched blockquotes are extracted into structured fields on the section and removed from `content`.

The `MarkdownSection` type gains optional decision context fields:

```typescript
interface MarkdownSection {
    title: string;
    content: string;
    order: number;
    rationale?: string;
    alternatives?: string[];
    consequences?: string;
}
```

#### Round-trip contract

The invariant: `split(render(doc))` produces sections with identical `title`, `content`, `tags`, `type`, `scope`, `rationale`, `alternatives`, and `consequences`. Tests verify this property using snapshot-style assertions on known documents.

#### Files

- `packages/api/src/documents/split-markdown.ts` — `splitLevel` param, auto-detect logic, decision-context extraction, `splitLevel` in return type
- `packages/api/src/documents/service.ts` — `renderDocument` emits frontmatter + decision context blockquotes, fetches tags/scope for chunks; `importDocument` persists detected `splitLevel` on document record
- `packages/db/src/schema/document.ts` — add nullable `splitLevel` integer column to `document` table
- `packages/api/src/chunks/parse-docs.ts` — minor consistency fixes for frontmatter format if needed

### 2. Frontend: Mermaid loading fix

Replace the `setTimeout` polling loop with a promise-based approach.

Current code stores a module-level `mermaidReady` variable and polls it every 100ms in a `useEffect`. The fix: store the `import("mermaid")` promise itself and `await` it directly.

```typescript
const mermaidPromise = typeof window !== "undefined"
    ? import("mermaid").then(m => {
        m.default.initialize({ startOnLoad: false, theme: "dark" });
        return m.default;
    })
    : null;
```

`MermaidBlock`'s `useEffect`:

```typescript
useEffect(() => {
    let cancelled = false;
    if (!mermaidPromise) return;
    mermaidPromise
        .then(mermaid => mermaid.render(`mermaid-${id}`, children.trim()))
        .then(({ svg }) => { if (!cancelled) setSvg(svg); })
        .catch(err => { if (!cancelled) setError(String(err)); });
    return () => { cancelled = true; };
}, [children, id]);
```

No retry loop, no race conditions. The promise resolves once and is shared across all `MermaidBlock` instances.

#### File

- `apps/web/src/components/markdown-renderer.tsx` — lines 9-73

### 3. Frontend: Copy button on code blocks

A `CopyButton` component rendered inside `CodeBlock`:

- Inline SVG clipboard icon (no new dependency)
- On click: `navigator.clipboard.writeText(code)`, swaps to checkmark icon for 2 seconds
- Positioned `absolute top-2 right-2` inside the code block wrapper
- Visible on hover only: `opacity-0 group-hover:opacity-100 transition-opacity`

`CodeBlock` wraps its output in a `relative group` div and renders `<CopyButton code={code} />` alongside the highlighted content. Applies to both the shiki-highlighted path and the plain fallback.

#### File

- `apps/web/src/components/markdown-renderer.tsx` — new `CopyButton` component, modify `CodeBlock`

### 4. Frontend: Auto-generated table of contents

When the markdown content has 3+ headings, render a TOC above the content as part of `MarkdownRenderer`.

#### Heading extraction

A `useMemo` hook in `MarkdownRenderer` parses the raw markdown string for headings using `/^(#{1,6})\s+(.+)$/gm`. Each heading gets a generated slug (lowercase, spaces to hyphens, strip non-alphanumeric). If fewer than 3 headings are found, no TOC is rendered.

#### Anchor targets

Add component overrides for `h1` through `h6` in the `components` object, each generating the same slug as the TOC extractor and setting it as the element's `id`.

#### TOC rendering

A `nav` element above the markdown content:

- Bordered box styled with `text-sm text-muted-foreground`
- Indentation via `ml-{n}` classes based on heading depth relative to the minimum heading level found
- Links are plain anchor tags (`#slug`)

#### File

- `apps/web/src/components/markdown-renderer.tsx` — slug generation utility, heading component overrides with `id`, TOC component, conditional rendering

### 5. Frontend: Vocabulary matching performance

Two changes to avoid redundant computation:

#### Precompile vocabulary regex

In `useSmartLinks` (or the smart-link provider), build a single compiled `RegExp` (word-boundary-wrapped alternation of all vocab terms) once when `vocabIndex` changes. Expose this precompiled regex from the provider context alongside `vocabIndex`. This avoids recompiling per text node.

#### Memoize components

- Wrap `SmartText` in `React.memo` and memoize the match result with `useMemo` keyed on `(children, vocabIndex)`. Since `vocabIndex` is stable across renders (comes from a query), match computation only runs when text or vocabulary actually changes.
- Wrap `SmartParagraph` and `SmartListItem` in `React.memo` so children aren't reprocessed when parent re-renders don't change props.

#### Files

- `apps/web/src/components/smart-link-provider.tsx` (or equivalent) — precompile regex, expose from provider
- `apps/web/src/components/markdown-renderer.tsx` — `React.memo` on `SmartText`, `SmartParagraph`, `SmartListItem`; `useMemo` for match results

## Testing

- **Round-trip property tests:** Create test documents with frontmatter, decision context, various heading levels, and verify `split(render(split(input))) === split(input)`.
- **Split-level tests:** Verify auto-detection picks correct level for H2-only, H3-only, and mixed documents. Verify explicit `splitLevel` overrides auto-detection.
- **TOC tests:** Verify slug generation, threshold behavior (no TOC under 3 headings), correct nesting depth.
- **Copy button:** Manual verification in browser (click → clipboard content matches code block).
- **Mermaid:** Manual verification that diagrams render without polling artifacts.
- **Vocab perf:** No functional change — verify existing behavior preserved via existing tests.
