# General Codebase Improvements Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Fix 20 prioritized issues spanning security, architecture, validation, DX, and frontend UX identified during a full codebase audit.

**Architecture:** Changes span all layers — backend security gates, input validation schemas, CI configuration, infrastructure activation, dead code removal, and frontend component refactoring. Each task is self-contained and can be committed independently.

**Tech Stack:** TypeScript, Elysia, Effect, Drizzle, TanStack Start/Router, React, Winston, OpenTelemetry, pnpm, Turborepo

---

## Phase 1: Critical Security & Safety

### Task 1: Gate auth bypass behind NODE_ENV

**Files:**
- Modify: `packages/api/src/index.ts:66-99`

- [ ] **Step 1: Add environment gate to getSession fallback**

In `packages/api/src/index.ts`, replace the `getSession` function (lines 92-99):

```typescript
// Before:
async function getSession(headers: Headers): Promise<Session> {
    try {
        const session = await auth.api.getSession({ headers });
        return session ?? DEV_SESSION;
    } catch {
        return DEV_SESSION;
    }
}
```

```typescript
// After:
async function getSession(headers: Headers): Promise<Session | null> {
    try {
        const session = await auth.api.getSession({ headers });
        if (session) return session;
    } catch {
        // auth.api.getSession can throw on invalid/expired tokens
    }
    if (process.env.NODE_ENV !== "production") return DEV_SESSION;
    return null;
}
```

- [ ] **Step 2: Update the .resolve handler to handle null session**

The `.resolve` block at line 130-133 currently always returns a session. Update the downstream `requireSession` to handle the null case. Check `packages/api/src/require-session.ts` — it already checks for `ctx.session` and fails with `AuthError` if missing, so the null propagation is already handled. No change needed there.

However, the `.resolve` return type changes. Update line 130-133:

```typescript
// Before:
.resolve(async ({ headers }) => {
    const session = await getSession(new Headers(headers as Record<string, string>));
    return { session };
})

// After:
.resolve(async ({ headers }) => {
    const session = await getSession(new Headers(headers as Record<string, string>));
    return { session: session ?? undefined };
})
```

- [ ] **Step 3: Verify locally**

Run: `pnpm dev`

Test in dev mode: `curl http://localhost:3000/api/me` should return the dev user (bypass works in dev).

Set `NODE_ENV=production` and restart — `curl http://localhost:3000/api/me` should return 401.

- [ ] **Step 4: Commit**

```bash
git add packages/api/src/index.ts
git commit -m "fix(security): gate auth bypass behind NODE_ENV to prevent production exposure"
```

---

### Task 2: Fix Cypher injection in AGE queries

**Files:**
- Modify: `packages/db/src/age/sync.ts`
- Modify: `packages/db/src/age/client.ts`

- [ ] **Step 1: Add a Cypher string escaping helper to `client.ts`**

Add this function at the top of `packages/db/src/age/client.ts`, after the imports:

```typescript
/** Escape a string value for use inside Cypher single-quoted literals. */
export function escCypher(value: string): string {
    return value.replace(/\\/g, "\\\\").replace(/'/g, "\\'");
}
```

- [ ] **Step 2: Apply escaping in `sync.ts`**

Replace the entire file `packages/db/src/age/sync.ts`:

```typescript
// packages/db/src/age/sync.ts
import { cypherVoid, escCypher } from "./client";

export function ensureVertex(label: string, id: string) {
    return cypherVoid(`MERGE (:${label} {id: '${escCypher(id)}'})`);
}

export function deleteVertex(label: string, id: string) {
    return cypherVoid(`MATCH (v:${label} {id: '${escCypher(id)}'}) DETACH DELETE v`);
}

export function createEdge(
    edgeLabel: string,
    fromLabel: string,
    fromId: string,
    toLabel: string,
    toId: string,
    props: Record<string, string> = {}
) {
    const propsStr = Object.entries(props)
        .map(([k, v]) => `${k}: '${escCypher(v)}'`)
        .join(", ");
    const propsClause = propsStr ? ` {${propsStr}}` : "";
    return cypherVoid(
        `MATCH (a:${fromLabel} {id: '${escCypher(fromId)}'}), (b:${toLabel} {id: '${escCypher(toId)}'})
         CREATE (a)-[:${edgeLabel}${propsClause}]->(b)`
    );
}

export function deleteEdge(edgeLabel: string, props: Record<string, string>) {
    const conditions = Object.entries(props)
        .map(([k, v]) => `e.${k} = '${escCypher(v)}'`)
        .join(" AND ");
    return cypherVoid(
        `MATCH ()-[e:${edgeLabel}]-() WHERE ${conditions} DELETE e`
    );
}

export function deleteEdgesFrom(edgeLabel: string, fromLabel: string, fromId: string) {
    return cypherVoid(
        `MATCH (a:${fromLabel} {id: '${escCypher(fromId)}'})-[e:${edgeLabel}]->() DELETE e`
    );
}
```

- [ ] **Step 3: Verify existing AGE tests still pass**

Run: `pnpm --filter @fubbik/db test -- --grep "age" --reporter verbose`

Expected: Tests pass (or skip if AGE is not available locally).

- [ ] **Step 4: Commit**

```bash
git add packages/db/src/age/client.ts packages/db/src/age/sync.ts
git commit -m "fix(security): escape Cypher string literals to prevent injection in AGE queries"
```

---

### Task 3: Replace drizzle-kit push with migrate in entrypoint

**Files:**
- Modify: `apps/server/entrypoint.sh:27-30`

- [ ] **Step 1: Replace push with migrate and fail-fast**

In `apps/server/entrypoint.sh`, replace lines 27-30:

```bash
# Before:
# Apply Drizzle-managed schema migrations (tracked in __drizzle_migrations table)
if ! drizzle-kit push 2>&1; then
    echo "Warning: drizzle-kit push failed. Trying migrate..."
    drizzle-kit migrate 2>&1 || echo "Warning: drizzle-kit migrate also failed. Continuing..."
fi
```

