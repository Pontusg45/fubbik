# Markdown Renderer Improvements Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make `splitMarkdown` and `renderDocument` a symmetric codec that round-trips frontmatter and decision context, add configurable heading-level splitting, and improve the frontend MarkdownRenderer with promise-based mermaid loading, code block copy buttons, auto-generated TOC, and vocabulary matching performance.

**Architecture:** Backend changes center on `split-markdown.ts` (parsing) and `service.ts:renderDocument` (serialization) sharing heading-level detection and decision-context format. A new `splitLevel` column on the `document` table persists the detected level. Frontend changes are isolated to `markdown-renderer.tsx` (mermaid, copy, TOC, heading IDs) and `smart-link-provider.tsx` (precompiled regex). All changes are additive — no existing behavior changes unless explicitly noted.

**Tech Stack:** Drizzle (schema migration), Effect (service layer), react-markdown + remark-gfm + rehype-raw (frontend rendering), vitest (tests)

---

### Task 1: Add `splitLevel` column to document schema

**Files:**
- Modify: `packages/db/src/schema/document.ts:8-31`

- [ ] **Step 1: Add `splitLevel` column to document table**

In `packages/db/src/schema/document.ts`, add an `integer` import and the column:

```typescript
import { index, integer, pgTable, text, timestamp, uniqueIndex } from "drizzle-orm/pg-core";
```

Add after the `description` column:

```typescript
        splitLevel: integer("split_level"),
```

- [ ] **Step 2: Push schema change to database**

Run: `pnpm db:push`
Expected: Schema updated successfully with new nullable `split_level` column on `document` table.

- [ ] **Step 3: Verify type-check passes**

Run: `pnpm run check-types`
Expected: All packages pass.

- [ ] **Step 4: Commit**

```bash
git add packages/db/src/schema/document.ts
git commit -m "feat: add splitLevel column to document table"
```

---

### Task 2: Flexible heading-level detection in `splitMarkdown`

**Files:**
- Modify: `packages/api/src/documents/split-markdown.ts`
- Modify: `packages/api/src/documents/split-markdown.test.ts`

- [ ] **Step 1: Write failing tests for heading-level detection**

Add these tests to `packages/api/src/documents/split-markdown.test.ts`:

```typescript
it("auto-detects H3 as split level when no H2s exist", () => {
    const md = `# Title\n\n### First\n\nContent one.\n\n### Second\n\nContent two.\n`;
    const result = splitMarkdown(md, "test.md");
    expect(result.splitLevel).toBe(3);
    expect(result.sections).toHaveLength(2);
    expect(result.sections[0]!.title).toBe("First");
    expect(result.sections[1]!.title).toBe("Second");
});

it("uses explicit splitLevel override", () => {
    const md = `# Title\n\n## H2 Section\n\nContent.\n\n### H3 Section\n\nMore.\n`;
    const result = splitMarkdown(md, "test.md", 3);
    expect(result.splitLevel).toBe(3);
    expect(result.sections).toHaveLength(1);
    expect(result.sections[0]!.title).toBe("H3 Section");
});

it("returns splitLevel 2 for existing H2 documents", () => {
    const md = `# My Document\n\nIntro.\n\n## First Section\n\nFirst content.\n`;
    const result = splitMarkdown(md, "test.md");
    expect(result.splitLevel).toBe(2);
});

it("defaults splitLevel to 2 when no headings found", () => {
    const md = `# Title\n\nJust content with no sub-headings.\n`;
    const result = splitMarkdown(md, "test.md");
    expect(result.splitLevel).toBe(2);
    expect(result.sections).toHaveLength(1);
    expect(result.sections[0]!.title).toBe("Title — Introduction");
});
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `pnpm --filter @fubbik/api test -- --reporter verbose src/documents/split-markdown.test.ts`
Expected: New tests fail (no `splitLevel` property on result, wrong section counts for H3 tests).

- [ ] **Step 3: Implement flexible heading-level detection**

Replace the contents of `packages/api/src/documents/split-markdown.ts`:

