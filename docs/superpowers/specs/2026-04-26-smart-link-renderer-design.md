# Smart Link Renderer

**Date:** 2026-04-26
**Status:** Draft

## Problem

Link intelligence is scattered across the app:

- `ChunkLinkRenderer` does fragile regex replacement on raw markdown strings before parsing — can break inside code blocks, existing links, and other markdown syntax
- `document-browser` does post-render click interception via `handleContentClick` — only works on the docs page
- Most of the app gets no smart linking at all
- Vocabulary terms aren't surfaced anywhere in content

## Goal

Extend `MarkdownRenderer` with a React context provider so that every markdown surface in the app automatically resolves backticked terms to chunks/file-refs and highlights vocabulary terms with hover popovers — without any callsite changes.

## Architecture

```
<SmartLinkProvider>          ← near app root, fetches + caches entities
  └─ <MarkdownRenderer>      ← reads from context, no extra props needed
       ├─ components.code     ← backticked terms: match chunks → file refs
       ├─ components.a        ← existing links pass through, enhanced
       └─ Text node wrapper   ← plain prose: match vocabulary terms
```

## Data Layer — SmartLinkProvider

A context provider placed in the root layout (`__root.tsx`). Uses three queries with long stale times (lookup indices, not live data):

- **Vocabulary**: `GET /api/vocabulary` → `Map<lowercase_word, { word, category, expects }>`
- **Chunks**: `GET /api/chunks` (title index) → `Map<lowercase_title, { id, title }>` + aliases flattened in
- **File refs**: `GET /api/file-refs/lookup` (existing endpoint, may need a batch/index variant) → `Map<lowercase_symbol, { chunkId, path }>`

All three maps are memoized. The context exposes:

```ts
interface SmartLinkContextValue {
  matchVocabulary(text: string): VocabularyMatch | null;
  matchChunk(text: string): ChunkMatch | null;
  matchFileRef(text: string): FileRefMatch | null;
  // Convenience: runs all three with priority vocab > chunk > fileRef
  matchAny(text: string): SmartMatch | null;
}

interface VocabularyMatch {
  word: string;
  category: string;
  expects: string[] | null;
}

interface ChunkMatch {
  id: string;
  title: string;
}

interface FileRefMatch {
  chunkId: string;
  path: string;
  symbol?: string;
}

type SmartMatch =
  | { type: "vocabulary"; match: VocabularyMatch }
  | { type: "chunk"; match: ChunkMatch }
  | { type: "fileRef"; match: FileRefMatch };
```

Queries only fire when the user is authenticated (no data to match on landing/login pages). The provider renders children immediately — if queries are still loading, matching functions return `null` (graceful degradation, no layout shift).

## Matching Rules

| Content type | What matches | Interaction |
|---|---|---|
| `` `backticked term` `` | Chunks (by title/alias), then file refs (by symbol/path) | Styled as clickable code link → navigates to `/chunks/:id` |
| Plain prose text | Vocabulary terms (by word, case-insensitive, word-boundary) | Subtle dotted underline → hover shows popover card |
| Existing `[links](url)` | Pass through unchanged | Normal link behavior |

**Priority depends on context:**

