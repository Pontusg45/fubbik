# Query Builder & Graph-Aware Search Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the text-only `/search` page with a hybrid query builder featuring visual filter blocks, structured text input, and graph-aware queries powered by Apache AGE.

**Architecture:** New `saved_query` schema + repository for persistence. New `packages/api/src/search/` module for query execution (dispatches graph clauses to AGE, standard clauses to Drizzle). Text parser converts structured syntax to query clauses. Frontend rewrites `/search` with filter pills, autocomplete, and result list.

**Tech Stack:** TypeScript, Effect, Drizzle ORM, Apache AGE (Cypher), Elysia, React, TanStack Router/Query, shadcn-ui

---

## File Structure

| File | Action | Responsibility |
|------|--------|---------------|
| `packages/db/src/schema/saved-query.ts` | Create | saved_query table definition |
| `packages/db/src/repository/saved-query.ts` | Create | CRUD for saved queries |
| `packages/api/src/search/types.ts` | Create | QueryClause, SearchQuery, SearchResult types |
| `packages/api/src/search/service.ts` | Create | Query execution engine |
| `packages/api/src/search/routes.ts` | Create | POST /search/query, GET /search/autocomplete, saved query CRUD |
| `packages/api/src/search/parser.ts` | Create | Text syntax → QueryClause[] parser |
| `packages/api/src/index.ts` | Modify | Register searchRoutes |
| `packages/db/src/schema/index.ts` | Modify | Export saved-query schema |
| `apps/web/src/features/search/query-types.ts` | Create | Shared types for frontend |
| `apps/web/src/features/search/use-query-builder.ts` | Create | State management hook |
| `apps/web/src/features/search/query-input.tsx` | Create | Text input with autocomplete |
| `apps/web/src/features/search/filter-pills.tsx` | Create | Visual filter pill row |
| `apps/web/src/features/search/add-filter-dropdown.tsx` | Create | Categorized filter type picker |
| `apps/web/src/features/search/search-results.tsx` | Create | Result list with graph context |
| `apps/web/src/routes/search.tsx` | Modify | Full rewrite — query builder page |

---

### Task 1: Saved Query Schema and Repository

**Files:**
- Create: `packages/db/src/schema/saved-query.ts`
- Create: `packages/db/src/repository/saved-query.ts`
- Modify: `packages/db/src/schema/index.ts`
- Modify: `packages/db/src/repository/index.ts`

- [ ] **Step 1: Create the saved_query schema**

```typescript
// packages/db/src/schema/saved-query.ts
import { relations } from "drizzle-orm";
import { index, jsonb, pgTable, text, timestamp } from "drizzle-orm/pg-core";

import { user } from "./auth";
import { codebase } from "./codebase";

export const savedQuery = pgTable(
    "saved_query",
    {
        id: text("id").primaryKey(),
        name: text("name").notNull(),
        query: jsonb("query").notNull(),
        userId: text("user_id")
            .notNull()
            .references(() => user.id, { onDelete: "cascade" }),
        codebaseId: text("codebase_id").references(() => codebase.id, { onDelete: "set null" }),
        createdAt: timestamp("created_at").defaultNow().notNull()
    },
    table => [
        index("saved_query_userId_idx").on(table.userId)
    ]
);

export const savedQueryRelations = relations(savedQuery, ({ one }) => ({
    user: one(user, { fields: [savedQuery.userId], references: [user.id] }),
    codebase: one(codebase, { fields: [savedQuery.codebaseId], references: [codebase.id] })
}));
```

- [ ] **Step 2: Export from schema index**

Add to `packages/db/src/schema/index.ts`:
```typescript
export * from "./saved-query";
```

- [ ] **Step 3: Create the repository**

```typescript
// packages/db/src/repository/saved-query.ts
import { and, desc, eq } from "drizzle-orm";
import { Effect } from "effect";

import { DatabaseError } from "../errors";
import { db } from "../index";
import { savedQuery } from "../schema/saved-query";

export function listSavedQueries(userId: string, codebaseId?: string) {
    return Effect.tryPromise({
        try: () => {
            const conditions = [eq(savedQuery.userId, userId)];
            if (codebaseId) conditions.push(eq(savedQuery.codebaseId, codebaseId));
            return db
                .select()
                .from(savedQuery)
                .where(and(...conditions))
                .orderBy(desc(savedQuery.createdAt));
        },
        catch: cause => new DatabaseError({ cause })
    });
}

export function createSavedQuery(params: {
    id: string;
    name: string;
    query: unknown;
    userId: string;
    codebaseId?: string;
}) {
    return Effect.tryPromise({
        try: async () => {
            const [created] = await db.insert(savedQuery).values(params).returning();
            return created;
        },
        catch: cause => new DatabaseError({ cause })
    });
}

export function deleteSavedQuery(id: string, userId: string) {
    return Effect.tryPromise({
        try: async () => {
            const [deleted] = await db
                .delete(savedQuery)
                .where(and(eq(savedQuery.id, id), eq(savedQuery.userId, userId)))
                .returning();
            return deleted ?? null;
        },
        catch: cause => new DatabaseError({ cause })
    });
}
```

