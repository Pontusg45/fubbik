# MCP Review Workflow — Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a structured AI implementer / human reviewer workflow where the AI records its context and assumptions during implementation, Fubbik generates a review brief, and the developer reviews against that brief with interactive tools.

**Architecture:** New `implementation_session` schema with related tables for chunk refs, assumptions, and requirement refs. New API routes under `/api/sessions`. New MCP tools in `packages/mcp/src/session-tools.ts`. New web pages at `/reviews` and `/reviews/:sessionId`. Integration with existing requirements, chunks, and knowledge health.

**Tech Stack:** Drizzle ORM, Effect, Elysia, @modelcontextprotocol/sdk, TanStack Start, React Query, Tailwind/shadcn-ui

---

### Task 1: Schema — implementation session tables

Create the schema for sessions, chunk refs, assumptions, and requirement refs.

**Files:**
- Create: `packages/db/src/schema/implementation-session.ts`
- Modify: `packages/db/src/schema/index.ts`

- [ ] **Step 1: Create the session schema**

Create `packages/db/src/schema/implementation-session.ts`:

```typescript
import { boolean, index, jsonb, pgTable, primaryKey, text, timestamp } from "drizzle-orm/pg-core";
import { user } from "./auth";
import { chunk } from "./chunk";
import { codebase } from "./codebase";
import { requirement } from "./requirement";

export const implementationSession = pgTable(
    "implementation_session",
    {
        id: text("id").primaryKey(),
        title: text("title").notNull(),
        status: text("status").notNull().default("in_progress"),
        userId: text("user_id")
            .notNull()
            .references(() => user.id, { onDelete: "cascade" }),
        codebaseId: text("codebase_id").references(() => codebase.id, { onDelete: "set null" }),
        prUrl: text("pr_url"),
        reviewBrief: text("review_brief"),
        createdAt: timestamp("created_at").defaultNow().notNull(),
        updatedAt: timestamp("updated_at")
            .defaultNow()
            .$onUpdate(() => new Date())
            .notNull(),
        completedAt: timestamp("completed_at"),
        reviewedAt: timestamp("reviewed_at")
    },
    table => [
        index("session_userId_idx").on(table.userId),
        index("session_codebaseId_idx").on(table.codebaseId),
        index("session_status_idx").on(table.status)
    ]
);

export const sessionChunkRef = pgTable(
    "session_chunk_ref",
    {
        sessionId: text("session_id")
            .notNull()
            .references(() => implementationSession.id, { onDelete: "cascade" }),
        chunkId: text("chunk_id")
            .notNull()
            .references(() => chunk.id, { onDelete: "cascade" }),
        reason: text("reason").notNull()
    },
    table => [primaryKey({ columns: [table.sessionId, table.chunkId] })]
);

export const sessionAssumption = pgTable(
    "session_assumption",
    {
        id: text("id").primaryKey(),
        sessionId: text("session_id")
            .notNull()
            .references(() => implementationSession.id, { onDelete: "cascade" }),
        description: text("description").notNull(),
        resolved: boolean("resolved").notNull().default(false),
        resolution: text("resolution")
    },
    table => [index("assumption_sessionId_idx").on(table.sessionId)]
);

export const sessionRequirementRef = pgTable(
    "session_requirement_ref",
    {
        sessionId: text("session_id")
            .notNull()
            .references(() => implementationSession.id, { onDelete: "cascade" }),
        requirementId: text("requirement_id")
            .notNull()
            .references(() => requirement.id, { onDelete: "cascade" }),
        stepsAddressed: jsonb("steps_addressed").notNull().default([])
    },
    table => [primaryKey({ columns: [table.sessionId, table.requirementId] })]
);
```

Add Drizzle relations (consistent with existing schema pattern):

