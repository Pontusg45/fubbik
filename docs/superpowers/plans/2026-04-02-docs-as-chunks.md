# Docs as Chunks — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Import markdown docs, split on H2 headings into ordered chunks tracked by a document entity, and render them as browsable pages in the web UI.

**Architecture:** New `document` table with source path + content hash tracking. Two new nullable columns on `chunk` (`documentId`, `documentOrder`). New API route module, CLI subcommand group, and a "Documents" tab on the existing `/docs` page.

**Tech Stack:** Drizzle ORM, Effect, Elysia, TanStack Router + React Query, Commander.js

---

## File Structure

### New Files
| File | Responsibility |
|------|---------------|
| `packages/db/src/schema/document.ts` | Document table + relations |
| `packages/db/src/repository/document.ts` | Document CRUD with Effect |
| `packages/api/src/documents/service.ts` | Split, sync, render logic |
| `packages/api/src/documents/routes.ts` | HTTP endpoints |
| `packages/api/src/documents/split-markdown.ts` | H2 splitting function |
| `packages/api/src/documents/split-markdown.test.ts` | Tests for splitting |
| `packages/api/src/documents/service.test.ts` | Tests for sync logic |
| `apps/cli/src/commands/docs.ts` | CLI subcommand group |
| `apps/web/src/features/documents/document-browser.tsx` | Sidebar + content reader |

### Modified Files
| File | Changes |
|------|---------|
| `packages/db/src/schema/chunk.ts` | Add `documentId`, `documentOrder` columns |
| `packages/db/src/schema/index.ts` | Export document schema |
| `packages/db/src/repository/index.ts` | Export document repository |
| `packages/api/src/index.ts` | Mount document routes |
| `apps/cli/src/index.ts` | Register docs command |
| `apps/web/src/routes/docs.tsx` | Add "Documents" tab |

---

### Task 1: Database Schema — Document Table

**Files:**
- Create: `packages/db/src/schema/document.ts`
- Modify: `packages/db/src/schema/chunk.ts`
- Modify: `packages/db/src/schema/index.ts`

- [ ] **Step 1: Create the document schema file**

```typescript
// packages/db/src/schema/document.ts
import { relations } from "drizzle-orm";
import { index, pgTable, text, timestamp, uniqueIndex } from "drizzle-orm/pg-core";

import { chunk } from "./chunk";
import { codebase } from "./codebase";
import { user } from "./auth";

export const document = pgTable(
    "document",
    {
        id: text("id").primaryKey(),
        title: text("title").notNull(),
        sourcePath: text("source_path").notNull(),
        contentHash: text("content_hash").notNull(),
        description: text("description"),
        codebaseId: text("codebase_id").references(() => codebase.id, { onDelete: "set null" }),
        userId: text("user_id")
            .notNull()
            .references(() => user.id, { onDelete: "cascade" }),
        createdAt: timestamp("created_at").defaultNow().notNull(),
        updatedAt: timestamp("updated_at")
            .defaultNow()
            .$onUpdate(() => new Date())
            .notNull()
    },
    table => [
        uniqueIndex("document_source_codebase_user_idx").on(table.sourcePath, table.codebaseId, table.userId),
        index("document_userId_idx").on(table.userId),
        index("document_codebaseId_idx").on(table.codebaseId)
    ]
);

export const documentRelations = relations(document, ({ one, many }) => ({
    user: one(user, { fields: [document.userId], references: [user.id] }),
    codebase: one(codebase, { fields: [document.codebaseId], references: [codebase.id] }),
    chunks: many(chunk)
}));
```

- [ ] **Step 2: Add documentId and documentOrder to chunk table**

In `packages/db/src/schema/chunk.ts`, add two columns to the `chunk` table definition and an import for the `document` table:

Add import at the top:
```typescript
import { document } from "./document";
```

Add columns after `archivedAt`:
```typescript
        documentId: text("document_id").references(() => document.id, { onDelete: "set null" }),
        documentOrder: integer("document_order"),
```

Also add `integer` to the `drizzle-orm/pg-core` import.

Add a unique index in the table's index array:
```typescript
        uniqueIndex("chunk_document_order_idx").on(table.documentId, table.documentOrder).where(sql`${table.documentId} IS NOT NULL`)
```

Also add `sql` to the `drizzle-orm` import.

Add a relation to `chunkRelations`:
```typescript
    document: one(document, { fields: [chunk.documentId], references: [document.id] }),
```

- [ ] **Step 3: Export document schema from index**

Add to `packages/db/src/schema/index.ts`:
```typescript
export * from "./document";
```

- [ ] **Step 4: Push schema to database**

Run: `cd packages/db && pnpm db:push`
Expected: Schema changes applied (new table + new columns)

- [ ] **Step 5: Commit**

```bash
git add packages/db/src/schema/document.ts packages/db/src/schema/chunk.ts packages/db/src/schema/index.ts
git commit -m "feat(db): add document table and chunk document columns"
```

---

### Task 2: H2 Markdown Splitting Function

**Files:**
- Create: `packages/api/src/documents/split-markdown.ts`
- Test: `packages/api/src/documents/split-markdown.test.ts`

- [ ] **Step 1: Write the failing test**

```typescript
// packages/api/src/documents/split-markdown.test.ts
import { describe, expect, it } from "vitest";

import { splitMarkdown } from "./split-markdown";

describe("splitMarkdown", () => {
    it("splits on H2 headings", () => {
        const md = `# My Document

Intro paragraph.

## First Section

First content.

## Second Section

Second content.
`;
        const result = splitMarkdown(md, "docs/test.md");

        expect(result.title).toBe("My Document");
        expect(result.sections).toHaveLength(3);
        expect(result.sections[0]).toEqual({
            title: "My Document \u2014 Introduction",
            content: "Intro paragraph.",
            order: 0
        });
        expect(result.sections[1]).toEqual({
            title: "First Section",
            content: "First content.",
            order: 1
        });
        expect(result.sections[2]).toEqual({
            title: "Second Section",
            content: "Second content.",
            order: 2
        });
    });

    it("skips empty preamble", () => {
        const md = `# Title