- [ ] **Step 4: Export from repository index**

Add to `packages/db/src/repository/index.ts`:
```typescript
export * from "./saved-query";
```

- [ ] **Step 5: Push schema**

Run: `pnpm db:push`

Expected: `[✓] Changes applied` — creates the `saved_query` table.

- [ ] **Step 6: Verify type-check passes**

Run: `pnpm --filter @fubbik/db run check-types 2>&1 | grep saved-query`

Expected: No errors.

- [ ] **Step 7: Commit**

```bash
git add packages/db/src/schema/saved-query.ts packages/db/src/repository/saved-query.ts packages/db/src/schema/index.ts packages/db/src/repository/index.ts
git commit -m "feat(search): add saved_query schema and repository"
```

---

### Task 2: Search Types and Query Execution Service

**Files:**
- Create: `packages/api/src/search/types.ts`
- Create: `packages/api/src/search/service.ts`

**Context:** The service receives a `SearchQuery`, dispatches graph clauses to AGE, builds Drizzle filters for standard clauses, and combines results. This is the core engine.

- [ ] **Step 1: Create shared types**

```typescript
// packages/api/src/search/types.ts

export interface QueryClause {
    field: string;
    operator: string;
    value: string;
    params?: Record<string, string>;
    negate?: boolean;
}

export interface SearchQuery {
    clauses: QueryClause[];
    join: "and" | "or";
    sort?: "relevance" | "newest" | "oldest" | "updated";
    limit?: number;
    offset?: number;
    codebaseId?: string;
}

export interface GraphContext {
    hopDistance?: number;
    pathPosition?: number;
    matchedRequirement?: string;
}

export interface SearchResultChunk {
    id: string;
    title: string;
    type: string;
    summary: string | null;
    tags: string[];
    connectionCount: number;
    updatedAt: Date;
    graphContext?: GraphContext;
}

export interface SearchResult {
    chunks: SearchResultChunk[];
    total: number;
    graphMeta?: {
        type: "neighborhood" | "path" | "requirement-reach";
        referenceChunk?: string;
        pathChunks?: string[];
    };
}
```

- [ ] **Step 2: Create the service**