```typescript
import { relations } from "drizzle-orm";

export const implementationSessionRelations = relations(implementationSession, ({ one, many }) => ({
    user: one(user, { fields: [implementationSession.userId], references: [user.id] }),
    codebase: one(codebase, { fields: [implementationSession.codebaseId], references: [codebase.id] }),
    chunkRefs: many(sessionChunkRef),
    assumptions: many(sessionAssumption),
    requirementRefs: many(sessionRequirementRef)
}));

export const sessionChunkRefRelations = relations(sessionChunkRef, ({ one }) => ({
    session: one(implementationSession, { fields: [sessionChunkRef.sessionId], references: [implementationSession.id] }),
    chunk: one(chunk, { fields: [sessionChunkRef.chunkId], references: [chunk.id] })
}));

export const sessionAssumptionRelations = relations(sessionAssumption, ({ one }) => ({
    session: one(implementationSession, { fields: [sessionAssumption.sessionId], references: [implementationSession.id] })
}));

export const sessionRequirementRefRelations = relations(sessionRequirementRef, ({ one }) => ({
    session: one(implementationSession, { fields: [sessionRequirementRef.sessionId], references: [implementationSession.id] }),
    requirement: one(requirement, { fields: [sessionRequirementRef.requirementId], references: [requirement.id] })
}));
```

- [ ] **Step 2: Add to schema barrel export**

In `packages/db/src/schema/index.ts`, add:
```typescript
export * from "./implementation-session";
```

- [ ] **Step 3: Generate migration**

```bash
pnpm db:generate
```

- [ ] **Step 4: Verify types and commit**

```bash
pnpm run check-types
git add packages/db/src/schema/ packages/db/src/migrations/
git commit -m "feat: add implementation session schema"
```

---

### Task 2: Repository — session CRUD and ref management

**Files:**
- Create: `packages/db/src/repository/implementation-session.ts`
- Modify: `packages/db/src/repository/index.ts`

- [ ] **Step 1: Create session repository**

Create `packages/db/src/repository/implementation-session.ts` with these functions:

```typescript
import { and, desc, eq, sql } from "drizzle-orm";
import { Effect } from "effect";
import { DatabaseError } from "../errors";
import { db } from "../index";
import { chunk } from "../schema/chunk";
import {
    implementationSession,
    sessionAssumption,
    sessionChunkRef,
    sessionRequirementRef
} from "../schema/implementation-session";
import { requirement } from "../schema/requirement";
```

Functions:

- `createSession(params: { id, title, userId, codebaseId? })` → insert and return session
- `getSessionById(id, userId?)` → select session by id with optional user filter
- `listSessions(params: { userId, status?, codebaseId?, limit, offset })` → list with count, ordered by `createdAt DESC`
- `updateSession(id, userId, params: { status?, prUrl?, reviewBrief?, completedAt?, reviewedAt? })` → partial update
- `addChunkRef(sessionId, chunkId, reason)` → insert into session_chunk_ref with onConflictDoNothing
- `addAssumption(params: { id, sessionId, description })` → insert into session_assumption
- `resolveAssumption(id, params: { resolved, resolution? })` → update assumption
- `addRequirementRef(sessionId, requirementId, stepsAddressed?)` → insert into session_requirement_ref with onConflictDoNothing
- `getSessionDetail(id)` → fetches session plus all chunk refs (with chunk title), assumptions, and requirement refs (with requirement title/status/steps)

Use three separate queries for `getSessionDetail` (chunk refs, assumptions, requirement refs) joined with their parent tables, then combine the results.

All functions follow the `Effect.tryPromise({ try, catch: cause => new DatabaseError({ cause }) })` pattern.

- [ ] **Step 2: Add to repository barrel export**

In `packages/db/src/repository/index.ts`, add:
```typescript
export * from "./implementation-session";
```

- [ ] **Step 3: Verify types and commit**

```bash
pnpm run check-types
git add packages/db/src/repository/implementation-session.ts packages/db/src/repository/index.ts
git commit -m "feat: add implementation session repository"
```

---

### Task 3: Service — session logic and review brief generation

**Files:**
- Create: `packages/api/src/sessions/service.ts`
- Create: `packages/api/src/sessions/brief-generator.ts`

- [ ] **Step 1: Create brief generator**

Create `packages/api/src/sessions/brief-generator.ts`:

A pure function that takes session detail data and produces a markdown string:

```typescript
interface BriefInput {
    session: { title: string; createdAt: Date; completedAt?: Date | null };
    chunkRefs: Array<{ chunkId: string; chunkTitle: string; reason: string }>;
    assumptions: Array<{ id: string; description: string }>;
    requirementRefs: Array<{
        requirementId: string;
        requirementTitle: string;
        requirementStatus: string;
        totalSteps: number;
        stepsAddressed: number[];
    }>;
    allRequirements: Array<{ id: string; title: string; status: string; stepsCount: number }>;
    allConventions: Array<{ id: string; title: string }>;
}

export function generateReviewBrief(input: BriefInput): string { ... }
```