- **In backticks:** chunk > file ref > vocabulary. Backticking is an explicit code-reference signal, so a chunk link is the most useful result. Vocabulary popover is the fallback if no chunk or file ref matches.
- **In prose:** only vocabulary matches (chunks and file refs don't match plain text).

This means there's no ambiguity — the matching context determines priority, not a global ranking.

## Component Overrides

### `components.code` (inline only)

When the code element is inline (no `className`, no newlines):

1. Check if text matches a chunk title/alias → render as `<Link to="/chunks/:id">` styled as code with a subtle link indicator
2. Else check file ref → render as `<Link to="/chunks/:chunkId">` styled as code
3. Else check vocabulary → wrap in `<VocabularyPopover>`
4. Else render as plain `<code>` (current behavior)

Block code (with language class) is unaffected — still rendered by `CodeBlock`/shiki.

### Text node wrapper

Plain text nodes are scanned for vocabulary term matches:

- Build a pre-compiled regex from all vocabulary words (sorted longest-first, word-boundary-delimited)
- Split text nodes on matches, wrap matched substrings in `<VocabularyPopover>`
- **Skip matching inside:** headings (h1-h6), existing links (`<a>`), code blocks — these are already interactive or semantic elements where injecting popovers would be disruptive

Implementation: a custom rehype plugin that walks text nodes after markdown parsing, or a React wrapper component around text content. The rehype plugin approach is cleaner since it operates on the AST before React rendering.

### `components.a`

Existing links pass through with current behavior (external links get `target="_blank"`, internal links use normal navigation). No changes needed here — the smart linking happens at the `code` and text levels.

## Vocabulary Popover

A custom hover card component (not a base-ui tooltip — needs more room for structured content):

```
┌─────────────────────────────┐
│ UserService          model  │  ← word + category badge
│                             │
│ Expects: class, interface   │  ← expects array, if present
│                             │
│ View in vocabulary →        │  ← link to /vocabulary
└─────────────────────────────┘
```

**Behavior:**

- Triggered on hover with ~200ms open delay (prevents flickering on mouse pass-through)
- Positioned above or below the term, flips on viewport overflow
- Light border, subtle shadow, consistent with existing card/popover styles
- Dismiss on mouse leave with ~100ms grace area (so user can move to the popover itself)
- On touch devices: tap navigates directly to `/vocabulary` (no hover state)
- Popover uses `React.createPortal` to avoid z-index/overflow issues

**Styling of vocabulary terms in prose:**

- Subtle dotted underline (`decoration-dotted underline-offset-2`)
- Slightly different text color from surrounding text (`text-foreground/80`)
- Cursor changes to indicate interactivity

## Migration

| Current | After |
|---|---|
| `ChunkLinkRenderer` wraps `MarkdownRenderer` with regex pre-processing | **Deleted** — matching lives inside `MarkdownRenderer` via context |
| `document-browser` uses `handleContentClick` + `docTitleMap` + `allDocsQuery` for inter-doc links | **Removed** — inter-doc title matching moves into the provider or is handled naturally by chunk title matching |
| Callsites use `<ChunkLinkRenderer content={...} currentChunkId={...}>` | Replaced with `<MarkdownRenderer>{content}</MarkdownRenderer>` |

### Callsites to update

- `features/chunks/detail/chunk-detail-content.tsx` — only current `ChunkLinkRenderer` import site
- `features/documents/document-browser.tsx` — remove `handleContentClick`, `docTitleMap`, `allDocsQuery`

### `currentChunkId` handling

`ChunkLinkRenderer` currently accepts `currentChunkId` to avoid self-linking. The new approach handles this differently: the `MarkdownRenderer` doesn't know which chunk it's rendering. Two options:

1. An optional `excludeChunkId` prop on `MarkdownRenderer` (simple, explicit)
2. A `SmartLinkExclude` context wrapper that callsites can use

Option 1 is simpler and sufficient — only chunk detail pages need this. One optional prop is acceptable.

## Performance

- Three cached queries with 5-minute stale time — no per-render fetching
- Vocabulary matching: pre-built regex (sorted longest-first, word-boundary-delimited), one pass per text node
- Chunk/file-ref matching: `Map.get()` on backtick content — O(1) per inline code element
- Text node scanning skips nodes inside headings/links/code (checked via parent in AST)
- Provider renders children immediately while queries load — no blocking, no layout shift

## Files

| File | Action |
|---|---|
| `apps/web/src/components/smart-link-provider.tsx` | **Create** — context, provider, queries, matching logic |
| `apps/web/src/components/vocabulary-popover.tsx` | **Create** — hover card component |
| `apps/web/src/components/markdown-renderer.tsx` | **Modify** — consume context, update `code` override, add text node processing via rehype plugin |
| `apps/web/src/features/chunks/chunk-link-renderer.tsx` | **Delete** |
| `apps/web/src/features/documents/document-browser.tsx` | **Modify** — remove `handleContentClick`, `docTitleMap`, `allDocsQuery` |
| `apps/web/src/routes/__root.tsx` | **Modify** — add `<SmartLinkProvider>` |
| All `ChunkLinkRenderer` import sites | **Modify** — replace with `<MarkdownRenderer>` |

## Out of Scope

- Editing/creating vocabulary terms from the popover (future enhancement)
- Fuzzy matching (exact word-boundary matches only for now)
- Matching inside markdown editor previews (only in read-only rendered content)
- Server-side matching / pre-processing (all client-side via React context)