```typescript
import { extractFrontmatter, tagsFromPath } from "../chunks/parse-docs";

export interface MarkdownSection {
    title: string;
    content: string;
    order: number;
    rationale?: string;
    alternatives?: string[];
    consequences?: string;
}

export interface SplitResult {
    title: string;
    description?: string;
    tags: string[];
    sections: MarkdownSection[];
    splitLevel: number;
}

function detectSplitLevel(content: string): number {
    const headingMatch = content.match(/^(#{2,6})\s+/m);
    if (!headingMatch) return 2;
    return headingMatch[1]!.length;
}

export function splitMarkdown(
    raw: string,
    filePath: string,
    splitLevel?: 2 | 3 | 4 | "auto"
): SplitResult {
    const { frontmatter, body } = extractFrontmatter(raw);

    let title = frontmatter.title as string | undefined;
    let content = body;

    const h1Match = content.match(/^#\s+(.+)$/m);
    if (h1Match) {
        if (!title) title = h1Match[1]!.trim();
        content = content.replace(/^#\s+.+\n?/m, "").trim();
    }

    if (!title) {
        const filename = filePath.split("/").pop() ?? filePath;
        title = filename.replace(/\.md$/i, "").replace(/[-_]/g, " ");
    }

    const fmTags = Array.isArray(frontmatter.tags) ? frontmatter.tags.map(String) : [];
    const pathTags = tagsFromPath(filePath);
    const tags = [...new Set([...fmTags, ...pathTags])];
    const description = (frontmatter.description as string) ?? undefined;

    const resolvedLevel = splitLevel === "auto" || splitLevel === undefined
        ? detectSplitLevel(content)
        : splitLevel;

    const prefix = "#".repeat(resolvedLevel);
    const headingRegex = new RegExp(`^${prefix} (.+)$`, "gm");
    const matches: { title: string; index: number }[] = [];
    let match: RegExpExecArray | null;
    while ((match = headingRegex.exec(content)) !== null) {
        matches.push({ title: match[1]!.trim(), index: match.index });
    }

    const sections: MarkdownSection[] = [];
    let orderCounter = 0;

    if (matches.length === 0) {
        const trimmed = content.trim();
        if (trimmed) {
            sections.push({ title: `${title} — Introduction`, content: trimmed, order: orderCounter });
        }
        return { title, description, tags, sections, splitLevel: resolvedLevel };
    }

    const preamble = content.slice(0, matches[0]!.index).trim();
    if (preamble) {
        sections.push({ title: `${title} — Introduction`, content: preamble, order: orderCounter++ });
    }

    for (let i = 0; i < matches.length; i++) {
        const heading = matches[i]!;
        const nextIndex = i + 1 < matches.length ? matches[i + 1]!.index : content.length;
        const sectionContent = content
            .slice(heading.index + `${prefix} ${heading.title}`.length + 1, nextIndex)
            .trim();
        sections.push({ title: heading.title, content: sectionContent, order: orderCounter++ });
    }

    return { title, description, tags, sections, splitLevel: resolvedLevel };
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `pnpm --filter @fubbik/api test -- --reporter verbose src/documents/split-markdown.test.ts`
Expected: All tests pass (old and new).

- [ ] **Step 5: Run full test suite to check for regressions**

Run: `pnpm test`
Expected: All tests pass. The `importDocument` service and other consumers of `splitMarkdown` are unaffected since `splitLevel` is a new additive field.

- [ ] **Step 6: Commit**

```bash
git add packages/api/src/documents/split-markdown.ts packages/api/src/documents/split-markdown.test.ts
git commit -m "feat: flexible heading-level detection in splitMarkdown"
```

---

### Task 3: Decision context extraction in `splitMarkdown`

**Files:**
- Modify: `packages/api/src/documents/split-markdown.ts`
- Modify: `packages/api/src/documents/split-markdown.test.ts`

- [ ] **Step 1: Write failing tests for decision context extraction**

Add to `packages/api/src/documents/split-markdown.test.ts`:

```typescript
it("extracts decision context from trailing blockquotes", () => {
    const md = [
        "# Doc",
        "",
        "## Auth",
        "",
        "We use JWT for authentication.",
        "",
        "> **Rationale:** Stateless, no server-side sessions needed.",
        "",
        "> **Alternatives:**",
        "> - Session cookies",
        "> - OAuth tokens",
        "",
        "> **Consequences:** Requires token refresh logic.",
    ].join("\n");
    const result = splitMarkdown(md, "test.md");
    expect(result.sections).toHaveLength(1);
    expect(result.sections[0]!.content).toBe("We use JWT for authentication.");
    expect(result.sections[0]!.rationale).toBe("Stateless, no server-side sessions needed.");
    expect(result.sections[0]!.alternatives).toEqual(["Session cookies", "OAuth tokens"]);
    expect(result.sections[0]!.consequences).toBe("Requires token refresh logic.");
});

it("does not extract blockquotes that are not decision context", () => {
    const md = [
        "# Doc",
        "",
        "## Notes",
        "",
        "> This is a regular blockquote in the middle.",
        "",
        "More content after the blockquote.",
    ].join("\n");
    const result = splitMarkdown(md, "test.md");
    expect(result.sections[0]!.content).toContain("> This is a regular blockquote");
    expect(result.sections[0]!.content).toContain("More content after the blockquote.");
    expect(result.sections[0]!.rationale).toBeUndefined();
});

it("handles partial decision context (only rationale)", () => {
    const md = [
        "# Doc",
        "",
        "## Design",
        "",
        "We chose X.",
        "",
        "> **Rationale:** Because Y.",
    ].join("\n");
    const result = splitMarkdown(md, "test.md");
    expect(result.sections[0]!.content).toBe("We chose X.");
    expect(result.sections[0]!.rationale).toBe("Because Y.");
    expect(result.sections[0]!.alternatives).toBeUndefined();
    expect(result.sections[0]!.consequences).toBeUndefined();
});
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `pnpm --filter @fubbik/api test -- --reporter verbose src/documents/split-markdown.test.ts`
Expected: New tests fail (no decision context extraction happening).