Generates markdown sections:
- **Summary Stats** — requirements addressed count / total, chunks referenced, assumptions count, duration
- **Requirements Addressed** — each with title, status, steps addressed/total. Flag partial coverage.
- **Requirements Not Addressed** — requirements in the codebase that weren't referenced (collapsed)
- **Conventions Applied** — each chunk ref with reason. Flag unreferenced conventions.
- **Assumptions Made** — each assumption description

- [ ] **Step 2: Create session service**

Create `packages/api/src/sessions/service.ts`:

```typescript
import {
    createSession as createSessionRepo,
    getSessionById,
    getSessionDetail,
    listSessions as listSessionsRepo,
    updateSession,
    addChunkRef as addChunkRefRepo,
    addAssumption as addAssumptionRepo,
    resolveAssumption as resolveAssumptionRepo,
    addRequirementRef as addRequirementRefRepo,
    listRequirements,
    updateRequirementStatus
} from "@fubbik/db/repository";
import { Effect } from "effect";
import { NotFoundError } from "../errors";
import { generateReviewBrief } from "./brief-generator";
```

Functions:

- `createSession(userId, body: { title, codebaseId? })` — creates session with `crypto.randomUUID()`, then fetches and returns context bundle:
  - Conventions: fetch chunks with convention tags or rationale (reuse the tag-based filtering from the existing `get_conventions` MCP tool logic)
  - Requirements: fetch all requirements for the codebase
  - Architecture decisions: fetch chunks with type='document' and rationale, filtered by architecture-related tags

- `getSession(id, userId)` — calls `getSessionDetail(id)`, then also fetches all requirements and convention chunks for the session's codebase (so the detail page can render "Not addressed" and "Not checked" sections). Returns `{ session, chunkRefs, assumptions, requirementRefs, allRequirements, allConventions }`

- `listSessions(userId, query)` — pagination wrapper

- `addChunkRef(sessionId, userId, chunkId, reason)` — validates session exists and belongs to user

- `addAssumption(sessionId, userId, description)` — validates session, creates with `crypto.randomUUID()`

- `resolveAssumption(sessionId, assumptionId, userId, body)` — validates session ownership

- `addRequirementRef(sessionId, userId, requirementId, stepsAddressed?)` — validates session

- `completeSession(sessionId, userId, prUrl?)` — sets status='completed', completedAt=now(), generates review brief from session data + all requirements/conventions, stores brief in session

- `reviewSession(sessionId, userId, requirementStatuses?)` — sets status='reviewed', reviewedAt=now(), updates requirement statuses if provided

- [ ] **Step 3: Verify types and commit**

```bash
pnpm run check-types
git add packages/api/src/sessions/
git commit -m "feat: add session service with review brief generation"
```

---

### Task 4: API routes — session endpoints

**Files:**
- Create: `packages/api/src/sessions/routes.ts`
- Modify: `packages/api/src/index.ts`

- [ ] **Step 1: Create session routes**

Create `packages/api/src/sessions/routes.ts`:

9 endpoints following the existing Elysia + Effect + requireSession pattern:

```typescript
import { Effect } from "effect";
import { Elysia, t } from "elysia";
import { requireSession } from "../require-session";
import * as sessionService from "./service";

export const sessionRoutes = new Elysia()
    .post("/sessions", ...) // Create session, returns session + context bundle. 201.
    .get("/sessions", ...) // List sessions. Query: status?, codebaseId?, limit?, offset?
    .get("/sessions/:id", ...) // Get session detail with refs, assumptions, brief
    .patch("/sessions/:id/complete", ...) // Complete session. Body: { prUrl? }
    .post("/sessions/:id/chunk-refs", ...) // Add chunk ref. Body: { chunkId, reason }. 201.
    .post("/sessions/:id/assumptions", ...) // Add assumption. Body: { description }. 201.
    .patch("/sessions/:id/assumptions/:assumptionId", ...) // Resolve. Body: { resolved, resolution? }
    .post("/sessions/:id/requirement-refs", ...) // Add req ref. Body: { requirementId, stepsAddressed? }. 201.
    .patch("/sessions/:id/review", ...) // Review. Body: { requirementStatuses?: [{requirementId, status}] }
```