```bash
# After:
# Apply Drizzle-managed schema migrations (tracked in __drizzle_migrations table)
echo "Running drizzle-kit migrate..."
drizzle-kit migrate 2>&1 || { echo "ERROR: drizzle-kit migrate failed. Aborting."; exit 1; }
```

- [ ] **Step 2: Commit**

```bash
git add apps/server/entrypoint.sh
git commit -m "fix(deploy): replace drizzle-kit push with migrate in entrypoint and fail-fast on error"
```

---

## Phase 2: Infrastructure & Developer Experience

### Task 4: Activate OpenTelemetry tracing

**Files:**
- Modify: `apps/server/src/index.ts`
- Modify: `apps/server/src/lib/tracing.ts`

- [ ] **Step 1: Use the project logger in tracing.ts instead of console.log**

In `apps/server/src/lib/tracing.ts`, add import and replace console calls:

Add at line 8:
```typescript
import { logger } from "../logger";
```

Replace lines 72-73:
```typescript
// Before:
console.log(`[OpenTelemetry] Tracing started for service: ${serviceName}`);
console.log(`[OpenTelemetry] Exporting to: ${otlpEndpoint}`);

// After:
logger.info(`[OpenTelemetry] Tracing started for service: ${serviceName}`);
logger.info(`[OpenTelemetry] Exporting to: ${otlpEndpoint}`);
```

Replace line 89:
```typescript
// Before:
console.log("[OpenTelemetry] Tracing shutdown complete");

// After:
logger.info("[OpenTelemetry] Tracing shutdown complete");
```

Replace line 91:
```typescript
// Before:
console.error("[OpenTelemetry] Error shutting down tracing:", error);

// After:
logger.error("[OpenTelemetry] Error shutting down tracing:", { error });
```

- [ ] **Step 2: Call startTracing() conditionally in server entry**

In `apps/server/src/index.ts`, add import at line 2 (after the cors import):

```typescript
import { startTracing, shutdownTracing } from "./lib/tracing";
```

Add before the `new Elysia()` block (before line 11):

```typescript
// Start OpenTelemetry if an OTLP endpoint is configured
if (process.env.OTEL_EXPORTER_OTLP_ENDPOINT) {
    startTracing();
}
```

Add shutdown handler at the end of the file (after `.listen(...)`):

```typescript
// Graceful shutdown
process.on("SIGTERM", async () => {
    if (process.env.OTEL_EXPORTER_OTLP_ENDPOINT) {
        await shutdownTracing();
    }
    process.exit(0);
});
```

- [ ] **Step 3: Verify server starts without OTEL configured**

Run: `pnpm dev`

Expected: Server starts normally, no OTEL log messages (because `OTEL_EXPORTER_OTLP_ENDPOINT` is not set).

- [ ] **Step 4: Commit**

```bash
git add apps/server/src/index.ts apps/server/src/lib/tracing.ts
git commit -m "feat(observability): activate OpenTelemetry tracing when OTLP endpoint is configured"
```

---

### Task 5: Consolidate duplicate logger and replace console.log in API layer

**Files:**
- Delete: `apps/server/src/lib/logger.ts`
- Modify: `apps/server/src/logger.ts`
- Modify: `packages/api/src/index.ts`
- Modify: `packages/api/src/events/handlers.ts`
- Modify: `packages/api/src/events/bus.ts`
- Modify: `packages/api/src/chunks/service.ts`

- [ ] **Step 1: Enhance the active logger with LOG_LEVEL support and createChildLogger**

Replace the entire `apps/server/src/logger.ts`:

```typescript
import { env } from "@fubbik/env/server";
import winston from "winston";

const logLevel = process.env.LOG_LEVEL || (env.NODE_ENV === "production" ? "info" : "debug");

export const logger = winston.createLogger({
    level: logLevel,
    format: winston.format.combine(
        winston.format.timestamp(),
        env.NODE_ENV === "production"
            ? winston.format.json()
            : winston.format.combine(winston.format.colorize(), winston.format.simple())
    ),
    defaultMeta: { env: env.NODE_ENV },
    transports: [
        new winston.transports.Console({
            stderrLevels: ["error"]
        })
    ]
});

export function createChildLogger(bindings: Record<string, unknown>): winston.Logger {
    return logger.child(bindings);
}
```

- [ ] **Step 2: Delete the dead logger**

```bash
rm apps/server/src/lib/logger.ts
```

- [ ] **Step 3: Export logger from server package for use by API**

The API package can't import directly from `apps/server`. Instead, we'll create a minimal logger instance in the API package. Create `packages/api/src/logger.ts`:

```typescript
import winston from "winston";

const logLevel = process.env.LOG_LEVEL || (process.env.NODE_ENV === "production" ? "info" : "debug");

export const logger = winston.createLogger({
    level: logLevel,
    format: winston.format.combine(
        winston.format.timestamp(),
        process.env.NODE_ENV === "production"
            ? winston.format.json()
            : winston.format.combine(winston.format.colorize(), winston.format.simple())
    ),
    transports: [new winston.transports.Console()]
});
```

Check that `winston` is already a dependency of `@fubbik/api` in `packages/api/package.json`. If not, add it.

- [ ] **Step 4: Replace console.log/error in packages/api/src/index.ts**

Add import at top of `packages/api/src/index.ts`:

```typescript
import { logger } from "./logger";
```

Replace line 118:
```typescript
// Before:
console.error("AI service error", effectError.cause);
// After:
logger.error("AI service error", { cause: effectError.cause });
```

Replace line 125:
```typescript
// Before:
console.error("Database error", effectError.cause);
// After:
logger.error("Database error", { cause: effectError.cause });
```

- [ ] **Step 5: Replace console calls in event handlers**

In `packages/api/src/events/handlers.ts`, add import:
```typescript
import { logger } from "../logger";
```