- [ ] **Step 3: Add decision context extraction function**

Add this function to `packages/api/src/documents/split-markdown.ts`, before `splitMarkdown`:

```typescript
interface DecisionContext {
    rationale?: string;
    alternatives?: string[];
    consequences?: string;
}

function extractDecisionContext(content: string): { cleanContent: string; context: DecisionContext } {
    const lines = content.split("\n");
    const context: DecisionContext = {};

    // Find where trailing blockquotes start (scan backwards from end)
    let trailingStart = lines.length;
    for (let i = lines.length - 1; i >= 0; i--) {
        const line = lines[i]!.trim();
        if (line === "" || line.startsWith(">")) {
            trailingStart = i;
        } else {
            break;
        }
    }

    const trailingBlock = lines.slice(trailingStart).join("\n");

    // Check if trailing block contains decision context markers
    const hasDecisionContext =
        trailingBlock.includes("> **Rationale:**") ||
        trailingBlock.includes("> **Alternatives:**") ||
        trailingBlock.includes("> **Consequences:**");

    if (!hasDecisionContext) {
        return { cleanContent: content, context };
    }

    // Parse decision context from trailing blockquotes
    const rationaleMatch = trailingBlock.match(/> \*\*Rationale:\*\*\s*(.*)/);
    if (rationaleMatch) context.rationale = rationaleMatch[1]!.trim();

    const altMatch = trailingBlock.match(/> \*\*Alternatives:\*\*\s*([\s\S]*?)(?=\n> \*\*(?:Rationale|Consequences):\*\*|\n[^>\n]|$)/);
    if (altMatch) {
        const altBlock = altMatch[0]!;
        const items = altBlock.match(/^>\s*-\s+(.+)$/gm);
        if (items) {
            context.alternatives = items.map(item => item.replace(/^>\s*-\s+/, "").trim());
        }
    }

    const consMatch = trailingBlock.match(/> \*\*Consequences:\*\*\s*(.*)/);
    if (consMatch) context.consequences = consMatch[1]!.trim();

    const cleanContent = lines.slice(0, trailingStart).join("\n").trimEnd();
    return { cleanContent, context };
}
```

- [ ] **Step 4: Integrate extraction into section building**

In the section-building loop inside `splitMarkdown`, update the two places where sections are created (the single-section fallback and the heading-split loop). After computing `sectionContent`, apply extraction:

For the preamble and introduction sections, no change needed (they rarely have decision context). For heading-split sections, replace the push with:

```typescript
        const { cleanContent, context } = extractDecisionContext(sectionContent);
        sections.push({
            title: heading.title,
            content: cleanContent,
            order: orderCounter++,
            ...context
        });
```

- [ ] **Step 5: Run tests to verify they pass**

Run: `pnpm --filter @fubbik/api test -- --reporter verbose src/documents/split-markdown.test.ts`
Expected: All tests pass.

- [ ] **Step 6: Run full test suite**

Run: `pnpm test`
Expected: All tests pass.

- [ ] **Step 7: Commit**

```bash
git add packages/api/src/documents/split-markdown.ts packages/api/src/documents/split-markdown.test.ts
git commit -m "feat: extract decision context from trailing blockquotes in splitMarkdown"
```

---

### Task 4: Round-trip `renderDocument` with frontmatter and decision context

**Files:**
- Modify: `packages/api/src/documents/service.ts:250-268`
- Create: `packages/api/src/documents/render-document.test.ts`

- [ ] **Step 1: Write failing round-trip tests**

Create `packages/api/src/documents/render-document.test.ts`:

```typescript
import { describe, expect, it } from "vitest";
import { splitMarkdown } from "./split-markdown";

describe("renderDocument round-trip format", () => {
    it("round-trips frontmatter through split → render → split", () => {
        const md = [
            "---",
            "title: Auth Guide",
            "type: reference",
            "tags:",
            "  - security",
            "  - backend",
            "---",
            "",
            "## Setup",
            "",
            "Install the auth library.",
            "",
            "## Configuration",
            "",
            "Edit config.json.",
        ].join("\n");

        const first = splitMarkdown(md, "docs/auth.md");
        expect(first.title).toBe("Auth Guide");
        expect(first.sections).toHaveLength(2);

        // Simulate render output from the parsed data
        const rendered = renderMarkdown({
            title: first.title,
            type: "reference",
            tags: ["security", "backend"],
            splitLevel: first.splitLevel,
            sections: first.sections,
        });

        const second = splitMarkdown(rendered, "docs/auth.md");
        expect(second.title).toBe(first.title);
        expect(second.sections).toHaveLength(first.sections.length);
        for (let i = 0; i < first.sections.length; i++) {
            expect(second.sections[i]!.title).toBe(first.sections[i]!.title);
            expect(second.sections[i]!.content).toBe(first.sections[i]!.content);
        }
    });

    it("round-trips decision context", () => {
        const md = [
            "---",
            "title: Decisions",
            "---",
            "",
            "## Token Strategy",
            "",
            "We use JWT.",
            "",
            "> **Rationale:** Stateless auth.",
            "",
            "> **Alternatives:**",
            "> - Sessions",
            "> - OAuth",
            "",
            "> **Consequences:** Need refresh tokens.",
        ].join("\n");

        const first = splitMarkdown(md, "test.md");
        expect(first.sections[0]!.rationale).toBe("Stateless auth.");

        const rendered = renderMarkdown({
            title: first.title,
            tags: [],
            splitLevel: first.splitLevel,
            sections: first.sections,
        });

        const second = splitMarkdown(rendered, "test.md");
        expect(second.sections[0]!.rationale).toBe(first.sections[0]!.rationale);
        expect(second.sections[0]!.alternatives).toEqual(first.sections[0]!.alternatives);
        expect(second.sections[0]!.consequences).toBe(first.sections[0]!.consequences);
        expect(second.sections[0]!.content).toBe(first.sections[0]!.content);
    });
});
```

This test references a `renderMarkdown` pure function that doesn't exist yet — the tests will fail.

- [ ] **Step 2: Run tests to verify they fail**

Run: `pnpm --filter @fubbik/api test -- --reporter verbose src/documents/render-document.test.ts`
Expected: Fails with `renderMarkdown is not defined`.

- [ ] **Step 3: Create `renderMarkdown` pure function**

Add the import and function to the test file for now, then extract. Actually — create `packages/api/src/documents/render-markdown.ts`:

```typescript
import type { MarkdownSection } from "./split-markdown";
import { tagsFromPath } from "../chunks/parse-docs";

interface RenderOptions {
    title: string;
    type?: string;
    tags: string[];
    scope?: Record<string, string>;
    splitLevel: number;
    sections: MarkdownSection[];
    sourcePath?: string;
}

export function renderMarkdown(opts: RenderOptions): string {
    const lines: string[] = [];

    // Frontmatter
    const fmLines: string[] = [];
    fmLines.push(`title: ${opts.title}`);
    if (opts.type && opts.type !== "document") {
        fmLines.push(`type: ${opts.type}`);
    }

    // Exclude path-derived tags to avoid duplication on re-import
    const pathTags = opts.sourcePath ? new Set(tagsFromPath(opts.sourcePath)) : new Set<string>();
    const contentTags = opts.tags.filter(t => !pathTags.has(t));
    if (contentTags.length > 0) {
        fmLines.push("tags:");
        for (const tag of contentTags) {
            fmLines.push(`  - ${tag}`);
        }
    }

    if (opts.scope && Object.keys(opts.scope).length > 0) {
        fmLines.push("scope:");
        for (const [key, value] of Object.entries(opts.scope)) {
            fmLines.push(`  ${key}: ${value}`);
        }
    }

    lines.push("---");
    lines.push(...fmLines);
    lines.push("---");
    lines.push("");

    const prefix = "#".repeat(opts.splitLevel);

    for (const section of opts.sections) {
        const isIntro = section.title.endsWith(" — Introduction");

        if (!isIntro) {
            lines.push(`${prefix} ${section.title}`);
            lines.push("");
        }

        if (section.content) {
            lines.push(section.content);
            lines.push("");
        }

        if (section.rationale) {
            lines.push(`> **Rationale:** ${section.rationale}`);
            lines.push("");
        }
        if (section.alternatives && section.alternatives.length > 0) {
            lines.push("> **Alternatives:**");
            for (const alt of section.alternatives) {
                lines.push(`> - ${alt}`);
            }
            lines.push("");
        }
        if (section.consequences) {
            lines.push(`> **Consequences:** ${section.consequences}`);
            lines.push("");
        }
    }

    return lines.join("\n").trimEnd();
}
```

- [ ] **Step 4: Update the test to import `renderMarkdown`**

Add to the top of `packages/api/src/documents/render-document.test.ts`:

```typescript
import { renderMarkdown } from "./render-markdown";
```

- [ ] **Step 5: Run tests to verify they pass**

Run: `pnpm --filter @fubbik/api test -- --reporter verbose src/documents/render-document.test.ts`
Expected: Both round-trip tests pass.

- [ ] **Step 6: Update `renderDocument` service to use `renderMarkdown`**

In `packages/api/src/documents/service.ts`, update the `renderDocument` function to use the new pure function. Add import:

```typescript
import { renderMarkdown } from "./render-markdown";
```

Replace the `renderDocument` function body:

```typescript
export function renderDocument(documentId: string, userId: string) {
    return Effect.gen(function* () {
        const doc = yield* getDocumentById(documentId);
        if (!doc || doc.userId !== userId) return yield* Effect.fail(new NotFoundError({ resource: "document" }));

        const chunks = yield* getDocumentChunks(documentId);
        if (chunks.length === 0) {
            return { document: doc, markdown: `# ${doc.title}\n` };
        }

        // Get tags from first chunk (shared across document chunks)
        const tags = yield* getTagsForChunk(chunks[0]!.id);
        const tagNames = tags.map((t: { name: string }) => t.name);

        const firstChunk = chunks[0]!;
        const scope = firstChunk.scope && Object.keys(firstChunk.scope).length > 0
            ? firstChunk.scope
            : undefined;

        const sections = chunks.map((c, i) => ({
            title: c.title,
            content: c.content ?? "",
            order: c.documentOrder ?? i,
            rationale: c.rationale ?? undefined,
            alternatives: c.alternatives ?? undefined,
            consequences: c.consequences ?? undefined,
        }));

        const markdown = renderMarkdown({
            title: doc.title,
            type: firstChunk.type,
            tags: tagNames,
            scope,
            splitLevel: doc.splitLevel ?? 2,
            sections,
            sourcePath: doc.sourcePath,
        });

        return { document: doc, markdown };
    });
}
```

- [ ] **Step 7: Run full test suite**

Run: `pnpm test`
Expected: All tests pass.

- [ ] **Step 8: Run type-check**

Run: `pnpm run check-types`
Expected: All packages pass.

- [ ] **Step 9: Commit**

```bash
git add packages/api/src/documents/render-markdown.ts packages/api/src/documents/render-document.test.ts packages/api/src/documents/service.ts
git commit -m "feat: round-trip renderDocument with frontmatter and decision context"
```

---

### Task 5: Persist `splitLevel` during import

**Files:**
- Modify: `packages/api/src/documents/service.ts`
- Modify: `packages/db/src/repository/document.ts`

- [ ] **Step 1: Update `createDocument` params to accept `splitLevel`**

In `packages/db/src/repository/document.ts`, add `splitLevel` to the interface:

```typescript
export interface CreateDocumentParams {
    id: string;
    title: string;
    sourcePath: string;
    contentHash: string;
    description?: string;
    codebaseId?: string;
    userId: string;
    splitLevel?: number;
}
```

- [ ] **Step 2: Update `updateDocument` to accept `splitLevel`**

In `packages/db/src/repository/document.ts`, update the `updateDocument` params type:

```typescript
export function updateDocument(id: string, params: { title?: string; contentHash?: string; description?: string; splitLevel?: number }) {
    return dbEffect(async () => {
            const [updated] = await db
                .update(document)
                .set({
                    ...(params.title !== undefined && { title: params.title }),
                    ...(params.contentHash !== undefined && { contentHash: params.contentHash }),
                    ...(params.description !== undefined && { description: params.description }),
                    ...(params.splitLevel !== undefined && { splitLevel: params.splitLevel })
                })
                .where(eq(document.id, id))
                .returning();
            return updated;
        });
}
```

- [ ] **Step 3: Pass `splitLevel` in `importDocument`**

In `packages/api/src/documents/service.ts`, in the default (non-template) import path, pass the detected split level to `createDocumentRepo`:

Find the line:
```typescript
        const split = splitMarkdown(rawContent, sourcePath);
```

The `createDocumentRepo` call below it should include `splitLevel`:

```typescript
        const doc = yield* createDocumentRepo({
            id: docId,
            title: split.title,
            sourcePath,
            contentHash,
            description: split.description,
            codebaseId,
            userId,
            splitLevel: split.splitLevel
        });
```

- [ ] **Step 4: Pass `splitLevel` in `syncDocument`**

In the `syncDocument` function, update the `updateDocumentRepo` call at the end:

```typescript
        yield* updateDocumentRepo(documentId, {
            title: split.title,
            contentHash,
            description: split.description,
            splitLevel: split.splitLevel
        });
```

- [ ] **Step 5: Run type-check**

Run: `pnpm run check-types`
Expected: All packages pass.

- [ ] **Step 6: Run full test suite**

Run: `pnpm test`
Expected: All tests pass.

- [ ] **Step 7: Commit**

```bash
git add packages/db/src/repository/document.ts packages/api/src/documents/service.ts
git commit -m "feat: persist splitLevel during document import and sync"
```

---

### Task 6: Promise-based mermaid loading

**Files:**
- Modify: `apps/web/src/components/markdown-renderer.tsx:1-73`

- [ ] **Step 1: Replace mermaid polling with promise**

In `apps/web/src/components/markdown-renderer.tsx`, replace lines 9-16 (the module-level mermaid setup):

```typescript
const mermaidPromise = typeof window !== "undefined"
    ? import("mermaid").then(m => {
        m.default.initialize({ startOnLoad: false, theme: "dark" });
        return m.default;
    })
    : null;