```typescript
// packages/api/src/search/service.ts
import {
    listChunks,
    findShortestPath,
    getNeighborhood,
    getChunksAffectedByRequirement,
    getChunkById
} from "@fubbik/db/repository";
import { Effect } from "effect";

import type { QueryClause, SearchQuery, SearchResult, SearchResultChunk, GraphContext } from "./types";

// ── Graph clause execution ──

function executeGraphClauses(clauses: QueryClause[]) {
    return Effect.gen(function* () {
        let graphChunkIds: string[] | null = null;
        let graphMeta: SearchResult["graphMeta"] = undefined;
        const graphContextMap = new Map<string, GraphContext>();

        for (const clause of clauses) {
            if (clause.field === "near") {
                const hops = Number(clause.params?.hops ?? 2);
                const ids = yield* getNeighborhood(clause.value, hops);
                // Add the reference chunk itself
                ids.push(clause.value);
                graphChunkIds = graphChunkIds
                    ? graphChunkIds.filter(id => ids.includes(id))
                    : ids;
                graphMeta = { type: "neighborhood", referenceChunk: clause.value };

                // Compute hop distances (approximate: re-query per result would be expensive)
                // For now, all neighborhood results get hopDistance = hops (max)
                for (const id of ids) {
                    graphContextMap.set(id, { hopDistance: id === clause.value ? 0 : undefined });
                }
            } else if (clause.field === "path") {
                // value format: "fromId->toId" or parsed from params
                const fromId = clause.params?.from ?? clause.value;
                const toId = clause.params?.to ?? "";
                const pathIds = yield* findShortestPath(fromId, toId);
                if (pathIds) {
                    graphChunkIds = graphChunkIds
                        ? graphChunkIds.filter(id => pathIds.includes(id))
                        : pathIds;
                    graphMeta = { type: "path", pathChunks: pathIds, referenceChunk: fromId };
                    for (let i = 0; i < pathIds.length; i++) {
                        graphContextMap.set(pathIds[i], { pathPosition: i });
                    }
                } else {
                    graphChunkIds = [];
                }
            } else if (clause.field === "affected-by") {
                const hops = Number(clause.params?.hops ?? 2);
                const ids = yield* getChunksAffectedByRequirement(clause.value, hops);
                graphChunkIds = graphChunkIds
                    ? graphChunkIds.filter(id => ids.includes(id))
                    : ids;
                graphMeta = { type: "requirement-reach", referenceChunk: clause.value };
                for (const id of ids) {
                    graphContextMap.set(id, { matchedRequirement: clause.value });
                }
            }
        }

        return { graphChunkIds, graphMeta, graphContextMap };
    });
}

// ── Standard clause → listChunks query params ──

function buildStandardQuery(clauses: QueryClause[], codebaseId?: string) {
    const query: Record<string, string> = {};

    if (codebaseId) query.codebaseId = codebaseId;

    for (const clause of clauses) {
        switch (clause.field) {
            case "type":
                query.type = clause.value;
                break;
            case "tag":
                query.tags = clause.value; // comma-separated
                break;
            case "text":
                query.search = clause.value;
                break;
            case "connections":
                query.minConnections = clause.value.replace("+", "");
                break;
            case "updated":
                query.after = clause.value.replace("d", "");
                break;
            case "origin":
                query.origin = clause.value;
                break;
            case "review":
                query.reviewStatus = clause.value;
                break;
            case "codebase":
                query.codebaseId = clause.value;
                break;
        }
    }

    return query;
}

// ── Main search execution ──

export function executeSearch(userId: string, searchQuery: SearchQuery) {
    return Effect.gen(function* () {
        const graphClauses = searchQuery.clauses.filter(c =>
            ["near", "path", "affected-by"].includes(c.field)
        );
        const standardClauses = searchQuery.clauses.filter(c =>
            !["near", "path", "affected-by"].includes(c.field)
        );

        // 1. Execute graph clauses (returns chunk ID set or null)
        const { graphChunkIds, graphMeta, graphContextMap } = yield* executeGraphClauses(graphClauses);

        // 2. If graph returned empty, short-circuit
        if (graphChunkIds !== null && graphChunkIds.length === 0) {
            return { chunks: [], total: 0, graphMeta } satisfies SearchResult;
        }

        // 3. Build standard query
        const query = buildStandardQuery(standardClauses, searchQuery.codebaseId);
        query.limit = String(searchQuery.limit ?? 20);
        query.offset = String(searchQuery.offset ?? 0);

        if (searchQuery.sort === "relevance" && query.search) {
            // Default sort for text search
        } else if (searchQuery.sort === "newest") {
            query.sort = "newest";
        } else if (searchQuery.sort === "oldest") {
            query.sort = "oldest";
        } else if (searchQuery.sort === "updated") {
            query.sort = "updated";
        }

        // 4. Execute standard query
        const result = yield* listChunks({
            userId,
            ...query,
            limit: graphChunkIds ? 1000 : Number(query.limit), // fetch more if we need to intersect
            offset: graphChunkIds ? 0 : Number(query.offset)
        });

        // 5. Intersect with graph results if applicable
        let chunks = result.chunks;
        if (graphChunkIds !== null) {
            const idSet = new Set(graphChunkIds);
            chunks = chunks.filter(c => idSet.has(c.id));
        }

        // 6. Apply pagination if we fetched extra for intersection
        const total = chunks.length;
        if (graphChunkIds !== null) {
            const offset = searchQuery.offset ?? 0;
            const limit = searchQuery.limit ?? 20;
            chunks = chunks.slice(offset, offset + limit);
        }

        // 7. Map to result format with graph context
        const resultChunks: SearchResultChunk[] = chunks.map(c => ({
            id: c.id,
            title: c.title,
            type: c.type,
            summary: c.summary ?? null,
            tags: [], // tags are loaded separately if needed
            connectionCount: 0, // connection count not in listChunks result
            updatedAt: c.updatedAt,
            graphContext: graphContextMap.get(c.id)
        }));

        return { chunks: resultChunks, total, graphMeta } satisfies SearchResult;
    });
}

// ── Autocomplete ──

import { db } from "@fubbik/db";
import { tag } from "@fubbik/db/schema/chunk";
import { chunk } from "@fubbik/db/schema/chunk";
import { requirement } from "@fubbik/db/schema/requirement";
import { ilike, eq, sql } from "drizzle-orm";
import { DatabaseError } from "@fubbik/db/errors";

export function autocomplete(userId: string, field: string, prefix: string) {
    return Effect.tryPromise({
        try: async () => {
            if (field === "tag") {
                const results = await db
                    .select({ name: tag.name })
                    .from(tag)
                    .where(ilike(tag.name, `${prefix}%`))
                    .limit(10);
                return results.map(r => ({ value: r.name, label: r.name }));
            }
            if (field === "chunk") {
                const results = await db
                    .select({ id: chunk.id, title: chunk.title })
                    .from(chunk)
                    .where(ilike(chunk.title, `%${prefix}%`))
                    .limit(10);
                return results.map(r => ({ value: r.id, label: r.title }));
            }
            if (field === "requirement") {
                const results = await db
                    .select({ id: requirement.id, title: requirement.title })
                    .from(requirement)
                    .where(ilike(requirement.title, `%${prefix}%`))
                    .limit(10);
                return results.map(r => ({ value: r.id, label: r.title }));
            }
            return [];
        },
        catch: cause => new DatabaseError({ cause })
    });
}
```