Replace `console.log(...)` and `console.error(...)` calls with `logger.info(...)` and `logger.error(...)`.

In `packages/api/src/events/bus.ts`, add import:
```typescript
import { logger } from "../logger";
```

Replace `console.error(...)` with `logger.error(...)`.

In `packages/api/src/chunks/service.ts`, add import:
```typescript
import { logger } from "../logger";
```

Replace `console.error(...)` with `logger.error(...)`.

- [ ] **Step 6: Verify build**

Run: `pnpm build`

Expected: No errors.

- [ ] **Step 7: Commit**

```bash
git add -A apps/server/src/logger.ts packages/api/src/logger.ts packages/api/src/index.ts packages/api/src/events/handlers.ts packages/api/src/events/bus.ts packages/api/src/chunks/service.ts
git rm apps/server/src/lib/logger.ts
git commit -m "refactor(logging): consolidate to single logger, replace console calls with structured logging"
```

---

### Task 6: Fix CI to use pnpm instead of bun

**Files:**
- Modify: `.github/workflows/ci.yml`

- [ ] **Step 1: Replace bun with pnpm in CI**

Replace the entire `.github/workflows/ci.yml`:

```yaml
name: CI

on:
    push:
        branches: [main]
    pull_request:
        branches: [main]
        paths-ignore:
            - "docs/**"
            - "*.md"
            - ".claude/**"

jobs:
    lint-and-format:
        runs-on: ubuntu-latest
        steps:
            - uses: actions/checkout@v4

            - uses: pnpm/action-setup@v4

            - uses: actions/setup-node@v4
              with:
                  node-version: 22
                  cache: pnpm

            - run: pnpm install --frozen-lockfile

            - name: Sherif
              run: pnpm sherif

            - name: Lint
              run: pnpm lint

            - name: Format check
              run: pnpm fmt:check

    typecheck-and-test:
        runs-on: ubuntu-latest
        steps:
            - uses: actions/checkout@v4

            - uses: pnpm/action-setup@v4

            - uses: actions/setup-node@v4
              with:
                  node-version: 22
                  cache: pnpm

            - run: pnpm install --frozen-lockfile

            - name: Type check
              run: pnpm check-types

            - name: Test
              run: pnpm test

    build:
        needs: [lint-and-format, typecheck-and-test]
        runs-on: ubuntu-latest
        steps:
            - uses: actions/checkout@v4

            - uses: pnpm/action-setup@v4

            - uses: actions/setup-node@v4
              with:
                  node-version: 22
                  cache: pnpm

            - run: pnpm install --frozen-lockfile

            - name: Build
              run: pnpm run build
```

Note: `pnpm/action-setup@v4` reads `packageManager` from `package.json` to auto-detect the pnpm version, so no explicit version pin is needed.

- [ ] **Step 2: Commit**

```bash
git add .github/workflows/ci.yml
git commit -m "fix(ci): use pnpm instead of bun for reproducible builds matching workspace lockfile"
```

---

### Task 7: Add check-types scripts to shared packages

**Files:**
- Modify: `packages/api/package.json`
- Modify: `packages/db/package.json`
- Modify: `packages/auth/package.json`
- Modify: `packages/env/package.json`

- [ ] **Step 1: Check what check-types command the apps use**

Look at `apps/server/package.json` and `apps/web/package.json` for their `check-types` scripts. They likely use `tsgo --noEmit` or `tsc --noEmit`.

Run: `grep -A1 '"check-types"' apps/server/package.json apps/web/package.json apps/cli/package.json`

- [ ] **Step 2: Add check-types to each shared package**

Add the `check-types` script to each package's `package.json` `scripts` field, matching the pattern from apps (likely `tsgo --noEmit`):

In `packages/api/package.json`, add to `"scripts"`:
```json
"check-types": "tsgo --noEmit"
```

In `packages/db/package.json`, add to `"scripts"`:
```json
"check-types": "tsgo --noEmit"
```

In `packages/auth/package.json`, add to `"scripts"`:
```json
"check-types": "tsgo --noEmit"
```

In `packages/env/package.json`, add to `"scripts"`:
```json
"check-types": "tsgo --noEmit"
```

- [ ] **Step 3: Verify type checking runs on all packages**

Run: `pnpm check-types`

Expected: All packages are type-checked (including the four newly added ones). Fix any type errors that surface.

- [ ] **Step 4: Commit**

```bash
git add packages/api/package.json packages/db/package.json packages/auth/package.json packages/env/package.json
git commit -m "fix(dx): add check-types scripts to shared packages for CI type coverage"
```

---

### Task 8: Clean up .env.example

**Files:**
- Modify: `apps/server/.env.example`

- [ ] **Step 1: Remove stale Typesense vars and add missing ones**

Replace `apps/server/.env.example`:

```env
# Required
BETTER_AUTH_SECRET=
BETTER_AUTH_URL=http://localhost:3000
DATABASE_URL=postgresql://postgres:password@localhost:5432/fubbik

# Comma-separated for multiple origins (e.g. with Caddy reverse proxy)
CORS_ORIGIN=http://localhost:3001

# Optional: OpenAI (for AI summarization/suggestion features)
# OPENAI_API_KEY=
# OPENAI_MODEL=gpt-4o-mini

# Optional: Ollama (for local enrichment + embeddings)
# OLLAMA_URL=http://localhost:11434

# Optional: OpenTelemetry
# OTEL_SERVICE_NAME=fubbik-server
# OTEL_EXPORTER_OTLP_ENDPOINT=http://localhost:4318

# Optional: Rate limiting
# RATE_LIMIT_MAX=100
# RATE_LIMIT_DURATION_MS=60000

# Optional: Logging
# LOG_LEVEL=info
```

- [ ] **Step 2: Commit**

```bash
git add apps/server/.env.example
git commit -m "docs(env): remove stale Typesense vars, add missing OPENAI/OLLAMA/LOG_LEVEL vars"
```