Body schemas using Elysia's `t.Object()`:
- Create: `t.Object({ title: t.String(), codebaseId: t.Optional(t.String()) })`
- Complete: `t.Object({ prUrl: t.Optional(t.String()) })`
- Chunk ref: `t.Object({ chunkId: t.String(), reason: t.String() })`
- Assumption: `t.Object({ description: t.String() })`
- Resolve: `t.Object({ resolved: t.Boolean(), resolution: t.Optional(t.String()) })`
- Req ref: `t.Object({ requirementId: t.String(), stepsAddressed: t.Optional(t.Array(t.Number())) })`
- Review: `t.Object({ requirementStatuses: t.Optional(t.Array(t.Object({ requirementId: t.String(), status: t.Union([t.Literal("passing"), t.Literal("failing"), t.Literal("untested")]) }))) })`

- [ ] **Step 2: Register routes in API index**

In `packages/api/src/index.ts`, add import and `.use(sessionRoutes)`:
```typescript
import { sessionRoutes } from "./sessions/routes";
```

- [ ] **Step 3: Verify types and commit**

```bash
pnpm run check-types
git add packages/api/src/sessions/routes.ts packages/api/src/index.ts
git commit -m "feat: add session API endpoints"
```

---

### Task 5: MCP tools — session workflow tools

**Files:**
- Create: `packages/mcp/src/session-tools.ts`
- Modify: `packages/mcp/src/index.ts`

- [ ] **Step 1: Create session tools**

Create `packages/mcp/src/session-tools.ts`:

Follow the exact same pattern as `tools.ts` — use `apiFetch` helper, `z` for schemas, `server.tool()` for registration.

Export a `registerSessionTools(server: McpServer)` function with 5 tools:

**`begin_implementation`:**
- Input: `{ title: z.string(), codebaseId: z.string().optional() }`
- Calls `POST /api/sessions` with `{ title, codebaseId }`
- Returns the session ID + context bundle (conventions, requirements, architecture decisions)
- Format response as structured text showing the context

**`record_chunk_reference`:**
- Input: `{ sessionId: z.string(), chunkId: z.string(), reason: z.string() }`
- Calls `POST /api/sessions/:id/chunk-refs`
- Returns confirmation

**`record_assumption`:**
- Input: `{ sessionId: z.string(), description: z.string() }`
- Calls `POST /api/sessions/:id/assumptions`
- Returns confirmation

**`record_requirement_addressed`:**
- Input: `{ sessionId: z.string(), requirementId: z.string(), stepsAddressed: z.array(z.number()).optional() }`
- Calls `POST /api/sessions/:id/requirement-refs`
- Returns confirmation

**`complete_implementation`:**
- Input: `{ sessionId: z.string(), prUrl: z.string().optional() }`
- Calls `PATCH /api/sessions/:id/complete`
- Returns the generated review brief

**Required prerequisite:** Extract `apiFetch`, `getServerUrl`, and `truncate` from `packages/mcp/src/tools.ts` into a new `packages/mcp/src/api-client.ts` file. Update `tools.ts` to import from `api-client.ts`. Then `session-tools.ts` also imports from `api-client.ts`.

Create `packages/mcp/src/api-client.ts`:
```typescript
export function getServerUrl(): string {
    return process.env["FUBBIK_SERVER_URL"] ?? "http://localhost:3000";
}

export async function apiFetch(path: string, options?: RequestInit): Promise<unknown> {
    const url = `${getServerUrl()}/api${path}`;
    const res = await fetch(url, {
        ...options,
        headers: { "Content-Type": "application/json", ...options?.headers }
    });
    if (!res.ok) {
        const text = await res.text();
        throw new Error(`API ${res.status}: ${text}`);
    }
    return res.json();
}

export function truncate(text: string | null | undefined, maxLength: number): string {
    if (!text) return "";
    return text.length > maxLength ? text.slice(0, maxLength) + "..." : text;
}
```

Update `tools.ts` to remove these functions and import from `./api-client.js`.

- [ ] **Step 2: Register session tools in MCP server**

In `packages/mcp/src/index.ts`, add:
```typescript
import { registerSessionTools } from "./session-tools.js";

// After existing registerTools(server):
registerSessionTools(server);
```

- [ ] **Step 3: Verify types and commit**

```bash
pnpm run check-types
git add packages/mcp/src/
git commit -m "feat: add MCP session workflow tools"
```

---

### Task 6: Web UI — reviews list page