```

- [ ] **Step 2: Rewrite `MermaidBlock` to use the promise**

Replace the `MermaidBlock` function (lines 23-73):

```typescript
function MermaidBlock({ children }: { children: string }) {
    const id = useId().replace(/:/g, "-");
    const [svg, setSvg] = useState<string | null>(null);
    const [error, setError] = useState<string | null>(null);

    useEffect(() => {
        let cancelled = false;
        if (!mermaidPromise) return;

        mermaidPromise
            .then(mermaid => mermaid.render(`mermaid-${id}`, children.trim()))
            .then(({ svg: rendered }) => {
                if (!cancelled) setSvg(rendered);
            })
            .catch(err => {
                if (!cancelled) setError(String(err));
            });

        return () => { cancelled = true; };
    }, [children, id]);

    if (error) {
        return (
            <pre className="overflow-x-auto rounded-lg bg-red-950/30 border border-red-500/20 p-4 text-sm text-red-400">
                <code>{children}</code>
            </pre>
        );
    }

    if (!svg) {
        return (
            <div className="flex items-center justify-center rounded-lg border border-border/40 bg-muted/20 p-8 text-sm text-muted-foreground">
                Rendering diagram...
            </div>
        );
    }

    return (
        <div
            className="my-4 flex justify-center overflow-x-auto [&_svg]:max-w-full"
            dangerouslySetInnerHTML={{ __html: svg }}
        />
    );
}
```

- [ ] **Step 3: Verify in browser**

Run: `pnpm dev`
Navigate to a chunk that contains a mermaid code block. Verify the diagram renders without visual delay artifacts.

- [ ] **Step 4: Run type-check**

Run: `pnpm run check-types`
Expected: All packages pass.

- [ ] **Step 5: Commit**

```bash
git add apps/web/src/components/markdown-renderer.tsx
git commit -m "fix: replace mermaid polling with promise-based lazy load"
```

---

### Task 7: Copy button on code blocks

**Files:**
- Modify: `apps/web/src/components/markdown-renderer.tsx`

- [ ] **Step 1: Add CopyButton component**

Add this component in `apps/web/src/components/markdown-renderer.tsx` after the mermaid section, before `CodeBlock`:

```typescript
function CopyButton({ code }: { code: string }) {
    const [copied, setCopied] = useState(false);

    const handleCopy = () => {
        navigator.clipboard.writeText(code).then(() => {
            setCopied(true);
            setTimeout(() => setCopied(false), 2000);
        });
    };

    return (
        <button
            onClick={handleCopy}
            className="absolute right-2 top-2 rounded-md border border-border/50 bg-background/80 p-1.5 text-muted-foreground opacity-0 backdrop-blur transition-opacity hover:text-foreground group-hover:opacity-100"
            aria-label="Copy code"
        >
            {copied ? (
                <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
                    <polyline points="20 6 9 17 4 12" />
                </svg>
            ) : (
                <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
                    <rect x="9" y="9" width="13" height="13" rx="2" ry="2" />
                    <path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1" />
                </svg>
            )}
        </button>
    );
}
```

- [ ] **Step 2: Wrap CodeBlock output in a group container with CopyButton**

Update the `CodeBlock` component. Wrap each return path (shiki-highlighted and fallback) in a `relative group` div with the copy button. Replace the shiki-highlighted return:

```typescript
    if (html) {
        return (
            <div className="group relative">
                <CopyButton code={code} />
                <div
                    className="my-3 overflow-x-auto rounded-lg border border-border/30 bg-[#f6f8fa] text-sm dark:bg-[#22272e] [&_pre]:!p-4 [&_pre]:!rounded-lg [&_.shiki]:!bg-transparent"
                    dangerouslySetInnerHTML={{ __html: html }}
                />
            </div>
        );
    }
```

Replace the plain fallback return:

```typescript
    return (
        <div className="group relative">
            <CopyButton code={code} />
            <pre className="my-3 overflow-x-auto rounded-lg border border-border/30 bg-[#f6f8fa] p-4 text-sm dark:bg-[#22272e]">
                <code>{code}</code>
            </pre>
        </div>
    );
```

- [ ] **Step 3: Verify in browser**

Run: `pnpm dev`
Navigate to a chunk with code blocks. Hover over a code block — copy button should appear top-right. Click it — icon should change to checkmark for 2 seconds.

- [ ] **Step 4: Commit**

```bash
git add apps/web/src/components/markdown-renderer.tsx
git commit -m "feat: add copy button to code blocks"
```

---

### Task 8: Auto-generated table of contents

**Files:**
- Modify: `apps/web/src/components/markdown-renderer.tsx`

- [ ] **Step 1: Add slug generation utility**

Add at the top of the component section in `markdown-renderer.tsx`:

```typescript
function slugify(text: string): string {
    return text
        .toLowerCase()
        .replace(/[^\w\s-]/g, "")
        .replace(/\s+/g, "-")
        .replace(/-+/g, "-")
        .trim();
}