## Only Section

Content here.
`;
        const result = splitMarkdown(md, "test.md");
        expect(result.sections).toHaveLength(1);
        expect(result.sections[0]!.title).toBe("Only Section");
        expect(result.sections[0]!.order).toBe(0);
    });

    it("falls back to filename for title", () => {
        const md = `## Section One

Content.
`;
        const result = splitMarkdown(md, "docs/my-cool-guide.md");
        expect(result.title).toBe("my cool guide");
    });

    it("preserves H3+ subheadings within sections", () => {
        const md = `# Doc

## Main

### Sub

Details.

#### Deep

More.
`;
        const result = splitMarkdown(md, "test.md");
        expect(result.sections).toHaveLength(1);
        expect(result.sections[0]!.content).toContain("### Sub");
        expect(result.sections[0]!.content).toContain("#### Deep");
    });

    it("extracts frontmatter tags and description", () => {
        const md = `---
tags:
  - backend
  - auth
description: A guide to auth
---

# Auth Guide

## Setup

Steps here.
`;
        const result = splitMarkdown(md, "docs/auth.md");
        expect(result.title).toBe("Auth Guide");
        expect(result.tags).toEqual(["backend", "auth"]);
        expect(result.description).toBe("A guide to auth");
    });

    it("treats whole file as single section when no H2s", () => {
        const md = `# Simple Note

Just some content with no H2 headings.
`;
        const result = splitMarkdown(md, "note.md");
        expect(result.sections).toHaveLength(1);
        expect(result.sections[0]!.title).toBe("Simple Note \u2014 Introduction");
        expect(result.sections[0]!.content).toBe("Just some content with no H2 headings.");
    });
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cd packages/api && pnpm vitest run src/documents/split-markdown.test.ts`
Expected: FAIL — module not found

- [ ] **Step 3: Implement the splitting function**

```typescript
// packages/api/src/documents/split-markdown.ts
import { extractFrontmatter } from "../chunks/parse-docs";

export interface MarkdownSection {
    title: string;
    content: string;
    order: number;
}

export interface SplitResult {
    title: string;
    description?: string;
    tags: string[];
    sections: MarkdownSection[];
}

export function splitMarkdown(raw: string, filePath: string): SplitResult {
    const { frontmatter, body } = extractFrontmatter(raw);

    // Extract title from frontmatter or H1
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

    // Extract tags from frontmatter
    const tags = Array.isArray(frontmatter.tags) ? frontmatter.tags.map(String) : [];
    const description = (frontmatter.description as string) ?? undefined;

    // Split on ## headings
    const h2Regex = /^## (.+)$/gm;
    const matches: { title: string; index: number }[] = [];
    let match: RegExpExecArray | null;
    while ((match = h2Regex.exec(content)) !== null) {
        matches.push({ title: match[1]!.trim(), index: match.index });
    }

    const sections: MarkdownSection[] = [];
    let orderCounter = 0;

    if (matches.length === 0) {
        // No H2 headings — whole content is one section
        const trimmed = content.trim();
        if (trimmed) {
            sections.push({ title: `${title} \u2014 Introduction`, content: trimmed, order: orderCounter });
        }
        return { title, description, tags, sections };
    }

    // Preamble (content before first H2)
    const preamble = content.slice(0, matches[0]!.index).trim();
    if (preamble) {
        sections.push({ title: `${title} \u2014 Introduction`, content: preamble, order: orderCounter++ });
    }

    // Each H2 section
    for (let i = 0; i < matches.length; i++) {
        const heading = matches[i]!;
        const nextIndex = i + 1 < matches.length ? matches[i + 1]!.index : content.length;
        const sectionContent = content
            .slice(heading.index + `## ${heading.title}`.length + 1, nextIndex)
            .trim();

        sections.push({
            title: heading.title,
            content: sectionContent,
            order: orderCounter++
        });
    }

    return { title, description, tags, sections };
}
```

Note: This requires `extractFrontmatter` to be exported from `packages/api/src/chunks/parse-docs.ts`. If it's not already exported, add `export` to the function declaration.

- [ ] **Step 4: Export extractFrontmatter from parse-docs**

In `packages/api/src/chunks/parse-docs.ts`, change:
```typescript
function extractFrontmatter(raw: string)
```
to:
```typescript
export function extractFrontmatter(raw: string)
```

- [ ] **Step 5: Run test to verify it passes**

Run: `cd packages/api && pnpm vitest run src/documents/split-markdown.test.ts`
Expected: All 6 tests PASS

- [ ] **Step 6: Commit**

```bash
git add packages/api/src/documents/split-markdown.ts packages/api/src/documents/split-markdown.test.ts packages/api/src/chunks/parse-docs.ts
git commit -m "feat: add H2 markdown splitting function with tests"
```

---

### Task 3: Document Repository

**Files:**
- Create: `packages/db/src/repository/document.ts`
- Modify: `packages/db/src/repository/index.ts`

- [ ] **Step 1: Create the document repository**

```typescript
// packages/db/src/repository/document.ts
import { and, eq, isNull, sql } from "drizzle-orm";
import { Effect } from "effect";

import { DatabaseError } from "../errors";
import { db } from "../index";
import { chunk } from "../schema/chunk";
import { document } from "../schema/document";

export interface CreateDocumentParams {
    id: string;
    title: string;
    sourcePath: string;
    contentHash: string;
    description?: string;
    codebaseId?: string;
    userId: string;
}

export function createDocument(params: CreateDocumentParams) {
    return Effect.tryPromise({
        try: async () => {
            const [created] = await db.insert(document).values(params).returning();
            return created;
        },
        catch: cause => new DatabaseError({ cause })
    });
}

export function getDocumentById(id: string) {
    return Effect.tryPromise({
        try: async () => {
            const [doc] = await db.select().from(document).where(eq(document.id, id));
            return doc ?? null;
        },
        catch: cause => new DatabaseError({ cause })
    });
}