Note: The `tag` import may need adjustment — check where the `tag` table is actually exported from (likely `@fubbik/db/schema/chunk` is wrong; it's probably in the tag schema). Read the actual imports before writing. Same for `chunk` — verify the import path.

- [ ] **Step 3: Verify type-check passes**

Run: `pnpm --filter @fubbik/api run check-types 2>&1 | grep "search/"`

Expected: No errors in `search/types.ts` or `search/service.ts`. Fix import paths if the tag/chunk schema imports are wrong.

- [ ] **Step 4: Commit**

```bash
git add packages/api/src/search/types.ts packages/api/src/search/service.ts
git commit -m "feat(search): add query execution service with graph clause support"
```

---

### Task 3: Text Query Parser

**Files:**
- Create: `packages/api/src/search/parser.ts`

**Context:** Parses structured text syntax like `type:reference tag:api near:"Auth Flow" hops:2 NOT tag:deprecated` into an array of `QueryClause` objects.

- [ ] **Step 1: Create the parser**

```typescript
// packages/api/src/search/parser.ts
import type { QueryClause } from "./types";

/**
 * Parse a structured query string into clauses.
 *
 * Supported syntax:
 *   type:reference           → { field: "type", operator: "is", value: "reference" }
 *   tag:api,auth             → { field: "tag", operator: "any_of", value: "api,auth" }
 *   "search text"            → { field: "text", operator: "contains", value: "search text" }
 *   connections:3+           → { field: "connections", operator: "gte", value: "3" }
 *   updated:30d              → { field: "updated", operator: "within", value: "30" }
 *   origin:ai                → { field: "origin", operator: "is", value: "ai" }
 *   review:draft             → { field: "review", operator: "is", value: "draft" }
 *   near:"Auth Flow" hops:2  → { field: "near", operator: "is", value: "Auth Flow", params: { hops: "2" } }
 *   path:"A"->"B"            → { field: "path", operator: "is", value: "A", params: { from: "A", to: "B" } }
 *   affected-by:"CRUD ops"   → { field: "affected-by", operator: "is", value: "CRUD ops" }
 *   NOT tag:deprecated       → { field: "tag", operator: "is", value: "deprecated", negate: true }
 *   bare text                → { field: "text", operator: "contains", value: "bare text" }
 */
export function parseQueryString(input: string): QueryClause[] {
    const clauses: QueryClause[] = [];
    const tokens = tokenize(input);
    let i = 0;

    while (i < tokens.length) {
        const token = tokens[i];

        // NOT prefix
        if (token.toUpperCase() === "NOT" && i + 1 < tokens.length) {
            const nextClause = parseSingleToken(tokens[i + 1], tokens, i + 1);
            if (nextClause.clause) {
                nextClause.clause.negate = true;
                clauses.push(nextClause.clause);
                i = nextClause.nextIndex + 1;
                continue;
            }
        }

        // hops:N is a param modifier for the previous "near" clause
        if (token.startsWith("hops:") && clauses.length > 0) {
            const lastClause = clauses[clauses.length - 1];
            if (lastClause.field === "near") {
                lastClause.params = { ...lastClause.params, hops: token.slice(5) };
            }
            i++;
            continue;
        }

        const result = parseSingleToken(token, tokens, i);
        if (result.clause) {
            clauses.push(result.clause);
            i = result.nextIndex + 1;
        } else {
            i++;
        }
    }

    return clauses;
}

function parseSingleToken(
    token: string,
    tokens: string[],
    index: number
): { clause: QueryClause | null; nextIndex: number } {
    // field:value patterns
    const colonIndex = token.indexOf(":");
    if (colonIndex > 0) {
        const field = token.slice(0, colonIndex);
        let value = token.slice(colonIndex + 1);

        // Handle quoted values: type:"some value"
        if (value.startsWith('"') && !value.endsWith('"')) {
            // Collect tokens until closing quote
            let fullValue = value.slice(1);
            let j = index + 1;
            while (j < tokens.length) {
                fullValue += " " + tokens[j];
                if (tokens[j].endsWith('"')) {
                    fullValue = fullValue.slice(0, -1);
                    return { clause: makeClause(field, fullValue), nextIndex: j };
                }
                j++;
            }
            return { clause: makeClause(field, fullValue), nextIndex: j - 1 };
        }

        // Strip quotes if fully quoted
        if (value.startsWith('"') && value.endsWith('"')) {
            value = value.slice(1, -1);
        }

        // Path syntax: path:"A"->"B"
        if (field === "path" && value.includes("->")) {
            const [from, to] = value.split("->").map(s => s.replace(/"/g, ""));
            return {
                clause: { field: "path", operator: "is", value: from, params: { from, to } },
                nextIndex: index
            };
        }

        return { clause: makeClause(field, value), nextIndex: index };
    }

    // Bare quoted string: "search text"
    if (token.startsWith('"')) {
        let value = token.slice(1);
        if (token.endsWith('"') && token.length > 1) {
            value = token.slice(1, -1);
            return { clause: { field: "text", operator: "contains", value }, nextIndex: index };
        }
        // Multi-token quoted string
        let j = index + 1;
        while (j < tokens.length) {
            value += " " + tokens[j];
            if (tokens[j].endsWith('"')) {
                value = value.slice(0, -1);
                return { clause: { field: "text", operator: "contains", value }, nextIndex: j };
            }
            j++;
        }
        return { clause: { field: "text", operator: "contains", value }, nextIndex: j - 1 };
    }

    // Bare word: treat as text search
    return { clause: { field: "text", operator: "contains", value: token }, nextIndex: index };
}

function makeClause(field: string, value: string): QueryClause {
    // Detect operators from value
    if (value.endsWith("+")) {
        return { field, operator: "gte", value: value.slice(0, -1) };
    }
    if (value.endsWith("d") && /^\d+d$/.test(value)) {
        return { field, operator: "within", value: value.slice(0, -1) };
    }
    if (value.includes(",")) {
        return { field, operator: "any_of", value };
    }
    return { field, operator: "is", value };
}

function tokenize(input: string): string[] {
    const tokens: string[] = [];
    let current = "";
    let inQuotes = false;

    for (let i = 0; i < input.length; i++) {
        const char = input[i];
        if (char === '"') {
            inQuotes = !inQuotes;
            current += char;
        } else if (char === " " && !inQuotes) {
            if (current.length > 0) {
                tokens.push(current);
                current = "";
            }
        } else {
            current += char;
        }
    }
    if (current.length > 0) tokens.push(current);

    return tokens;
}

/**
 * Convert clauses back to a query string (for syncing pills → text input).
 */
export function clausesToQueryString(clauses: QueryClause[]): string {
    return clauses.map(c => {
        const prefix = c.negate ? "NOT " : "";
        const value = c.value.includes(" ") ? `"${c.value}"` : c.value;

        if (c.field === "text") return `${prefix}${value}`;
        if (c.field === "path" && c.params?.from && c.params?.to) {
            return `${prefix}path:"${c.params.from}"->"${c.params.to}"`;
        }

        let result = `${prefix}${c.field}:${value}`;
        if (c.field === "near" && c.params?.hops) {
            result += ` hops:${c.params.hops}`;
        }
        if (c.field === "connections" && c.operator === "gte") {
            result = `${prefix}${c.field}:${c.value}+`;
        }
        if (c.field === "updated" && c.operator === "within") {
            result = `${prefix}${c.field}:${c.value}d`;
        }
        return result;
    }).join(" ");
}
```

- [ ] **Step 2: Verify type-check passes**

Run: `pnpm --filter @fubbik/api run check-types 2>&1 | grep "search/parser"`

Expected: No errors.

- [ ] **Step 3: Commit**

```bash
git add packages/api/src/search/parser.ts
git commit -m "feat(search): add text query parser with structured syntax support"
```

---

### Task 4: API Routes and Registration

**Files:**
- Create: `packages/api/src/search/routes.ts`
- Modify: `packages/api/src/index.ts`

**Context:** Elysia routes for query execution, autocomplete, and saved query CRUD. Register in the main API.

- [ ] **Step 1: Create the routes**

```typescript
// packages/api/src/search/routes.ts
import { Effect } from "effect";
import { Elysia, t } from "elysia";

import { requireSession } from "../require-session";
import { executeSearch, autocomplete } from "./service";
import { parseQueryString } from "./parser";
import { listSavedQueries, createSavedQuery, deleteSavedQuery } from "@fubbik/db/repository";

const ClauseSchema = t.Object({
    field: t.String(),
    operator: t.String(),
    value: t.String(),
    params: t.Optional(t.Record(t.String(), t.String())),
    negate: t.Optional(t.Boolean())
});

export const searchRoutes = new Elysia()
    .post(
        "/search/query",
        ctx =>
            Effect.runPromise(
                requireSession(ctx).pipe(
                    Effect.flatMap(session =>
                        executeSearch(session.user.id, {
                            clauses: ctx.body.clauses,
                            join: ctx.body.join ?? "and",
                            sort: ctx.body.sort,
                            limit: ctx.body.limit,
                            offset: ctx.body.offset,
                            codebaseId: ctx.body.codebaseId
                        })
                    )
                )
            ),
        {
            body: t.Object({
                clauses: t.Array(ClauseSchema),
                join: t.Optional(t.Union([t.Literal("and"), t.Literal("or")])),
                sort: t.Optional(t.Union([t.Literal("relevance"), t.Literal("newest"), t.Literal("oldest"), t.Literal("updated")])),
                limit: t.Optional(t.Number()),
                offset: t.Optional(t.Number()),
                codebaseId: t.Optional(t.String())
            })
        }
    )
    .get(
        "/search/parse",
        ctx => {
            const clauses = parseQueryString(ctx.query.q ?? "");
            return { clauses };
        },
        {
            query: t.Object({
                q: t.Optional(t.String())
            })
        }
    )
    .get(
        "/search/autocomplete",
        ctx =>
            Effect.runPromise(
                requireSession(ctx).pipe(
                    Effect.flatMap(session =>
                        autocomplete(session.user.id, ctx.query.field, ctx.query.prefix ?? "")
                    )
                )
            ),
        {
            query: t.Object({
                field: t.String(),
                prefix: t.Optional(t.String())
            })
        }
    )
    .get(
        "/search/saved",
        ctx =>
            Effect.runPromise(
                requireSession(ctx).pipe(
                    Effect.flatMap(session =>
                        listSavedQueries(session.user.id, ctx.query.codebaseId)
                    )
                )
            ),
        {
            query: t.Object({
                codebaseId: t.Optional(t.String())
            })
        }
    )
    .post(
        "/search/saved",
        ctx =>
            Effect.runPromise(
                requireSession(ctx).pipe(
                    Effect.flatMap(session =>
                        createSavedQuery({
                            id: crypto.randomUUID(),
                            name: ctx.body.name,
                            query: ctx.body.query,
                            userId: session.user.id,
                            codebaseId: ctx.body.codebaseId
                        })
                    )
                )
            ),
        {
            body: t.Object({
                name: t.String(),
                query: t.Any(),
                codebaseId: t.Optional(t.String())
            })
        }
    )
    .delete(
        "/search/saved/:id",
        ctx =>
            Effect.runPromise(
                requireSession(ctx).pipe(
                    Effect.flatMap(session =>
                        deleteSavedQuery(ctx.params.id, session.user.id)
                    )
                )
            ),
        {
            params: t.Object({
                id: t.String()
            })
        }
    );
```

- [ ] **Step 2: Register in main API**

In `packages/api/src/index.ts`, add import:
```typescript
import { searchRoutes } from "./search/routes";
```

Add `.use(searchRoutes)` after the last `.use(stalenessRoutes)`:
```typescript
    .use(stalenessRoutes)
    .use(searchRoutes);
```

- [ ] **Step 3: Verify type-check passes**

Run: `pnpm --filter @fubbik/api run check-types 2>&1 | grep "search/"`

Expected: No errors. If repository imports fail, check the exact export paths.

- [ ] **Step 4: Commit**

```bash
git add packages/api/src/search/routes.ts packages/api/src/index.ts
git commit -m "feat(search): add query API routes and register in main API"
```

---

### Task 5: Frontend — Query Builder Components

**Files:**
- Create: `apps/web/src/features/search/query-types.ts`
- Create: `apps/web/src/features/search/use-query-builder.ts`
- Create: `apps/web/src/features/search/query-input.tsx`
- Create: `apps/web/src/features/search/filter-pills.tsx`
- Create: `apps/web/src/features/search/add-filter-dropdown.tsx`
- Create: `apps/web/src/features/search/search-results.tsx`

**Context:** These are the building blocks for the search page. The state management hook holds the clauses array and syncs between text input and pills. Each component is focused on one responsibility.

- [ ] **Step 1: Create shared frontend types**

```typescript
// apps/web/src/features/search/query-types.ts
export interface QueryClause {
    field: string;
    operator: string;
    value: string;
    params?: Record<string, string>;
    negate?: boolean;
}

export interface SearchQuery {
    clauses: QueryClause[];
    join: "and" | "or";
    sort?: "relevance" | "newest" | "oldest" | "updated";
}

// Color mapping for filter pill categories
export const FILTER_COLORS: Record<string, string> = {
    type: "bg-indigo-500/15 border-indigo-500/30 text-indigo-400",
    origin: "bg-indigo-500/15 border-indigo-500/30 text-indigo-400",
    review: "bg-indigo-500/15 border-indigo-500/30 text-indigo-400",
    tag: "bg-teal-500/15 border-teal-500/30 text-teal-400",
    near: "bg-amber-500/15 border-amber-500/30 text-amber-400",
    path: "bg-amber-500/15 border-amber-500/30 text-amber-400",
    "affected-by": "bg-amber-500/15 border-amber-500/30 text-amber-400",
    text: "bg-slate-500/15 border-slate-500/30 text-slate-400",
    connections: "bg-slate-500/15 border-slate-500/30 text-slate-400",
    updated: "bg-slate-500/15 border-slate-500/30 text-slate-400",
    codebase: "bg-slate-500/15 border-slate-500/30 text-slate-400",
};

export const GRAPH_FIELDS = ["near", "path", "affected-by"];

export const FILTER_CATEGORIES = [
    {
        label: "Basic",
        fields: [
            { field: "type", label: "Type", description: "Chunk type (note, document, reference, schema, checklist)" },
            { field: "tag", label: "Tag", description: "Filter by tag name" },
            { field: "text", label: "Text search", description: "Full-text search in title and content" },
            { field: "connections", label: "Connections", description: "Minimum connection count" },
            { field: "updated", label: "Updated within", description: "Days since last update" },
            { field: "origin", label: "Origin", description: "Created by human or AI" },
            { field: "review", label: "Review status", description: "Draft or approved" },
        ]
    },
    {
        label: "Graph",
        fields: [
            { field: "near", label: "Neighborhood", description: "Chunks within N hops of a chunk" },
            { field: "path", label: "Path finding", description: "Find connection path between two chunks" },
            { field: "affected-by", label: "Affected by requirement", description: "Chunks linked to a requirement" },
        ]
    }
];
```

- [ ] **Step 2: Create the state management hook**

```typescript
// apps/web/src/features/search/use-query-builder.ts
import { useCallback, useState } from "react";

import type { QueryClause, SearchQuery } from "./query-types";

export function useQueryBuilder(initialClauses: QueryClause[] = []) {
    const [clauses, setClauses] = useState<QueryClause[]>(initialClauses);
    const [join, setJoin] = useState<"and" | "or">("and");
    const [sort, setSort] = useState<SearchQuery["sort"]>("relevance");

    const addClause = useCallback((clause: QueryClause) => {
        setClauses(prev => [...prev, clause]);
    }, []);

    const removeClause = useCallback((index: number) => {
        setClauses(prev => prev.filter((_, i) => i !== index));
    }, []);

    const updateClause = useCallback((index: number, clause: QueryClause) => {
        setClauses(prev => prev.map((c, i) => (i === index ? clause : c)));
    }, []);

    const clearAll = useCallback(() => {
        setClauses([]);
    }, []);

    const loadClauses = useCallback((newClauses: QueryClause[]) => {
        setClauses(newClauses);
    }, []);

    const query: SearchQuery = { clauses, join, sort };

    const hasGraphClauses = clauses.some(c =>
        ["near", "path", "affected-by"].includes(c.field)
    );

    return {
        clauses,
        join,
        sort,
        query,
        hasGraphClauses,
        addClause,
        removeClause,
        updateClause,
        clearAll,
        loadClauses,
        setJoin,
        setSort,
    };
}
```

- [ ] **Step 3: Create the remaining UI components**

Create the following files. Each is a focused React component:

**`apps/web/src/features/search/query-input.tsx`** — text input bar with monospace font. On Enter, parses the text via `GET /api/search/parse?q=...` and calls `loadClauses`. Shows syntax help below. The implementer should:
- Use a controlled input with `onKeyDown` Enter handler
- Call the parse API endpoint to convert text to clauses
- Display the syntax reference line below the input

**`apps/web/src/features/search/filter-pills.tsx`** — renders clauses as color-coded pills using `FILTER_COLORS`. Each pill shows field name, operator word, value, and × button. AND/OR toggle between pills. The implementer should:
- Map over `clauses` array
- Render each as a pill with the appropriate color class
- Show join toggle between pills
- Call `removeClause(index)` on × click

**`apps/web/src/features/search/add-filter-dropdown.tsx`** — dropdown triggered by "+ Add filter" button. Shows `FILTER_CATEGORIES` grouped by Basic/Graph. Clicking a field opens a sub-form for entering the value (text input for most, chunk autocomplete for `near`/`path`). The implementer should:
- Use the existing `DropdownMenu` component from `@/components/ui/dropdown-menu`
- Group fields by category
- On field selection, show a value input (inline or via a small form)
- Call `addClause` with the completed clause

**`apps/web/src/features/search/search-results.tsx`** — renders the result list. Each chunk shows title, type badge, tags, connection count, updated time, and optional graph context (hop distance, path position, requirement name). The implementer should:
- Accept `chunks` array and `graphMeta` as props
- Show result count with "graph filtered" badge when graphMeta is present
- Render each chunk as a `Link` to `/chunks/$chunkId`
- Show `graphContext` metadata in an amber label when present

For each of these files, the implementer should follow the patterns in existing feature components (e.g., `apps/web/src/features/chunks/chunk-filters-popover.tsx` for the dropdown pattern, existing chunk list pages for the result rendering pattern). Use shadcn-ui components and Tailwind classes matching the project's dark theme.

- [ ] **Step 4: Verify type-check passes**

Run: `pnpm --filter web run check-types 2>&1 | grep "features/search"`

Expected: No errors.

- [ ] **Step 5: Commit**

```bash
git add apps/web/src/features/search/
git commit -m "feat(search): add query builder frontend components"
```

---

### Task 6: Rewrite Search Page

**Files:**
- Modify: `apps/web/src/routes/search.tsx`

**Context:** Full rewrite of the search page to use the query builder components from Task 5. This composes QueryInput, FilterPills, AddFilterDropdown, and SearchResults into the final page.

- [ ] **Step 1: Rewrite search.tsx**

Replace the full file. The page should:

1. **Route setup:** Keep `createFileRoute("/search")` with `validateSearch` that reads `q` from URL params. `beforeLoad` authenticates.

2. **State:** Use `useQueryBuilder()` hook. On mount, if `q` param exists, parse it via the API and load clauses.

3. **Query execution:** Use `useMutation` for `POST /api/search/query` (not useQuery, since it's a POST). Trigger on clause changes (debounced).

4. **Saved queries:** Use `useQuery` for `GET /api/search/saved`. Show as a dropdown next to the search bar.

5. **Layout (top to bottom):**
   - Page header: "Search" title + saved queries dropdown + clear button
   - `<QueryInput>` — text input bar
   - `<FilterPills>` — visual pill row with `<AddFilterDropdown>` at the end
   - Graph indicator bar (when `hasGraphClauses`)
   - `<SearchResults>` — result list

6. **URL sync:** When clauses change, update the `q` search param with `clausesToQueryString`. This makes queries bookmarkable.

The implementer should read the existing `search.tsx` for the route/auth pattern, then compose the new components. Follow the project's page layout patterns (container, max-width, padding).

- [ ] **Step 2: Verify the page renders**

Run: `pnpm dev` and navigate to `http://localhost:3001/search`

Expected: Query builder renders with text input, empty pill row, "+ Add filter" button, and empty results.

- [ ] **Step 3: Smoke test**

1. Type `type:reference` in the text input, press Enter → should create an indigo pill and show results
2. Click "+ Add filter" → should show categorized dropdown
3. Add a "near" filter with a chunk name → should show amber graph pill and filtered results
4. Click "Save query" → should persist and appear in saved queries dropdown
5. Clear all → should reset

- [ ] **Step 4: Commit**

```bash
git add apps/web/src/routes/search.tsx
git commit -m "feat(search): rewrite search page with query builder and graph-aware filters"
```