---

## Phase 3: Input Validation & Architecture

### Task 9: Replace t.Any() with proper schemas

**Files:**
- Modify: `packages/api/src/templates/routes.ts:30-31,51-52`
- Modify: `packages/api/src/plans/analyze.ts:70,87`
- Modify: `packages/api/src/search/routes.ts:119`

- [ ] **Step 1: Define proper schema for template matchRules and fieldMappings**

In `packages/api/src/templates/routes.ts`, replace `t.Any()` on lines 30-31 and 51-52.

First, define the schemas above `templateRoutes`:

```typescript
const MatchRuleSchema = t.Object({
    field: t.String(),
    pattern: t.String(),
    weight: t.Optional(t.Number())
});

const FieldMappingSchema = t.Object({
    source: t.String(),
    target: t.String(),
    transform: t.Optional(t.String())
});
```

Then replace in the POST body (lines 30-31):
```typescript
// Before:
matchRules: t.Optional(t.Any()),
fieldMappings: t.Optional(t.Any()),

// After:
matchRules: t.Optional(t.Array(MatchRuleSchema)),
fieldMappings: t.Optional(t.Array(FieldMappingSchema)),
```

And the same in the PATCH body (lines 51-52):
```typescript
matchRules: t.Optional(t.Array(MatchRuleSchema)),
fieldMappings: t.Optional(t.Array(FieldMappingSchema)),
```

**Important:** Before defining the schemas, check what shape the frontend actually sends for these fields by grepping:
```bash
grep -r "matchRules\|fieldMappings" apps/web/src/ packages/api/src/templates/
```
Adjust the schemas to match the actual data shape used in the codebase.

- [ ] **Step 2: Define proper schema for plan analyze metadata**

In `packages/api/src/plans/analyze.ts`, the `metadata` field stores kind-specific data (severity for risks, verified flag for assumptions, answer for questions, line range for files). Replace `t.Any()` on lines 70 and 87:

```typescript
// A flexible but bounded metadata schema
const AnalyzeMetadataSchema = t.Optional(
    t.Record(t.String({ maxLength: 50 }), t.Union([
        t.String({ maxLength: 2000 }),
        t.Number(),
        t.Boolean(),
        t.Null()
    ]))
);
```

Replace line 70:
```typescript
// Before:
metadata: t.Optional(t.Any()),
// After:
metadata: AnalyzeMetadataSchema,
```

Replace line 87:
```typescript
// Before:
metadata: t.Optional(t.Any()),
// After:
metadata: AnalyzeMetadataSchema,
```

- [ ] **Step 3: Define proper schema for saved search query**

In `packages/api/src/search/routes.ts`, the saved query's `query` field stores an array of search clauses. The `ClauseSchema` is already defined at lines 9-15. Reuse it:

Replace line 119:
```typescript
// Before:
query: t.Any(),

// After:
query: t.Object({
    clauses: t.Array(ClauseSchema),
    join: t.Optional(t.Union([t.Literal("and"), t.Literal("or")])),
    sort: t.Optional(t.String()),
    codebaseId: t.Optional(t.String())
}),
```

**Important:** Verify this matches what the frontend sends by checking the saved query creation flow:
```bash
grep -r "search/saved" apps/web/src/
```

- [ ] **Step 4: Verify build**

Run: `pnpm build`

Expected: No errors.

- [ ] **Step 5: Commit**

```bash
git add packages/api/src/templates/routes.ts packages/api/src/plans/analyze.ts packages/api/src/search/routes.ts
git commit -m "fix(validation): replace t.Any() with proper schemas for template, analyze, and search routes"
```

---

### Task 10: Replace Number() coercion with t.Numeric()

**Files:**
- Modify: `packages/api/src/activity/routes.ts`
- Modify: `packages/api/src/proposals/routes.ts`
- Modify: `packages/api/src/notifications/routes.ts`
- Modify: `packages/api/src/staleness/routes.ts`

- [ ] **Step 1: Fix activity routes**

In `packages/api/src/activity/routes.ts`, replace the query schema (lines 23-28) and remove the manual `Number()` coercion (lines 16-17):

Replace lines 16-17:
```typescript
// Before:
limit: ctx.query.limit ? Number(ctx.query.limit) : undefined,
offset: ctx.query.offset ? Number(ctx.query.offset) : undefined

// After:
limit: ctx.query.limit,
offset: ctx.query.offset
```

Replace lines 26-27 in the query schema:
```typescript
// Before:
limit: t.Optional(t.String()),
offset: t.Optional(t.String())

// After:
limit: t.Optional(t.Numeric()),
offset: t.Optional(t.Numeric())
```

- [ ] **Step 2: Fix proposals routes**

In `packages/api/src/proposals/routes.ts`, replace lines 103-104:
```typescript
// Before:
limit: ctx.query.limit ? Number(ctx.query.limit) : undefined,
offset: ctx.query.offset ? Number(ctx.query.offset) : undefined,

// After:
limit: ctx.query.limit,
offset: ctx.query.offset,
```

Replace lines 114-115 in the query schema:
```typescript
// Before:
limit: t.Optional(t.String()),
offset: t.Optional(t.String()),

// After:
limit: t.Optional(t.Numeric()),
offset: t.Optional(t.Numeric()),
```

- [ ] **Step 3: Fix notifications routes**

In `packages/api/src/notifications/routes.ts`, replace line 33:
```typescript
// Before:
limit: ctx.query.limit ? Number(ctx.query.limit) : undefined,

// After:
limit: ctx.query.limit,
```

Replace line 41 in the query schema:
```typescript
// Before:
limit: t.Optional(t.String()),

// After:
limit: t.Optional(t.Numeric()),
```

- [ ] **Step 4: Fix staleness routes**