export function getDocumentBySourcePath(sourcePath: string, codebaseId: string | undefined, userId: string) {
    return Effect.tryPromise({
        try: async () => {
            const conditions = [eq(document.sourcePath, sourcePath), eq(document.userId, userId)];
            if (codebaseId) {
                conditions.push(eq(document.codebaseId, codebaseId));
            } else {
                conditions.push(isNull(document.codebaseId));
            }
            const [doc] = await db.select().from(document).where(and(...conditions));
            return doc ?? null;
        },
        catch: cause => new DatabaseError({ cause })
    });
}

export function listDocuments(userId: string, codebaseId?: string) {
    return Effect.tryPromise({
        try: async () => {
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
                    chunkCount: sql<number>`count(${chunk.id})`.as("chunk_count")
                })
                .from(document)
                .leftJoin(chunk, eq(chunk.documentId, document.id))
                .where(and(...conditions))
                .groupBy(document.id)
                .orderBy(document.title);
            return docs;
        },
        catch: cause => new DatabaseError({ cause })
    });
}

export function updateDocument(id: string, params: { title?: string; contentHash?: string; description?: string }) {
    return Effect.tryPromise({
        try: async () => {
            const [updated] = await db
                .update(document)
                .set({
                    ...(params.title !== undefined && { title: params.title }),
                    ...(params.contentHash !== undefined && { contentHash: params.contentHash }),
                    ...(params.description !== undefined && { description: params.description })
                })
                .where(eq(document.id, id))
                .returning();
            return updated;
        },
        catch: cause => new DatabaseError({ cause })
    });
}

export function deleteDocument(id: string) {
    return Effect.tryPromise({
        try: async () => {
            // Unlink chunks (don't delete them)
            await db.update(chunk).set({ documentId: null, documentOrder: null }).where(eq(chunk.documentId, id));
            const [deleted] = await db.delete(document).where(eq(document.id, id)).returning();
            return deleted;
        },
        catch: cause => new DatabaseError({ cause })
    });
}

export function getDocumentChunks(documentId: string) {
    return Effect.tryPromise({
        try: async () => {
            const chunks = await db
                .select()
                .from(chunk)
                .where(eq(chunk.documentId, documentId))
                .orderBy(chunk.documentOrder);
            return chunks;
        },
        catch: cause => new DatabaseError({ cause })
    });
}
```

- [ ] **Step 2: Export from repository index**

Add to `packages/db/src/repository/index.ts`:
```typescript
export * from "./document";
```

- [ ] **Step 3: Commit**

```bash
git add packages/db/src/repository/document.ts packages/db/src/repository/index.ts
git commit -m "feat(db): add document repository with CRUD operations"
```

---

### Task 4: Document Service — Import & Render

**Files:**
- Create: `packages/api/src/documents/service.ts`

- [ ] **Step 1: Create the document service**

```typescript
// packages/api/src/documents/service.ts
import {
    createChunk as createChunkRepo,
    createDocument as createDocumentRepo,
    deleteDocument as deleteDocumentRepo,
    getDocumentById,
    getDocumentBySourcePath,
    getDocumentChunks,
    getTagsForChunk,
    listDocuments as listDocumentsRepo,
    setChunkCodebases,
    setChunkTags,
    updateChunk as updateChunkRepo,
    updateDocument as updateDocumentRepo
} from "@fubbik/db/repository";
import { Effect } from "effect";

import { NotFoundError } from "../errors";
import { splitMarkdown } from "./split-markdown";

function hashContent(content: string): string {
    const encoder = new TextEncoder();
    const data = encoder.encode(content);
    // Use Bun's built-in hashing
    const hasher = new Bun.CryptoHasher("sha256");
    hasher.update(data);
    return hasher.digest("hex");
}

export function importDocument(
    userId: string,
    sourcePath: string,
    rawContent: string,
    codebaseId?: string
) {
    return Effect.gen(function* () {
        const contentHash = hashContent(rawContent);

        // Check if already imported
        const existing = yield* getDocumentBySourcePath(sourcePath, codebaseId, userId);
        if (existing && existing.contentHash === contentHash) {
            return { document: existing, created: 0, updated: 0, status: "unchanged" as const };
        }

        if (existing) {
            return yield* syncDocument(existing.id, rawContent, userId, codebaseId);
        }

        // New import
        const split = splitMarkdown(rawContent, sourcePath);
        const docId = crypto.randomUUID();

        const doc = yield* createDocumentRepo({
            id: docId,
            title: split.title,
            sourcePath,
            contentHash,
            description: split.description,
            codebaseId,
            userId
        });

        for (const section of split.sections) {
            const chunkId = crypto.randomUUID();
            yield* createChunkRepo({
                id: chunkId,
                title: section.title,
                content: section.content,
                type: "document",
                userId,
                documentId: docId,
                documentOrder: section.order
            });

            if (split.tags.length > 0) {
                yield* setChunkTags(chunkId, split.tags, userId);
            }
            if (codebaseId) {
                yield* setChunkCodebases(chunkId, [codebaseId]);
            }
        }

        return { document: doc, created: split.sections.length, updated: 0, status: "created" as const };
    });
}