**Files:**
- Create: `apps/web/src/routes/reviews.tsx`
- Create: `apps/web/src/features/reviews/session-card.tsx`
- Modify: `apps/web/src/routes/__root.tsx` (add Reviews to nav)
- Modify: `apps/web/src/features/nav/mobile-nav.tsx` (add Reviews to mobile nav)

- [ ] **Step 1: Create SessionCard component**

Create `apps/web/src/features/reviews/session-card.tsx`:

Props:
```typescript
interface SessionCardProps {
    id: string;
    title: string;
    status: string; // in_progress, completed, reviewed
    codebaseName?: string;
    createdAt: string;
    requirementCount: number;
    assumptionCount: number;
    unresolvedCount: number;
}
```

Card layout:
- Title as clickable Link to `/reviews/$sessionId`
- Status badge: in_progress=blue, completed=amber, reviewed=green
- Codebase name badge if set
- Footer: "N requirements · N assumptions (N unresolved)" + creation date

- [ ] **Step 2: Create reviews list page**

Create `apps/web/src/routes/reviews.tsx`:

Route: `createFileRoute("/reviews")`

Features:
- Fetches from `GET /api/sessions` with pagination (limit=20)
- Filter by status (All, In Progress, Completed, Reviewed)
- Filter by codebase (using `useActiveCodebase`)
- Renders SessionCard for each session
- Empty state: "No implementation sessions yet"
- Pagination (same pattern as requirements list)

- [ ] **Step 3: Add Reviews to nav**

In `apps/web/src/routes/__root.tsx`, add a "Reviews" link in the desktop nav (between Requirements and other items). Use `ClipboardList` icon from lucide-react.

In `apps/web/src/features/nav/mobile-nav.tsx`, add to the `primaryItems` array:
```typescript
{ label: "Reviews", to: "/reviews" as const, icon: ClipboardList }
```

- [ ] **Step 4: Verify types and commit**

```bash
pnpm run check-types
git add apps/web/src/routes/reviews.tsx apps/web/src/features/reviews/ apps/web/src/routes/__root.tsx apps/web/src/features/nav/mobile-nav.tsx
git commit -m "feat: add reviews list page"
```

---

### Task 7: Web UI — review detail page

**Files:**
- Create: `apps/web/src/routes/reviews_.$sessionId.tsx`
- Create: `apps/web/src/features/reviews/assumption-resolver.tsx`

- [ ] **Step 1: Create AssumptionResolver component**

Create `apps/web/src/features/reviews/assumption-resolver.tsx`:

Props:
```typescript
interface AssumptionResolverProps {
    assumption: { id: string; description: string; resolved: boolean; resolution: string | null };
    sessionId: string;
}
```

Features:
- Shows description text
- Resolved indicator (checkmark if resolved, warning if not)
- "Resolve" button → expands inline form with resolution text input + Save/Cancel
- "Create chunk" button → navigates to `/chunks/new` with description pre-filled as content (via URL query param or stored in sessionStorage)
- On resolve, calls `PATCH /api/sessions/:id/assumptions/:assumptionId`
- Mutation invalidates `["session", sessionId]`

- [ ] **Step 2: Create review detail page**

Create `apps/web/src/routes/reviews_.$sessionId.tsx`:

Route: `createFileRoute("/reviews_/$sessionId")`

Fetches from `GET /api/sessions/:id` which returns the full session detail including refs, assumptions, and review brief.

Layout:
- BackLink to `/reviews`
- Header: title, status badge, codebase, creation date, PR link if set
- If review brief exists, render it as markdown (or structured sections)
- If no brief yet (in_progress), show the raw data

**Structured sections (rendered from session data, not markdown):**

**Requirements Addressed:**
- Each requirement as a Link to `/requirements/$requirementId` with status badge
- Show "N/M steps addressed"

**Conventions Applied:**
- Each chunk ref as a Link to `/chunks/$chunkId` with the AI's reason

**Assumptions:**
- Render each via `AssumptionResolver` component

**Knowledge Gaps:**
- Show conventions from `allConventions` that were NOT in `chunkRefs` — collapsed "Not checked" section
- Show requirements from `allRequirements` that were NOT in `requirementRefs` — collapsed "Not addressed" section
- Each item links to its detail page

**Actions (when status = completed):**
- "Mark as Reviewed" button
- Shows requirement status form: for each addressed requirement, a dropdown to set status (passing/failing/untested)
- On submit, calls `PATCH /api/sessions/:id/review` with requirementStatuses