In `packages/api/src/staleness/routes.ts`, replace line 18:
```typescript
// Before:
limit: ctx.query.limit ? Number(ctx.query.limit) : undefined

// After:
limit: ctx.query.limit
```

Replace line 27 in the query schema:
```typescript
// Before:
limit: t.Optional(t.String())

// After:
limit: t.Optional(t.Numeric())
```

- [ ] **Step 5: Verify build**

Run: `pnpm build`

Expected: No errors. `t.Numeric()` auto-coerces string query params to numbers and validates they are numeric.

- [ ] **Step 6: Commit**

```bash
git add packages/api/src/activity/routes.ts packages/api/src/proposals/routes.ts packages/api/src/notifications/routes.ts packages/api/src/staleness/routes.ts
git commit -m "fix(validation): use t.Numeric() instead of manual Number() coercion for query params"
```

---

### Task 11: Add service layer for routes that bypass it

**Files:**
- Create: `packages/api/src/learning-paths/service.ts`
- Modify: `packages/api/src/learning-paths/routes.ts`
- Modify: `packages/api/src/search/routes.ts`
- Modify: `packages/api/src/enrich/routes.ts`
- Modify: `packages/api/src/tasks/routes.ts`

- [ ] **Step 1: Create learning-paths service**

Create `packages/api/src/learning-paths/service.ts`:

```typescript
import {
    listLearningPaths,
    getLearningPath,
    createLearningPath,
    updateLearningPath,
    deleteLearningPath,
} from "@fubbik/db/repository";

export { listLearningPaths, getLearningPath, createLearningPath, updateLearningPath, deleteLearningPath };
```

This is a thin pass-through for now, establishing the pattern. Business logic (ownership checks, validation) can be added later.

- [ ] **Step 2: Update learning-paths routes to import from service**

In `packages/api/src/learning-paths/routes.ts`, replace lines 4-10:

```typescript
// Before:
import {
    listLearningPaths,
    getLearningPath,
    createLearningPath,
    updateLearningPath,
    deleteLearningPath,
} from "@fubbik/db/repository";

// After:
import {
    listLearningPaths,
    getLearningPath,
    createLearningPath,
    updateLearningPath,
    deleteLearningPath,
} from "./service";
```

- [ ] **Step 3: Move saved query repo calls out of search routes**

In `packages/api/src/search/routes.ts`, the saved query CRUD calls go directly to the repository. Add these re-exports to the existing search service.

Check if `packages/api/src/search/service.ts` exists. If so, add to it:

```typescript
import { listSavedQueries, createSavedQuery, deleteSavedQuery } from "@fubbik/db/repository";
export { listSavedQueries, createSavedQuery, deleteSavedQuery };
```

Then update `packages/api/src/search/routes.ts` line 4:

```typescript
// Before:
import { listSavedQueries, createSavedQuery, deleteSavedQuery } from "@fubbik/db/repository";
import { requireSession } from "../require-session";
import { executeSearch, autocomplete } from "./service";

// After:
import { requireSession } from "../require-session";
import { executeSearch, autocomplete, listSavedQueries, createSavedQuery, deleteSavedQuery } from "./service";
```

- [ ] **Step 4: Move listChunks out of enrich routes**

In `packages/api/src/enrich/routes.ts`, replace line 1:

```typescript
// Before:
import { listChunks } from "@fubbik/db/repository";

// After:
import { listChunks } from "../chunks/service";
```

Verify `listChunks` is exported from `packages/api/src/chunks/service.ts`. If not, add:
```typescript
export { listChunks } from "@fubbik/db/repository";
```

- [ ] **Step 5: Route task claim/complete through plan service**

In `packages/api/src/tasks/routes.ts`, the `claim` and `complete` endpoints call `planRepo.updateTask` directly. Route through plan service instead.

First, check if `planService` already has a `updateTask` method. If not, add to `packages/api/src/plans/service.ts`:

```typescript
export function updateTask(taskId: string, data: { status: string }) {
    return planRepo.updateTask(taskId, data);
}
```

Then in `packages/api/src/tasks/routes.ts`, remove the direct repo import (line 4):

```typescript
// Before:
import * as planRepo from "@fubbik/db/repository/plan";

// After: (remove this line entirely)
```

Replace `planRepo.updateTask` calls (lines 59, 78) with `planService.updateTask`:

```typescript
// Before:
yield* planRepo.updateTask(firstTask.id, { status: "in_progress" });
// After:
yield* planService.updateTask(firstTask.id, { status: "in_progress" });
```

```typescript
// Before:
yield* planRepo.updateTask(firstTask.id, { status: "done" });
// After:
yield* planService.updateTask(firstTask.id, { status: "done" });
```

- [ ] **Step 6: Verify build**

Run: `pnpm build`

Expected: No errors.

- [ ] **Step 7: Commit**

```bash
git add packages/api/src/learning-paths/service.ts packages/api/src/learning-paths/routes.ts packages/api/src/search/routes.ts packages/api/src/search/service.ts packages/api/src/enrich/routes.ts packages/api/src/chunks/service.ts packages/api/src/tasks/routes.ts packages/api/src/plans/service.ts
git commit -m "refactor(architecture): route all endpoints through service layer instead of direct repo access"
```

---

## Phase 4: Dead Code Cleanup

### Task 12: Address Vercel AI SDK / document its actual status

**Files:**
- Modify: `packages/api/src/ai/service.ts` (if removing)
- Modify: `packages/api/package.json` (if removing deps)
- OR: Modify: `CLAUDE.md` (if keeping the SDK)

- [ ] **Step 1: Decide: keep or remove**

The AI service (`packages/api/src/ai/service.ts`) uses the Vercel AI SDK (`@ai-sdk/openai`, `ai`) for three routes: `/ai/summarize`, `/ai/suggest-connections`, `/ai/generate`. CLAUDE.md says "vercel-ai SDK was removed" but it's still active.