export function syncDocument(
    documentId: string,
    rawContent: string,
    userId: string,
    codebaseId?: string
) {
    return Effect.gen(function* () {
        const doc = yield* getDocumentById(documentId);
        if (!doc) return yield* Effect.fail(new NotFoundError({ resource: "document" }));

        const contentHash = hashContent(rawContent);
        if (doc.contentHash === contentHash) {
            return { document: doc, created: 0, updated: 0, status: "unchanged" as const };
        }

        const split = splitMarkdown(rawContent, doc.sourcePath);
        const existingChunks = yield* getDocumentChunks(documentId);

        // Build lookup of existing chunks by normalized title
        const normalize = (t: string) => t.trim().toLowerCase();
        const existingByTitle = new Map(existingChunks.map(c => [normalize(c.title), c]));
        const matchedIds = new Set<string>();

        let created = 0;
        let updated = 0;

        for (const section of split.sections) {
            const match = existingByTitle.get(normalize(section.title));

            if (match) {
                matchedIds.add(match.id);
                // Update content and order if changed
                if (match.content !== section.content || match.documentOrder !== section.order) {
                    yield* updateChunkRepo(match.id, {
                        content: section.content,
                        documentOrder: section.order
                    });
                    updated++;
                }
            } else {
                // New section
                const chunkId = crypto.randomUUID();
                yield* createChunkRepo({
                    id: chunkId,
                    title: section.title,
                    content: section.content,
                    type: "document",
                    userId,
                    documentId,
                    documentOrder: section.order
                });

                if (split.tags.length > 0) {
                    yield* setChunkTags(chunkId, split.tags, userId);
                }
                if (codebaseId) {
                    yield* setChunkCodebases(chunkId, [codebaseId]);
                }
                created++;
            }
        }

        // Flag deleted sections as stale (don't delete)
        for (const existing of existingChunks) {
            if (!matchedIds.has(existing.id)) {
                // getTagsForChunk returns tags with name; extract names and add "stale"
                const currentTags = yield* getTagsForChunk(existing.id);
                const tagNames = currentTags.map((t: { name: string }) => t.name);
                if (!tagNames.includes("stale")) {
                    yield* setChunkTags(existing.id, [...tagNames, "stale"], userId);
                }
            }
        }

        yield* updateDocumentRepo(documentId, {
            title: split.title,
            contentHash,
            description: split.description
        });

        return { document: doc, created, updated, status: "synced" as const };
    });
}

export function renderDocument(documentId: string) {
    return Effect.gen(function* () {
        const doc = yield* getDocumentById(documentId);
        if (!doc) return yield* Effect.fail(new NotFoundError({ resource: "document" }));

        const chunks = yield* getDocumentChunks(documentId);

        let markdown = `# ${doc.title}\n\n`;
        for (const c of chunks) {
            // If it's an introduction section, don't re-add H2
            if (c.title.endsWith("\u2014 Introduction")) {
                markdown += `${c.content}\n\n`;
            } else {
                markdown += `## ${c.title}\n\n${c.content}\n\n`;
            }
        }

        return { document: doc, markdown: markdown.trim() };
    });
}

export function listDocuments(userId: string, codebaseId?: string) {
    return listDocumentsRepo(userId, codebaseId);
}

export function getDocument(documentId: string) {
    return Effect.gen(function* () {
        const doc = yield* getDocumentById(documentId);
        if (!doc) return yield* Effect.fail(new NotFoundError({ resource: "document" }));
        const chunks = yield* getDocumentChunks(documentId);
        return { ...doc, chunks };
    });
}

export function removeDocument(documentId: string) {
    return deleteDocumentRepo(documentId);
}
```

- [ ] **Step 2: Commit**

```bash
git add packages/api/src/documents/service.ts
git commit -m "feat: add document service with import, sync, and render"
```

---

### Task 5: Document API Routes

**Files:**
- Create: `packages/api/src/documents/routes.ts`
- Modify: `packages/api/src/index.ts`

- [ ] **Step 1: Create the routes file**

```typescript
// packages/api/src/documents/routes.ts
import { Effect } from "effect";
import { Elysia, t } from "elysia";

import { requireSession } from "../require-session";
import * as documentService from "./service";

export const documentRoutes = new Elysia()
    .get(
        "/documents",
        ctx =>
            Effect.runPromise(
                requireSession(ctx).pipe(
                    Effect.flatMap(session =>
                        documentService.listDocuments(session.user.id, ctx.query.codebaseId)
                    )
                )
            ),
        {
            query: t.Object({
                codebaseId: t.Optional(t.String())
            })
        }
    )
    .get(
        "/documents/:id",
        ctx =>
            Effect.runPromise(
                requireSession(ctx).pipe(
                    Effect.flatMap(() => documentService.getDocument(ctx.params.id))
                )
            ),
        {
            params: t.Object({ id: t.String() })
        }
    )
    .post(
        "/documents/import",
        ctx =>
            Effect.runPromise(
                requireSession(ctx).pipe(
                    Effect.flatMap(session =>
                        documentService.importDocument(
                            session.user.id,
                            ctx.body.sourcePath,
                            ctx.body.content,
                            ctx.body.codebaseId
                        )
                    )
                )
            ),
        {
            body: t.Object({
                sourcePath: t.String({ maxLength: 500 }),
                content: t.String({ maxLength: 200000 }),
                codebaseId: t.Optional(t.String())
            })
        }
    )
    .post(
        "/documents/import-dir",
        ctx =>
            Effect.runPromise(
                requireSession(ctx).pipe(
                    Effect.flatMap(session =>
                        Effect.forEach(ctx.body.files, file =>
                            documentService.importDocument(
                                session.user.id,
                                file.sourcePath,
                                file.content,
                                ctx.body.codebaseId
                            )
                        )
                    )
                )
            ),
        {
            body: t.Object({
                files: t.Array(
                    t.Object({
                        sourcePath: t.String({ maxLength: 500 }),
                        content: t.String({ maxLength: 200000 })
                    }),
                    { maxItems: 200 }
                ),
                codebaseId: t.Optional(t.String())
            })
        }
    )
    .post(
        "/documents/:id/sync",
        ctx =>
            Effect.runPromise(
                requireSession(ctx).pipe(
                    Effect.flatMap(session =>
                        documentService.syncDocument(
                            ctx.params.id,
                            ctx.body.content,
                            session.user.id,
                            ctx.body.codebaseId
                        )
                    )
                )
            ),
        {
            params: t.Object({ id: t.String() }),
            body: t.Object({
                content: t.String({ maxLength: 200000 }),
                codebaseId: t.Optional(t.String())
            })
        }
    )
    .get(
        "/documents/:id/render",
        ctx =>
            Effect.runPromise(
                requireSession(ctx).pipe(
                    Effect.flatMap(() => documentService.renderDocument(ctx.params.id))
                )
            ),
        {
            params: t.Object({ id: t.String() })
        }
    )
    .delete(
        "/documents/:id",
        ctx =>
            Effect.runPromise(
                requireSession(ctx).pipe(
                    Effect.flatMap(() => documentService.removeDocument(ctx.params.id))
                )
            ),
        {
            params: t.Object({ id: t.String() })
        }
    );