interface TocEntry {
    level: number;
    text: string;
    slug: string;
}

function extractToc(markdown: string): TocEntry[] {
    const entries: TocEntry[] = [];
    const regex = /^(#{1,6})\s+(.+)$/gm;
    let match: RegExpExecArray | null;
    while ((match = regex.exec(markdown)) !== null) {
        entries.push({
            level: match[1]!.length,
            text: match[2]!.trim(),
            slug: slugify(match[2]!.trim()),
        });
    }
    return entries;
}
```

- [ ] **Step 2: Add TOC component**

```typescript
function TableOfContents({ entries }: { entries: TocEntry[] }) {
    const minLevel = Math.min(...entries.map(e => e.level));

    return (
        <nav className="mb-6 rounded-lg border border-border/50 bg-muted/20 px-4 py-3">
            <p className="mb-2 text-xs font-semibold uppercase tracking-wider text-muted-foreground">
                Table of Contents
            </p>
            <ul className="space-y-1 text-sm">
                {entries.map((entry, i) => (
                    <li key={i} style={{ marginLeft: `${(entry.level - minLevel) * 16}px` }}>
                        <a
                            href={`#${entry.slug}`}
                            className="text-muted-foreground hover:text-foreground transition-colors"
                        >
                            {entry.text}
                        </a>
                    </li>
                ))}
            </ul>
        </nav>
    );
}
```

- [ ] **Step 3: Add heading component overrides with `id` attributes**

Add heading overrides to the `components` object in `markdown-renderer.tsx`. Add after the `img` override:

```typescript
    h1({ children }) {
        const text = typeof children === "string" ? children : String(children);
        return <h1 id={slugify(text)}>{children}</h1>;
    },
    h2({ children }) {
        const text = typeof children === "string" ? children : String(children);
        return <h2 id={slugify(text)}>{children}</h2>;
    },
    h3({ children }) {
        const text = typeof children === "string" ? children : String(children);
        return <h3 id={slugify(text)}>{children}</h3>;
    },
    h4({ children }) {
        const text = typeof children === "string" ? children : String(children);
        return <h4 id={slugify(text)}>{children}</h4>;
    },
    h5({ children }) {
        const text = typeof children === "string" ? children : String(children);
        return <h5 id={slugify(text)}>{children}</h5>;
    },
    h6({ children }) {
        const text = typeof children === "string" ? children : String(children);
        return <h6 id={slugify(text)}>{children}</h6>;
    },
```

- [ ] **Step 4: Integrate TOC into MarkdownRenderer**

Update the `MarkdownRenderer` export to compute and render the TOC:

```typescript
export function MarkdownRenderer({ children, excludeChunkId }: { children: string; excludeChunkId?: string }) {
    const tocEntries = useMemo(() => extractToc(children), [children]);

    return (
        <ExcludeChunkContext.Provider value={excludeChunkId}>
            {tocEntries.length >= 3 && <TableOfContents entries={tocEntries} />}
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

Add `useMemo` to the import from `react` at the top of the file if not already there.

- [ ] **Step 5: Verify in browser**

Run: `pnpm dev`
Navigate to a chunk with 3+ headings. Verify TOC appears above content with indentation. Click a TOC link — page should scroll to the heading.

- [ ] **Step 6: Run type-check**

Run: `pnpm run check-types`
Expected: All packages pass.

- [ ] **Step 7: Commit**

```bash
git add apps/web/src/components/markdown-renderer.tsx
git commit -m "feat: auto-generated table of contents for markdown content"
```

---

### Task 9: Vocabulary matching performance

**Files:**
- Modify: `apps/web/src/components/smart-link-provider.tsx`
- Modify: `apps/web/src/components/markdown-renderer.tsx`

- [ ] **Step 1: Precompile vocabulary regex in the provider**

In `apps/web/src/components/smart-link-provider.tsx`, update the context type and provider to include a precompiled regex. Change the interface:

```typescript
interface SmartLinkContextValue {
    chunkIndex: Map<string, ChunkMatch>;
    vocabIndex: Map<string, VocabularyMatch>;
    fileRefIndex: Map<string, FileRefMatch>;
    vocabPattern: RegExp | null;
}
```

Update the default context:

```typescript
const SmartLinkContext = createContext<SmartLinkContextValue>({
    chunkIndex: new Map(),
    vocabIndex: new Map(),
    fileRefIndex: new Map(),
    vocabPattern: null
});
```

Add a regex builder function:

```typescript
export function buildVocabPattern(vocabIndex: Map<string, VocabularyMatch>): RegExp | null {
    if (vocabIndex.size === 0) return null;
    const words = Array.from(vocabIndex.keys()).sort((a, b) => b.length - a.length);
    const escaped = words.map(w => w.replace(/[.*+?^${}()|[\]\\]/g, "\\$&"));
    return new RegExp(`\\b(${escaped.join("|")})\\b`, "gi");
}
```

Update the `useMemo` in `SmartLinkProvider`:

```typescript
    const value = useMemo<SmartLinkContextValue>(() => {
        const chunkIndex = buildChunkIndex(chunksQuery.data ?? []);
        const vocabIndex = buildVocabularyIndex(vocabQuery.data ?? []);
        const fileRefIndex = buildFileRefIndex(fileRefsQuery.data ?? []);
        const vocabPattern = buildVocabPattern(vocabIndex);
        return { chunkIndex, vocabIndex, fileRefIndex, vocabPattern };
    }, [chunksQuery.data, vocabQuery.data, fileRefsQuery.data]);
```

- [ ] **Step 2: Update `matchVocabularyInText` to accept precompiled pattern**

Update the function signature in `smart-link-provider.tsx`:

```typescript
export function matchVocabularyInText(
    text: string,
    vocabIndex: Map<string, VocabularyMatch>,
    pattern?: RegExp | null
): VocabularyTextMatch[] {
    if (vocabIndex.size === 0) return [];

    const regex = pattern
        ? new RegExp(pattern.source, pattern.flags)  // clone to reset lastIndex
        : (() => {
            const words = Array.from(vocabIndex.keys()).sort((a, b) => b.length - a.length);
            const escaped = words.map(w => w.replace(/[.*+?^${}()|[\]\\]/g, "\\$&"));
            return new RegExp(`\\b(${escaped.join("|")})\\b`, "gi");
        })();

    const matches: VocabularyTextMatch[] = [];
    let m: RegExpExecArray | null;
    while ((m = regex.exec(text)) !== null) {
        const vocab = vocabIndex.get(m[1]!.toLowerCase());
        if (vocab) {
            matches.push({ start: m.index, end: m.index + m[0].length, ...vocab });
        }
    }

    return matches;
}
```

- [ ] **Step 3: Memoize `SmartText`, `SmartParagraph`, and `SmartListItem`**

In `apps/web/src/components/markdown-renderer.tsx`, update the imports from `react`:

```typescript
import { createContext, memo, useContext, useEffect, useId, useMemo, useState } from "react";
```

Wrap `SmartText` in `memo` and use `useMemo` for matches:

```typescript
const SmartText = memo(function SmartText({ children }: { children: string }) {
    const { vocabIndex, vocabPattern } = useSmartLinks();
    const matches = useMemo(
        () => matchVocabularyInText(children, vocabIndex, vocabPattern),
        [children, vocabIndex, vocabPattern]
    );

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
});
```

Wrap `SmartParagraph` and `SmartListItem`:

```typescript
const SmartParagraph = memo(function SmartParagraph({ children }: { children: React.ReactNode }) {
    return <p>{processChildren(children)}</p>;
});

const SmartListItem = memo(function SmartListItem({ children }: { children: React.ReactNode }) {
    return <li>{processChildren(children)}</li>;
});
```

- [ ] **Step 4: Run existing smart-link-provider tests**

Run: `pnpm --filter web test -- --reporter verbose src/components/smart-link-provider.test.ts`
Expected: All 13 tests pass (the `matchVocabularyInText` tests still work since the `pattern` param is optional).

- [ ] **Step 5: Run type-check**

Run: `pnpm run check-types`
Expected: All packages pass.

- [ ] **Step 6: Verify in browser**

Run: `pnpm dev`
Navigate to a chunk with vocabulary terms. Verify popovers still appear on hover. Navigate between chunks — verify no visible performance difference or broken behavior.

- [ ] **Step 7: Commit**

```bash
git add apps/web/src/components/smart-link-provider.tsx apps/web/src/components/markdown-renderer.tsx
git commit -m "perf: precompile vocab regex and memoize markdown text matching"
```

---

### Task 10: Final integration verification

**Files:** None (verification only)

- [ ] **Step 1: Run full test suite**

Run: `pnpm test`
Expected: All tests pass across all packages.

- [ ] **Step 2: Run type-check**

Run: `pnpm run check-types`
Expected: All packages pass.

- [ ] **Step 3: Run full CI pipeline**

Run: `pnpm ci`
Expected: type-check, lint, test, build, format check, sherif all pass.

- [ ] **Step 4: Manual browser verification**

Run: `pnpm dev`

Verify:
1. Import a folder of markdown docs — chunks get folder connections (from previous work)
2. View a chunk with 3+ headings — TOC appears, links scroll to headings
3. View a chunk with code blocks — copy button appears on hover, copies correctly
4. View a chunk with mermaid diagrams — renders without polling artifacts
5. View a chunk with vocabulary terms — popovers work, no visible lag
6. Export a document via the API — frontmatter and decision context are present
7. Re-import the exported markdown — round-trips cleanly (same content, tags, decision context)

- [ ] **Step 5: Commit any final adjustments**

If browser testing reveals issues, fix and commit individually.