**Option A (keep it):** Update CLAUDE.md to reflect reality — the Vercel AI SDK is used for OpenAI-based features alongside Ollama for local enrichment/embeddings.

In `CLAUDE.md`, update the AI line under Tech Stack:
```markdown
- AI: Ollama (local LLM for enrichment + embeddings), OpenAI via Vercel AI SDK (optional, for summarization/suggestions)
```

**Option B (remove it):** Delete `packages/api/src/ai/service.ts`, `packages/api/src/ai/routes.ts`, remove the `aiRoutes` import from `packages/api/src/index.ts`, and remove `"ai"` and `"@ai-sdk/openai"` from `packages/api/package.json`. Also remove `OPENAI_API_KEY` and `OPENAI_MODEL` from `packages/env/src/server.ts`.

**Choose based on whether the OpenAI features are used.** If unsure, go with Option A (document reality).

- [ ] **Step 2: Apply chosen option**

If Option A, update the CLAUDE.md line and commit. If Option B, remove the files and dependencies.

- [ ] **Step 3: Commit**

```bash
git add -A
git commit -m "docs: update CLAUDE.md to reflect actual Vercel AI SDK usage"
# OR
git commit -m "refactor: remove unused Vercel AI SDK code and dependencies"
```

---

## Phase 5: Frontend UX Improvements

### Task 13: Fix SmartLinkProvider to lazy-load chunk index

**Files:**
- Modify: `apps/web/src/components/smart-link-provider.tsx`

- [ ] **Step 1: Gate the 2000-chunk fetch behind markdown-rendering routes**

In `apps/web/src/components/smart-link-provider.tsx`, the `useQuery` that fetches all chunks runs unconditionally at the root. Add an `enabled` flag so it only fetches when needed.

Find the query that fetches chunks (around line 162-166). Add `enabled: false` by default, then create a context to enable it:

```typescript
// Add a hook for routes to signal they need smart links
const SmartLinkContext = createContext({ enabled: false });

export function useEnableSmartLinks() {
    return useContext(SmartLinkContext);
}
```

Alternatively, a simpler approach: check if the current route is one that renders markdown content:

```typescript
import { useLocation } from "@tanstack/react-router";

// Inside the provider component:
const location = useLocation();
const needsSmartLinks = /^\/(chunks|dashboard|plans|requirements)/.test(location.pathname);

// In the query:
const { data: chunks } = useQuery({
    queryKey: ["chunks-for-smart-links"],
    queryFn: ...,
    enabled: needsSmartLinks,
    staleTime: 5 * 60 * 1000, // 5 minutes — these don't change often
    gcTime: 10 * 60 * 1000,
});
```

- [ ] **Step 2: Verify smart links still work on chunk pages**

Navigate to `/chunks/:id` — smart links in markdown content should still resolve.
Navigate to `/settings` — no chunk fetch should fire (check Network tab).

- [ ] **Step 3: Commit**

```bash
git add apps/web/src/components/smart-link-provider.tsx
git commit -m "perf: lazy-load SmartLinkProvider chunk index only on routes that render markdown"
```

---

### Task 14: Fix dashboard loading states to prevent layout shift

**Files:**
- Modify: `apps/web/src/features/dashboard/stats-bar.tsx`
- Modify: `apps/web/src/features/dashboard/active-plan-card.tsx`

- [ ] **Step 1: Replace `return null` with skeleton in StatsBar**

In `apps/web/src/features/dashboard/stats-bar.tsx`, find the `return null` during loading (around line 29). Replace with a skeleton placeholder that preserves the layout:

```typescript
// Before:
if (isLoading) return null;

// After:
if (isLoading) {
    return (
        <div className="flex items-center gap-6 text-sm text-muted-foreground animate-pulse">
            <div className="h-4 w-20 rounded bg-muted" />
            <div className="h-4 w-20 rounded bg-muted" />
            <div className="h-4 w-20 rounded bg-muted" />
        </div>
    );
}
```

- [ ] **Step 2: Replace `return null` with skeleton in ActivePlanCard**

In `apps/web/src/features/dashboard/active-plan-card.tsx`, find the `return null` during loading (around line 75). Replace with a card-shaped skeleton:

```typescript
// Before:
if (inProgressQuery.isLoading) return null;

// After:
if (inProgressQuery.isLoading) {
    return (
        <div className="rounded-lg border p-4 animate-pulse">
            <div className="h-5 w-40 rounded bg-muted mb-3" />
            <div className="h-4 w-full rounded bg-muted mb-2" />
            <div className="h-4 w-2/3 rounded bg-muted" />
        </div>
    );
}
```

- [ ] **Step 3: Verify dashboard loads without layout shift**

Run: `pnpm dev`, navigate to `/dashboard`, observe that content areas show skeletons instead of jumping.

- [ ] **Step 4: Commit**

```bash
git add apps/web/src/features/dashboard/stats-bar.tsx apps/web/src/features/dashboard/active-plan-card.tsx
git commit -m "fix(ux): replace return-null loading states with skeleton placeholders on dashboard"
```

---

### Task 15: Add error boundaries around dashboard widgets

**Files:**
- Modify: `apps/web/src/routes/dashboard.tsx`

- [ ] **Step 1: Import the existing ErrorBoundary component**

In `apps/web/src/routes/dashboard.tsx`, add import:

```typescript
import { ErrorBoundary } from "../components/error-boundary";
```

- [ ] **Step 2: Wrap each independent dashboard section**

Find the JSX where `StatsBar`, `ActivePlanCard`, and the main feed/content are rendered. Wrap each in an `<ErrorBoundary>` with a minimal fallback:

```tsx
<ErrorBoundary fallback={<div className="text-sm text-muted-foreground p-4">Failed to load stats</div>}>
    <StatsBar />
</ErrorBoundary>

<ErrorBoundary fallback={<div className="text-sm text-muted-foreground p-4">Failed to load active plan</div>}>
    <ActivePlanCard />
</ErrorBoundary>
```