```

- [ ] **Step 2: Mount routes in the API index**

In `packages/api/src/index.ts`, add import:
```typescript
import { documentRoutes } from "./documents/routes";
```

Add `.use(documentRoutes)` alongside the other `.use()` calls.

- [ ] **Step 3: Verify the server starts**

Run: `cd apps/server && pnpm dev`
Expected: Server starts without errors. Check `http://localhost:3000/docs` shows the new endpoints.

- [ ] **Step 4: Commit**

```bash
git add packages/api/src/documents/routes.ts packages/api/src/index.ts
git commit -m "feat: add document API routes (list, detail, import, sync, render, delete)"
```

---

### Task 6: Service Tests

**Files:**
- Create: `packages/api/src/documents/service.test.ts`

- [ ] **Step 1: Write integration tests for import and sync**

```typescript
// packages/api/src/documents/service.test.ts
import { describe, expect, it } from "vitest";

import { splitMarkdown } from "./split-markdown";

describe("document import flow", () => {
    it("splits a multi-section doc into ordered sections", () => {
        const md = `---
tags:
  - guide
---

# Getting Started

Welcome to the guide.

## Installation

Run npm install.

## Configuration

Edit config.json.

## Usage

Import and call the function.
`;
        const result = splitMarkdown(md, "docs/getting-started.md");

        expect(result.title).toBe("Getting Started");
        expect(result.tags).toEqual(["guide"]);
        expect(result.sections).toHaveLength(4); // preamble + 3 H2s
        expect(result.sections[0]!.title).toBe("Getting Started \u2014 Introduction");
        expect(result.sections[0]!.order).toBe(0);
        expect(result.sections[1]!.title).toBe("Installation");
        expect(result.sections[1]!.order).toBe(1);
        expect(result.sections[2]!.title).toBe("Configuration");
        expect(result.sections[2]!.order).toBe(2);
        expect(result.sections[3]!.title).toBe("Usage");
        expect(result.sections[3]!.order).toBe(3);
    });

    it("handles markdown with only frontmatter and content", () => {
        const md = `---
title: Quick Reference
tags:
  - reference
description: A quick ref card
---

Just a simple reference document with no sections.
`;
        const result = splitMarkdown(md, "ref.md");

        expect(result.title).toBe("Quick Reference");
        expect(result.description).toBe("A quick ref card");
        expect(result.sections).toHaveLength(1);
        expect(result.sections[0]!.content).toBe("Just a simple reference document with no sections.");
    });

    it("handles empty file gracefully", () => {
        const result = splitMarkdown("", "empty.md");
        expect(result.title).toBe("empty");
        expect(result.sections).toHaveLength(0);
    });
});
```

- [ ] **Step 2: Run tests**

Run: `cd packages/api && pnpm vitest run src/documents/`
Expected: All tests PASS

- [ ] **Step 3: Commit**

```bash
git add packages/api/src/documents/service.test.ts
git commit -m "test: add document import flow tests"
```

---

### Task 7: CLI Docs Command

**Files:**
- Create: `apps/cli/src/commands/docs.ts`
- Modify: `apps/cli/src/index.ts`

- [ ] **Step 1: Create the docs CLI command**

```typescript
// apps/cli/src/commands/docs.ts
import { readdirSync, readFileSync, statSync } from "node:fs";
import { join, relative } from "node:path";

import { Command } from "commander";

import { formatSuccess } from "../lib/colors";
import { resolveCodebaseId } from "../lib/detect-codebase";
import { output, outputError } from "../lib/output";
import { getServerUrl } from "../lib/store";

async function apiRequest(method: string, path: string, body?: unknown) {
    const serverUrl = getServerUrl();
    if (!serverUrl) {
        outputError("No server URL configured. Run: fubbik init");
        process.exit(1);
    }
    const res = await fetch(`${serverUrl}/api${path}`, {
        method,
        headers: { "Content-Type": "application/json" },
        body: body ? JSON.stringify(body) : undefined
    });
    if (!res.ok) {
        const err = await res.json().catch(() => ({ message: res.statusText }));
        throw new Error(err.message ?? `HTTP ${res.status}`);
    }
    return res.json();
}

function collectMarkdownFiles(dir: string): string[] {
    const files: string[] = [];
    for (const entry of readdirSync(dir, { withFileTypes: true })) {
        const fullPath = join(dir, entry.name);
        if (entry.isDirectory()) {
            files.push(...collectMarkdownFiles(fullPath));
        } else if (entry.isFile() && entry.name.endsWith(".md")) {
            files.push(fullPath);
        }
    }
    return files;
}

export const docsCommand = new Command("docs")
    .description("Manage imported documents");

docsCommand
    .command("import")
    .description("Import a markdown file as a document")
    .argument("<path>", "path to a .md file")
    .option("--codebase <name>", "codebase name")
    .action(async (inputPath: string, opts: { codebase?: string }) => {
        try {
            const stat = statSync(inputPath);
            if (!stat.isFile()) {
                outputError("Path must be a .md file");
                process.exit(1);
            }

            const codebaseId = opts.codebase ? await resolveCodebaseId(opts.codebase) : undefined;
            const content = readFileSync(inputPath, "utf-8");
            const sourcePath = relative(process.cwd(), inputPath);

            const result = await apiRequest("POST", "/documents/import", {
                sourcePath,
                content,
                codebaseId
            });

            output(formatSuccess(`Imported "${result.document.title}" — ${result.created} chunks created (${result.status})`));
        } catch (err: any) {
            outputError(err.message);
            process.exit(1);
        }
    });

docsCommand
    .command("import-dir")
    .description("Import a directory of markdown files as documents")
    .argument("<dir>", "path to directory")
    .option("--codebase <name>", "codebase name")
    .action(async (dir: string, opts: { codebase?: string }) => {
        try {
            const stat = statSync(dir);
            if (!stat.isDirectory()) {
                outputError("Path must be a directory");
                process.exit(1);
            }

            const codebaseId = opts.codebase ? await resolveCodebaseId(opts.codebase) : undefined;
            const mdFiles = collectMarkdownFiles(dir);

            if (mdFiles.length === 0) {
                outputError("No .md files found");
                process.exit(1);
            }

            const files = mdFiles.map(f => ({
                sourcePath: relative(process.cwd(), f),
                content: readFileSync(f, "utf-8")
            }));

            const results = await apiRequest("POST", "/documents/import-dir", {
                files,
                codebaseId
            });

            const total = results.reduce((sum: number, r: any) => sum + r.created, 0);
            output(formatSuccess(`Imported ${results.length} documents — ${total} chunks created`));
        } catch (err: any) {
            outputError(err.message);
            process.exit(1);
        }
    });

docsCommand
    .command("list")
    .description("List imported documents")
    .option("--codebase <name>", "codebase name")
    .action(async (opts: { codebase?: string }) => {
        try {
            const codebaseId = opts.codebase ? await resolveCodebaseId(opts.codebase) : undefined;
            const query = codebaseId ? `?codebaseId=${codebaseId}` : "";
            const docs = await apiRequest("GET", `/documents${query}`);

            if (docs.length === 0) {
                output("No documents found.");
                return;
            }

            for (const doc of docs) {
                output(`${doc.id}  ${doc.title}  (${doc.chunkCount} chunks)  ${doc.sourcePath}`);
            }
        } catch (err: any) {
            outputError(err.message);
            process.exit(1);
        }
    });

docsCommand
    .command("show")
    .description("Show a document with its chunks")
    .argument("<id>", "document ID")
    .action(async (id: string) => {
        try {
            const doc = await apiRequest("GET", `/documents/${id}`);
            output(`# ${doc.title}`);
            output(`Source: ${doc.sourcePath}`);
            output(`Chunks: ${doc.chunks.length}`);
            output("");
            for (const chunk of doc.chunks) {
                output(`  ${chunk.documentOrder}. ${chunk.title} (${chunk.id})`);
            }
        } catch (err: any) {
            outputError(err.message);
            process.exit(1);
        }
    });