- [ ] **Step 3: Verify types and commit**

```bash
pnpm run check-types
git add apps/web/src/routes/reviews_.\$sessionId.tsx apps/web/src/features/reviews/assumption-resolver.tsx
git commit -m "feat: add review detail page with assumption resolver"
```

---

### Task 8: Integration — requirement detail + knowledge health

**Files:**
- Modify: `apps/web/src/routes/requirements_.$requirementId.tsx`
- Modify: `apps/web/src/routes/knowledge-health.tsx` (if it exists, otherwise the knowledge health page)

- [ ] **Step 1: Add sessions section to requirement detail**

In `apps/web/src/routes/requirements_.$requirementId.tsx`:

Add a new section "Implementation Sessions" after the existing linked chunks section (in view mode only). Fetch from `GET /api/sessions` filtered by... actually, there's no direct API to get sessions by requirement. Add a simple approach: the `session_requirement_ref` data is available when viewing a session, but for the reverse lookup we need a query.

Simplest approach: add a small query that fetches sessions referencing this requirement. Add a repository function `getSessionsByRequirementId(requirementId)` that joins `session_requirement_ref` with `implementation_session`.

Add to repository: `packages/db/src/repository/implementation-session.ts`:
```typescript
export function getSessionsForRequirement(requirementId: string) {
    return Effect.tryPromise({
        try: () =>
            db.select({
                id: implementationSession.id,
                title: implementationSession.title,
                status: implementationSession.status,
                createdAt: implementationSession.createdAt
            })
            .from(sessionRequirementRef)
            .innerJoin(implementationSession, eq(sessionRequirementRef.sessionId, implementationSession.id))
            .where(eq(sessionRequirementRef.requirementId, requirementId)),
        catch: cause => new DatabaseError({ cause })
    });
}
```

Add an API endpoint: `GET /api/sessions/by-requirement/:requirementId` in `sessions/routes.ts`.

On the requirement detail page, fetch and show a small list of sessions that referenced this requirement, each linking to `/reviews/$sessionId`.

- [ ] **Step 2: Add knowledge gaps section to knowledge health**

Read the knowledge health page file first. Then add a "Knowledge Gaps from AI Sessions" section.

Fetch unresolved assumptions aggregated across all sessions. Query:
```sql
SELECT description, count(*) as frequency, array_agg(session_id) as session_ids
FROM session_assumption
WHERE resolved = false
GROUP BY description
ORDER BY count(*) DESC
LIMIT 20
```

Add this as a repository function: `getUnresolvedAssumptionsSummary(userId)`.
Add this as an API endpoint: `GET /api/sessions/knowledge-gaps`.

On the knowledge health page, add a section showing:
- Each gap with frequency count ("AI assumed this N times")
- Links to the sessions where it appeared
- "Create chunk" shortcut button per gap

- [ ] **Step 3: Verify types and commit**

```bash
pnpm run check-types
git add packages/db/src/repository/implementation-session.ts packages/api/src/sessions/ apps/web/src/routes/requirements_.\$requirementId.tsx apps/web/src/routes/knowledge-health.tsx
git commit -m "feat: integrate sessions into requirement detail and knowledge health"
```

---

### Task 9: Final verification

- [ ] **Step 1: Run full type check**

Run: `pnpm run check-types`

- [ ] **Step 2: Run tests**

Run: `pnpm test`

- [ ] **Step 3: Generate and apply migration to local DB**

```bash
pnpm db:push
```

- [ ] **Step 4: Manual verification checklist**

Run `pnpm dev` and verify:
- [ ] MCP server starts with `npx tsx packages/mcp/src/index.ts`
- [ ] `begin_implementation` returns a context bundle
- [ ] `record_chunk_reference` works
- [ ] `record_assumption` works
- [ ] `record_requirement_addressed` works
- [ ] `complete_implementation` generates a review brief
- [ ] `/reviews` page lists sessions
- [ ] `/reviews/:id` page shows review brief with interactive assumptions
- [ ] Assumption resolver works (resolve + create chunk shortcut)
- [ ] "Mark as Reviewed" updates session and requirement statuses
- [ ] Requirement detail shows linked sessions
- [ ] Knowledge health page shows knowledge gaps
- [ ] Nav shows Reviews link

- [ ] **Step 5: Commit any remaining fixes**

```bash
git add -A
git commit -m "chore: final cleanup for MCP review workflow"
```