This way, a crash in one widget doesn't take down the entire dashboard.

- [ ] **Step 3: Add componentDidCatch logging to ErrorBoundary**

In `apps/web/src/components/error-boundary.tsx`, add the missing lifecycle method:

```typescript
componentDidCatch(error: Error, errorInfo: React.ErrorInfo) {
    console.error("[ErrorBoundary]", error, errorInfo.componentStack);
}
```

- [ ] **Step 4: Commit**

```bash
git add apps/web/src/routes/dashboard.tsx apps/web/src/components/error-boundary.tsx
git commit -m "fix(ux): add per-widget error boundaries on dashboard, add error logging to ErrorBoundary"
```

---

### Task 16: Add optimistic update for favorites toggle

**Files:**
- Modify: `apps/web/src/features/chunks/use-favorites.ts`

- [ ] **Step 1: Add optimistic update logic to the toggle mutation**

In `apps/web/src/features/chunks/use-favorites.ts`, find the mutation that toggles a favorite. Update it following the same pattern as `use-bulk-chunk-operations.ts` (the only existing optimistic update):

```typescript
const toggleFavorite = useMutation({
    mutationFn: async ({ chunkId, isFavorited }: { chunkId: string; isFavorited: boolean }) => {
        if (isFavorited) {
            await api.favorites[":id"].delete({ params: { id: chunkId } });
        } else {
            await api.favorites.post({ body: { chunkId } });
        }
    },
    onMutate: async ({ chunkId, isFavorited }) => {
        // Cancel in-flight queries
        await queryClient.cancelQueries({ queryKey: ["favorites"] });

        // Snapshot previous state
        const previousFavorites = queryClient.getQueryData(["favorites"]);

        // Optimistically update
        queryClient.setQueryData(["favorites"], (old: string[] | undefined) => {
            if (!old) return old;
            return isFavorited
                ? old.filter(id => id !== chunkId)
                : [...old, chunkId];
        });

        return { previousFavorites };
    },
    onError: (_err, _vars, context) => {
        // Roll back on error
        if (context?.previousFavorites) {
            queryClient.setQueryData(["favorites"], context.previousFavorites);
        }
    },
    onSettled: () => {
        queryClient.invalidateQueries({ queryKey: ["favorites"] });
    },
});
```

**Important:** Read the actual file first to understand the current shape of the favorites data and mutation. Adapt the query key, data shape, and API calls to match what's actually there.

- [ ] **Step 2: Verify the star toggle feels instant**

Navigate to `/chunks`, click the favorite star on a chunk. It should toggle immediately without waiting for the server response.

- [ ] **Step 3: Commit**

```bash
git add apps/web/src/features/chunks/use-favorites.ts
git commit -m "feat(ux): add optimistic update for favorites toggle for instant feedback"
```

---

### Task 17: Fix accessibility — label/input associations

**Files:**
- Modify: `apps/web/src/routes/chunks.new.tsx`
- Modify: `apps/web/src/routes/chunks.$chunkId_.edit.tsx`
- Modify: `apps/web/src/routes/requirements_.new.tsx`

- [ ] **Step 1: Fix chunk creation form labels**

In `apps/web/src/routes/chunks.new.tsx`, find all `<label>` elements and add `htmlFor` attributes. Find the corresponding `<Input>` or `<select>` elements and add matching `id` attributes.

Example pattern:
```tsx
// Before:
<label className="mb-1.5 block text-sm font-medium">Title</label>
<Input value={title} onChange={...} />

// After:
<label htmlFor="chunk-title" className="mb-1.5 block text-sm font-medium">Title</label>
<Input id="chunk-title" value={title} onChange={...} />
```

Apply this for all form fields: Title, Content/textarea, Type select, Tags input, and any other labeled inputs.

- [ ] **Step 2: Fix chunk edit form labels**

Apply the same `htmlFor`/`id` pattern in `apps/web/src/routes/chunks.$chunkId_.edit.tsx`.

- [ ] **Step 3: Fix requirement creation form labels**

Apply the same pattern in `apps/web/src/routes/requirements_.new.tsx`.

- [ ] **Step 4: Add aria-label to view toggle buttons**

In `apps/web/src/routes/chunks.index.tsx`, find the List/Grid/Kanban view toggle buttons (around lines 589-604). Add `aria-label` and `aria-pressed` attributes:

```tsx
<button
    aria-label="List view"
    aria-pressed={view === "list"}
    onClick={() => setView("list")}
>
    <ListIcon />
</button>
```

- [ ] **Step 5: Add aria-expanded to mobile nav**

In `apps/web/src/features/nav/mobile-nav.tsx`, find the "Manage" collapsible button (around line 86). Add `aria-expanded`:

```tsx
<button
    aria-expanded={manageOpen}
    onClick={() => setManageOpen(!manageOpen)}
>
    Manage
</button>
```

- [ ] **Step 6: Commit**

```bash
git add apps/web/src/routes/chunks.new.tsx apps/web/src/routes/chunks.\$chunkId_.edit.tsx apps/web/src/routes/requirements_.new.tsx apps/web/src/routes/chunks.index.tsx apps/web/src/features/nav/mobile-nav.tsx
git commit -m "fix(a11y): associate labels with inputs, add aria attributes to toggle buttons and nav"
```

---

### Task 18: Add route-level data prefetching for key routes

**Files:**
- Modify: `apps/web/src/routes/dashboard.tsx`
- Modify: `apps/web/src/routes/chunks.index.tsx`
- Modify: `apps/web/src/routes/chunks.$chunkId.tsx`

- [ ] **Step 1: Add prefetch to dashboard route**

In `apps/web/src/routes/dashboard.tsx`, find the route definition (likely `createFileRoute`). Add a `loader` that prefetches the stats and recent chunks queries:

```typescript
export const Route = createFileRoute("/dashboard")({
    beforeLoad: ({ context }) => {
        // existing auth check
    },
    loader: ({ context }) => {
        const queryClient = context.queryClient;
        // Prefetch stats — start fetching before the component renders
        queryClient.ensureQueryData({
            queryKey: ["stats"],
            queryFn: () => api.stats.get().then(r => r.data),
            staleTime: 60_000,
        });
    },
    component: DashboardPage,
});
```

**Important:** Check how `queryClient` is passed through the router context. Look at `apps/web/src/router.tsx` for the context shape. Adapt accordingly.

- [ ] **Step 2: Add prefetch to chunk detail route**

In `apps/web/src/routes/chunks.$chunkId.tsx`, add a loader that prefetches the chunk detail:

```typescript
loader: ({ params, context }) => {
    context.queryClient.ensureQueryData({
        queryKey: ["chunks", params.chunkId],
        queryFn: () => api.chunks[":id"].get({ params: { id: params.chunkId } }).then(r => r.data),
        staleTime: 60_000,
    });
},
```

- [ ] **Step 3: Verify faster navigations**

Run: `pnpm dev`, navigate between routes. Dashboard and chunk detail should load faster because data fetching starts during navigation, not after render.

- [ ] **Step 4: Commit**

```bash
git add apps/web/src/routes/dashboard.tsx apps/web/src/routes/chunks.\$chunkId.tsx
git commit -m "perf: add route-level data prefetching for dashboard and chunk detail"
```

---

### Task 19: Extract ChunkRow component from chunks.index.tsx

**Files:**
- Create: `apps/web/src/features/chunks/chunk-row.tsx`
- Modify: `apps/web/src/routes/chunks.index.tsx`

- [ ] **Step 1: Identify the row rendering code**

In `apps/web/src/routes/chunks.index.tsx`, find the chunk row rendering JSX. It appears twice — once for the flat view and once for the grouped view (each ~200 lines). The two blocks are nearly identical.

- [ ] **Step 2: Extract ChunkRow component**

Create `apps/web/src/features/chunks/chunk-row.tsx` with a memoized component:

```tsx
import { memo } from "react";
// ... import needed types and components

interface ChunkRowProps {
    chunk: ChunkListItem;
    isSelected: boolean;
    onSelect: (id: string) => void;
    onNavigate: (id: string) => void;
    onToggleFavorite: (id: string, isFavorited: boolean) => void;
    onInlineEdit?: (id: string, title: string) => void;
    // ... other needed props
}

export const ChunkRow = memo(function ChunkRow({
    chunk,
    isSelected,
    onSelect,
    onNavigate,
    onToggleFavorite,
    onInlineEdit,
}: ChunkRowProps) {
    // Move the row JSX here — the single source of truth for row rendering
    return (
        // ... row JSX
    );
});
```

- [ ] **Step 3: Replace both row-rendering blocks in chunks.index.tsx**

Import and use `<ChunkRow>` in both the flat and grouped views, eliminating the duplication:

```tsx
import { ChunkRow } from "../features/chunks/chunk-row";

// In both the flat view and grouped view:
{chunks.map(chunk => (
    <ChunkRow
        key={chunk.id}
        chunk={chunk}
        isSelected={selectedIds.has(chunk.id)}
        onSelect={handleSelect}
        onNavigate={handleNavigate}
        onToggleFavorite={handleToggleFavorite}
    />
))}
```

- [ ] **Step 4: Verify list behavior**

Run: `pnpm dev`, navigate to `/chunks`. Verify:
- Row rendering works in both flat and grouped views
- Selection, inline editing, favorite toggling all work
- Hover prefetch still works

- [ ] **Step 5: Commit**

```bash
git add apps/web/src/features/chunks/chunk-row.tsx apps/web/src/routes/chunks.index.tsx
git commit -m "refactor: extract ChunkRow component, deduplicate row rendering, add React.memo"
```

---

### Task 20: Fix ActivePlanCard query waterfall

**Files:**
- Modify: `apps/web/src/features/dashboard/active-plan-card.tsx`

- [ ] **Step 1: Fetch both plan statuses in parallel**

In `apps/web/src/features/dashboard/active-plan-card.tsx`, find the two sequential queries (around lines 45-55): `plans-in-progress` runs first, then `plans-ready` only if the first returns empty (`enabled: !inProgressQuery.isLoading && !inProgressPlans.length`).

Change to fetch both in parallel:

```typescript
const inProgressQuery = useQuery({
    queryKey: ["plans", "in-progress"],
    queryFn: () => api.plans.get({ query: { status: "in_progress" } }).then(r => r.data),
});

const readyQuery = useQuery({
    queryKey: ["plans", "ready"],
    queryFn: () => api.plans.get({ query: { status: "ready" } }).then(r => r.data),
});

// Prefer in_progress, fall back to ready
const activePlan = inProgressQuery.data?.[0] ?? readyQuery.data?.[0] ?? null;
const isLoading = inProgressQuery.isLoading || readyQuery.isLoading;
```

- [ ] **Step 2: Verify dashboard loads faster**

The active plan card should now resolve in one network round-trip instead of two sequential ones.

- [ ] **Step 3: Commit**

```bash
git add apps/web/src/features/dashboard/active-plan-card.tsx
git commit -m "perf: fetch in-progress and ready plans in parallel to eliminate query waterfall"
```

---

## Summary

| Phase | Tasks | Focus |
|-------|-------|-------|
| 1 | 1-3 | Critical security & safety |
| 2 | 4-8 | Infrastructure & DX |
| 3 | 9-11 | Validation & architecture |
| 4 | 12 | Dead code cleanup |
| 5 | 13-20 | Frontend UX |

**Total: 20 tasks, ~60 commits**

Tasks within each phase are independent and can be parallelized via subagent-driven development.

Tasks across phases can also be parallelized — the only dependency is that Task 5 (logger consolidation) should happen before Task 4 (tracing activation) since Task 4 references the logger.