docsCommand
    .command("sync")
    .description("Re-import changed documents from disk")
    .argument("[id]", "document ID (omit to sync all)")
    .option("--codebase <name>", "codebase name")
    .action(async (id: string | undefined, opts: { codebase?: string }) => {
        try {
            if (id) {
                // Sync single document
                const doc = await apiRequest("GET", `/documents/${id}`);
                const content = readFileSync(doc.sourcePath, "utf-8");
                const result = await apiRequest("POST", `/documents/${id}/sync`, {
                    content,
                    codebaseId: doc.codebaseId
                });
                output(formatSuccess(`Synced "${result.document.title}" — ${result.created} new, ${result.updated} updated (${result.status})`));
            } else {
                // Sync all documents for codebase
                const codebaseId = opts.codebase ? await resolveCodebaseId(opts.codebase) : undefined;
                const query = codebaseId ? `?codebaseId=${codebaseId}` : "";
                const docs = await apiRequest("GET", `/documents${query}`);

                let totalCreated = 0;
                let totalUpdated = 0;
                let synced = 0;

                for (const doc of docs) {
                    try {
                        const content = readFileSync(doc.sourcePath, "utf-8");
                        const result = await apiRequest("POST", `/documents/${doc.id}/sync`, {
                            content,
                            codebaseId: doc.codebaseId
                        });
                        if (result.status !== "unchanged") {
                            synced++;
                            totalCreated += result.created;
                            totalUpdated += result.updated;
                        }
                    } catch {
                        outputError(`  Skipped ${doc.sourcePath} (file not found or error)`);
                    }
                }

                output(formatSuccess(`Synced ${synced}/${docs.length} documents — ${totalCreated} new, ${totalUpdated} updated`));
            }
        } catch (err: any) {
            outputError(err.message);
            process.exit(1);
        }
    });

docsCommand
    .command("render")
    .description("Output reconstructed markdown to stdout")
    .argument("<id>", "document ID")
    .action(async (id: string) => {
        try {
            const result = await apiRequest("GET", `/documents/${id}/render`);
            output(result.markdown);
        } catch (err: any) {
            outputError(err.message);
            process.exit(1);
        }
    });
```

- [ ] **Step 2: Register the docs command in CLI index**

In `apps/cli/src/index.ts`, add import:
```typescript
import { docsCommand } from "./commands/docs";
```

Add after the other `addCommand` calls:
```typescript
program.addCommand(docsCommand);
```

- [ ] **Step 3: Verify CLI help**

Run: `cd apps/cli && pnpm tsx src/index.ts docs --help`
Expected: Shows docs subcommands (import, import-dir, list, show, sync, render)

- [ ] **Step 4: Commit**

```bash
git add apps/cli/src/commands/docs.ts apps/cli/src/index.ts
git commit -m "feat(cli): add docs subcommand group (import, sync, render, list)"
```

---

### Task 8: Web UI — Documents Tab on /docs Page

**Files:**
- Create: `apps/web/src/features/documents/document-browser.tsx`
- Modify: `apps/web/src/routes/docs.tsx`

- [ ] **Step 1: Create the document browser component**

```tsx
// apps/web/src/features/documents/document-browser.tsx
import { useQuery } from "@tanstack/react-query";
import { Link } from "@tanstack/react-router";
import { Book, ChevronRight, FileText, FolderOpen, Pencil, Search } from "lucide-react";
import { useMemo, useState } from "react";

import { Badge } from "@/components/ui/badge";
import { PageEmpty, PageLoading } from "@/components/ui/page";
import { useActiveCodebase } from "@/features/codebases/use-active-codebase";
import { api } from "@/utils/api";
import { unwrapEden } from "@/utils/eden";

interface Document {
    id: string;
    title: string;
    sourcePath: string;
    description: string | null;
    chunkCount: number;
    updatedAt: string;
}

interface DocumentChunk {
    id: string;
    title: string;
    content: string;
    documentOrder: number;
}

interface DocumentDetail {
    id: string;
    title: string;
    sourcePath: string;
    description: string | null;
    chunks: DocumentChunk[];
}

function buildFolderTree(docs: Document[]) {
    const tree: Record<string, Document[]> = {};
    for (const doc of docs) {
        const parts = doc.sourcePath.split("/");
        parts.pop(); // remove filename
        const folder = parts.length > 0 ? parts.join("/") : ".";
        if (!tree[folder]) tree[folder] = [];
        tree[folder]!.push(doc);
    }
    return Object.entries(tree).sort(([a], [b]) => a.localeCompare(b));
}

function MarkdownContent({ content }: { content: string }) {
    const html = content
        .replace(/^### (.+)$/gm, '<h3 class="text-base font-semibold mt-6 mb-2">$1</h3>')
        .replace(/^#### (.+)$/gm, '<h4 class="text-sm font-semibold mt-4 mb-1">$1</h4>')
        .replace(/\*\*(.+?)\*\*/g, "<strong>$1</strong>")
        .replace(/`([^`]+)`/g, '<code class="bg-muted px-1.5 py-0.5 rounded text-[13px] font-mono">$1</code>')
        .replace(/^```(\w*)\n([\s\S]*?)```$/gm, (_m, _lang, code) =>
            `<pre class="bg-muted/50 border rounded-lg p-4 text-[13px] font-mono overflow-x-auto my-3 leading-relaxed"><code>${code.trim()}</code></pre>`
        )
        .replace(/^- (.+)$/gm, '<li class="text-sm ml-4 list-disc mb-1">$1</li>')
        .replace(/^\d+\. (.+)$/gm, '<li class="text-sm ml-4 list-decimal mb-1">$1</li>')
        .replace(/\n{2,}/g, '<div class="h-3"></div>');

    return (
        <div
            className="text-foreground/90 text-sm leading-relaxed"
            dangerouslySetInnerHTML={{ __html: html }}
        />
    );
}

export function DocumentBrowser() {
    const { codebaseId } = useActiveCodebase();
    const [selectedDocId, setSelectedDocId] = useState<string | null>(null);
    const [search, setSearch] = useState("");

    const docsQuery = useQuery({
        queryKey: ["documents", codebaseId],
        queryFn: async () => {
            try {
                return unwrapEden(
                    await api.api.documents.get({
                        query: codebaseId ? { codebaseId } : {}
                    })
                ) as Document[];
            } catch {
                return [];
            }
        }
    });

    const docDetailQuery = useQuery({
        queryKey: ["document", selectedDocId],
        queryFn: async () => {
            if (!selectedDocId) return null;
            try {
                return unwrapEden(
                    await api.api.documents({ id: selectedDocId }).get()
                ) as DocumentDetail;
            } catch {
                return null;
            }
        },
        enabled: !!selectedDocId
    });

    const docs = docsQuery.data ?? [];
    const filteredDocs = search
        ? docs.filter(d => d.title.toLowerCase().includes(search.toLowerCase()) || d.sourcePath.toLowerCase().includes(search.toLowerCase()))
        : docs;
    const folderTree = useMemo(() => buildFolderTree(filteredDocs), [filteredDocs]);
    const detail = docDetailQuery.data;

    if (docsQuery.isLoading) return <PageLoading count={5} />;

    if (docs.length === 0) {
        return (
            <PageEmpty
                icon={Book}
                title="No documents imported"
                description="Import markdown files with the CLI: fubbik docs import <path>"
            />
        );
    }

    return (
        <div className="grid gap-8 lg:grid-cols-[260px_1fr]">
            {/* Sidebar */}
            <nav className="hidden lg:block">
                <div className="sticky top-24 space-y-4">
                    <div className="relative">
                        <Search className="text-muted-foreground absolute left-2.5 top-2.5 size-4" />
                        <input
                            type="text"
                            placeholder="Filter documents..."
                            value={search}
                            onChange={e => setSearch(e.target.value)}
                            className="bg-muted/50 border-border w-full rounded-md border py-2 pl-9 pr-3 text-sm"
                        />
                    </div>

                    <div className="space-y-3">
                        {folderTree.map(([folder, folderDocs]) => (
                            <div key={folder}>
                                <div className="text-muted-foreground mb-1 flex items-center gap-1.5 text-xs font-medium">
                                    <FolderOpen className="size-3.5" />
                                    {folder}
                                </div>
                                <div className="space-y-0.5">
                                    {folderDocs.map(doc => (
                                        <button
                                            key={doc.id}
                                            onClick={() => setSelectedDocId(doc.id)}
                                            className={`flex w-full items-center gap-2 rounded-md px-3 py-2 text-left text-sm transition-colors ${
                                                selectedDocId === doc.id
                                                    ? "bg-muted text-foreground font-medium"
                                                    : "text-muted-foreground hover:text-foreground hover:bg-muted/50"
                                            }`}
                                        >
                                            <FileText className="size-3.5 shrink-0" />
                                            <span className="truncate">{doc.title}</span>
                                            <Badge variant="secondary" className="ml-auto shrink-0 text-[10px]">
                                                {doc.chunkCount}
                                            </Badge>
                                        </button>
                                    ))}
                                </div>
                            </div>
                        ))}
                    </div>
                </div>
            </nav>

            {/* Mobile document selector */}
            <div className="mb-4 flex gap-2 overflow-x-auto lg:hidden">
                {docs.map(doc => (
                    <button
                        key={doc.id}
                        onClick={() => setSelectedDocId(doc.id)}
                        className={`shrink-0 rounded-full px-3 py-1 text-xs font-medium transition-colors ${
                            selectedDocId === doc.id
                                ? "bg-foreground text-background"
                                : "bg-muted text-muted-foreground"
                        }`}
                    >
                        {doc.title}
                    </button>
                ))}
            </div>

            {/* Main content */}
            <div className="min-w-0">
                {!selectedDocId ? (
                    <div className="text-muted-foreground flex flex-col items-center gap-3 py-16">
                        <Book className="size-10 opacity-30" />
                        <p className="text-sm">Select a document from the sidebar to read it.</p>
                    </div>
                ) : docDetailQuery.isLoading ? (
                    <PageLoading count={3} />
                ) : !detail ? (
                    <p className="text-muted-foreground text-sm">Document not found.</p>
                ) : (
                    <article>
                        <div className="mb-6">
                            <h2 className="text-2xl font-bold tracking-tight">{detail.title}</h2>
                            <p className="text-muted-foreground mt-1 text-xs font-mono">{detail.sourcePath}</p>
                        </div>

                        <div className="space-y-8">
                            {detail.chunks.map(chunk => (
                                <section key={chunk.id} id={chunk.title.toLowerCase().replace(/\s+/g, "-")}>
                                    <div className="group mb-3 flex items-center gap-2">
                                        {!chunk.title.endsWith("\u2014 Introduction") && (
                                            <h3 className="text-lg font-semibold">{chunk.title}</h3>
                                        )}
                                        <Link
                                            to="/chunks/$chunkId/edit"
                                            params={{ chunkId: chunk.id }}
                                            className="text-muted-foreground hover:text-foreground opacity-0 transition-opacity group-hover:opacity-100"
                                            title="Edit this section"
                                        >
                                            <Pencil className="size-3.5" />
                                        </Link>
                                    </div>
                                    <MarkdownContent content={chunk.content} />
                                </section>
                            ))}
                        </div>

                        {/* Navigation anchors */}
                        {detail.chunks.length > 3 && (
                            <nav className="border-border mt-8 border-t pt-4">
                                <p className="text-muted-foreground mb-2 text-xs font-medium">On this page</p>
                                <div className="space-y-1">
                                    {detail.chunks
                                        .filter(c => !c.title.endsWith("\u2014 Introduction"))
                                        .map(c => (
                                            <a
                                                key={c.id}
                                                href={`#${c.title.toLowerCase().replace(/\s+/g, "-")}`}
                                                className="text-muted-foreground hover:text-foreground block text-sm"
                                            >
                                                {c.title}
                                            </a>
                                        ))}
                                </div>
                            </nav>
                        )}
                    </article>
                )}
            </div>
        </div>
    );
}
```

- [ ] **Step 2: Add Documents tab to the existing /docs route**

In `apps/web/src/routes/docs.tsx`, add the import at the top:
```typescript
import { DocumentBrowser } from "@/features/documents/document-browser";
```

Add "documents" to the tab state type:
```typescript
const [tab, setTab] = useState<"guide" | "dev" | "documents" | "api">(
    search.tab === "documents" ? "documents" : search.tab === "dev" ? "dev" : search.tab === "api" ? "api" : "guide"
);
```

Add the new tab button after the "Developer Docs" button (inside the tabs `<div>`):
```tsx
<button
    onClick={() => setTab("documents")}
    className={`-mb-px border-b-2 px-4 py-2 text-sm font-medium transition-colors ${
        tab === "documents"
            ? "border-foreground text-foreground"
            : "text-muted-foreground hover:text-foreground border-transparent"
    }`}
>
    <span className="flex items-center gap-2">
        <FileText className="size-4" />
        Documents
    </span>
</button>
```

Add the `FileText` import from lucide-react.

Add the content section before the API tab content:
```tsx
{tab === "documents" && <DocumentBrowser />}
```

Update the `useEffect` to handle the "documents" tab:
```typescript
if (search.tab === "documents") {
    setTab("documents");
}
```

- [ ] **Step 3: Verify the page loads**

Run: `pnpm dev` and open `http://localhost:3001/docs?tab=documents`
Expected: The Documents tab shows with the empty state message.

- [ ] **Step 4: Commit**

```bash
git add apps/web/src/features/documents/document-browser.tsx apps/web/src/routes/docs.tsx
git commit -m "feat(web): add document browser tab to /docs page"
```

---

### Task 9: Update chunk repository params for documentId/documentOrder

> **Note:** This task should be done right after Task 3 (before Task 4), since the document service depends on passing `documentId`/`documentOrder` to `createChunkRepo`/`updateChunkRepo`.

**Files:**
- Modify: `packages/db/src/repository/chunk.ts`

- [ ] **Step 1: Ensure createChunk accepts documentId and documentOrder**

In `packages/db/src/repository/chunk.ts`, check the `CreateChunkParams` interface. Add:

```typescript
    documentId?: string;
    documentOrder?: number;
```

And ensure the `createChunk` function passes these through in the `.values()` call.

Similarly, in `UpdateChunkParams` add:
```typescript
    documentOrder?: number;
```

And in the `updateChunk` function's `.set()` call:
```typescript
    ...(params.documentOrder !== undefined && { documentOrder: params.documentOrder }),
```

- [ ] **Step 2: Verify nothing is broken**

Run: `cd packages/api && pnpm vitest run`
Expected: All existing tests still pass.

- [ ] **Step 3: Commit**

```bash
git add packages/db/src/repository/chunk.ts
git commit -m "feat(db): add documentId/documentOrder to chunk create/update params"
```

---

### Task 10: End-to-End Verification

- [ ] **Step 1: Start the dev server**

Run: `pnpm dev`

- [ ] **Step 2: Test import via CLI**

Create a test markdown file and import it:

```bash
cat > /tmp/test-doc.md << 'EOF'
---
tags:
  - test
---

# Test Document

This is the introduction.

## Getting Started

Follow these steps to get started.

## Configuration

Edit the config file.

## Advanced Usage

For power users.
EOF

cd apps/cli && pnpm tsx src/index.ts docs import /tmp/test-doc.md
```

Expected: Success message showing 4 chunks created.

- [ ] **Step 3: Verify list**

Run: `cd apps/cli && pnpm tsx src/index.ts docs list`
Expected: Shows the imported document with chunk count.

- [ ] **Step 4: Verify render**

Run: `cd apps/cli && pnpm tsx src/index.ts docs render <id from step 3>`
Expected: Outputs reconstructed markdown matching original structure.

- [ ] **Step 5: Test in web UI**

Open `http://localhost:3001/docs?tab=documents`
Expected: Document appears in sidebar, clicking shows ordered sections.

- [ ] **Step 6: Test sync**

Modify `/tmp/test-doc.md` (change content of one section), then:
```bash
cd apps/cli && pnpm tsx src/index.ts docs sync <id>
```
Expected: Shows updated count, unchanged sections preserved.

- [ ] **Step 7: Final commit if any fixes needed**

```bash
git add -A && git commit -m "fix: address issues found during e2e verification"
```
