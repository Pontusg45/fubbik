# Project Hardening Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Harden fubbik for production readiness — error handling, testing, logging, validation, docs, and deployment fixes.

**Architecture:** Bottom-up approach: fix foundational layers first (env, db, auth), then API, then frontend, then deployment. Each task is
self-contained with tests. CLI improvements are deferred to a separate plan since they require a design decision (local-first vs
API-connected).

**Tech Stack:** Bun, Elysia, Drizzle ORM, TanStack Start/Router/Query, Vitest, Better Auth, Arktype, Docker

---

## Phase 1: Foundation (env, db, config fixes)

### Task 1: Fix server tsconfig — remove unnecessary jsx

**Files:**

- Modify: `apps/server/tsconfig.json`

**Step 1: Remove jsx config**

In `apps/server/tsconfig.json`, remove the `"jsx": "react-jsx"` line. The server has no React code.

```json
{
    "extends": "@fubbik/config/tsconfig.base.json",
    "compilerOptions": {
        "composite": true,
        "outDir": "dist",
        "paths": {
            "@/*": ["./src/*"]
        }
    },
    "include": ["src"]
}
```

**Step 2: Verify type checking still passes**

Run: `cd apps/server && bun run check-types` Expected: No errors

**Step 3: Commit**

```bash
git add apps/server/tsconfig.json
git commit -m "fix: remove unnecessary jsx config from server tsconfig"
```

---

### Task 2: Add PORT to server environment config

**Files:**

- Modify: `packages/env/src/server.ts`
- Modify: `apps/server/src/index.ts`

**Step 1: Add PORT to env schema**

In `packages/env/src/server.ts`, add PORT with a default of 3000:

```typescript
// Add to the server section of createEnv:
PORT: type("string.integer.parse").pipe(type("number >= 1 & number <= 65535")),

// Add to runtimeEnv:
PORT: process.env.PORT ?? "3000",
```

If arktype pipe is too complex, use a simpler approach:

```typescript
PORT: type("string"),

// runtimeEnv:
PORT: process.env.PORT ?? "3000",
```

**Step 2: Use env.PORT in server**

In `apps/server/src/index.ts`, replace the hardcoded port:

```typescript
.listen(Number(env.PORT), () => {
  console.log(`Server running at http://localhost:${env.PORT}`);
});
```

**Step 3: Verify server starts**

Run: `cd apps/server && bun run dev` Expected: Server starts on port 3000 (default)

**Step 4: Commit**

```bash
git add packages/env/src/server.ts apps/server/src/index.ts
git commit -m "feat: make server port configurable via PORT env var"
```

---

### Task 3: Add database migrations tracking

**Files:**

- Modify: `packages/db/package.json`
- Create: `packages/db/drizzle.config.ts` (if not exists, check first)

**Step 1: Verify drizzle config exists**

Check if `packages/db/drizzle.config.ts` exists and has a migrations directory configured. If not, ensure it has:

```typescript
import { defineConfig } from "drizzle-kit";

export default defineConfig({
    schema: "./src/schema/index.ts",
    out: "./src/migrations",
    dialect: "postgresql",
    dbCredentials: {
        url: process.env.DATABASE_URL!
    }
});
```

**Step 2: Generate initial migration**

Run: `cd packages/db && bun run db:generate` Expected: Migration files created in `packages/db/src/migrations/`

**Step 3: Add migrate script**

In `packages/db/package.json`, verify `db:migrate` script exists. If not, add:

```json
"db:migrate": "drizzle-kit migrate"
```

**Step 4: Commit migrations**

```bash
git add packages/db/src/migrations/ packages/db/drizzle.config.ts packages/db/package.json
git commit -m "feat: add tracked database migrations"
```

---

## Phase 2: API Error Handling & Logging

### Task 4: Add structured logging with Winston

**Files:**

- Create: `apps/server/src/logger.ts`
- Modify: `apps/server/src/index.ts`

**Step 1: Create logger module**

```typescript
// apps/server/src/logger.ts
import winston from "winston";
import { env } from "@fubbik/env/server";

export const logger = winston.createLogger({
    level: env.NODE_ENV === "production" ? "info" : "debug",
    format: winston.format.combine(
        winston.format.timestamp(),
        env.NODE_ENV === "production" ? winston.format.json() : winston.format.combine(winston.format.colorize(), winston.format.simple())
    ),
    transports: [new winston.transports.Console()]
});
```

**Step 2: Add request logging middleware to server**

In `apps/server/src/index.ts`, add logging after CORS:

```typescript
import { logger } from "./logger";

// After .use(cors(...)):
.onRequest(({ request }) => {
  logger.info(`${request.method} ${new URL(request.url).pathname}`);
})
.onError(({ error, request }) => {
  logger.error(`${request.method} ${new URL(request.url).pathname}`, {
    error: error.message,
  });
})
```

**Step 3: Verify logging works**

Run: `cd apps/server && bun run dev` Hit: `curl http://localhost:3000/api/health` Expected: See structured log output in console

**Step 4: Commit**

```bash
git add apps/server/src/logger.ts apps/server/src/index.ts
git commit -m "feat: add structured logging with Winston"
```

---

### Task 5: Add error handling to all API database queries

**Files:**

- Modify: `packages/api/src/index.ts`
- Create: `packages/api/src/error.ts`

**Step 1: Create error helper**

```typescript
// packages/api/src/error.ts
export function dbError(set: { status: number }, message: string, err: unknown) {
    set.status = 500;
    console.error(message, err);
    return { message: "Internal server error" };
}
```

**Step 2: Wrap all DB calls in try-catch**

In `packages/api/src/index.ts`, wrap every database operation. Example for GET `/chunks`:

```typescript
.get("/chunks", async ({ session, set, query }) => {
  if (!session) {
    set.status = 401;
    return { message: "Authentication required" };
  }
  try {
    const conditions = [eq(chunk.userId, session.user.id)];
    // ... existing query logic ...
    return { chunks, total: Number(total[0]?.count ?? 0) };
  } catch (err) {
    return dbError(set, "Failed to fetch chunks", err);
  }
}, /* ... */)
```

Apply the same pattern to: GET `/chunks/:id`, POST `/chunks`, PATCH `/chunks/:id`, DELETE `/chunks/:id`, GET `/stats`.

**Step 3: Verify error handling works**

Run: `cd apps/server && bun run dev` Expected: Normal requests still work. DB errors return 500 with "Internal server error".

**Step 4: Commit**

```bash
git add packages/api/src/error.ts packages/api/src/index.ts
git commit -m "feat: add error handling to all API database queries"
```

---

### Task 6: Improve health check to verify DB connectivity

**Files:**

- Modify: `packages/api/src/index.ts`

**Step 1: Update health endpoint**

Replace the health check in the API:

```typescript
.get("/health", async ({ set }) => {
  try {
    await db.execute(sql`SELECT 1`);
    return { status: "ok", db: "connected" };
  } catch {
    set.status = 503;
    return { status: "degraded", db: "disconnected" };
  }
})
```

Import `sql` from `drizzle-orm` and `db` from `@fubbik/db` if not already imported.

**Step 2: Verify**

Run: `curl http://localhost:3000/api/health` Expected: `{ "status": "ok", "db": "connected" }`

**Step 3: Commit**

```bash
git add packages/api/src/index.ts
git commit -m "feat: health check verifies database connectivity"
```

---

### Task 7: Add pagination to chunks list endpoint

**Files:**

- Modify: `packages/api/src/index.ts`

**Step 1: Add offset parameter**

Update the GET `/chunks` query schema and logic:

```typescript
.get(
  "/chunks",
  async ({ session, set, query }) => {
    if (!session) { set.status = 401; return { message: "Authentication required" }; }
    try {
      const limit = Math.min(Number(query.limit ?? 50), 100);
      const offset = Number(query.offset ?? 0);
      const conditions = [eq(chunk.userId, session.user.id)];
      if (query.type) conditions.push(eq(chunk.type, query.type));
      if (query.search) {
        conditions.push(
          or(ilike(chunk.title, `%${query.search}%`), ilike(chunk.content, `%${query.search}%`))!,
        );
      }
      const chunks = await db
        .select().from(chunk)
        .where(and(...conditions))
        .orderBy(desc(chunk.updatedAt))
        .limit(limit)
        .offset(offset);
      const total = await db
        .select({ count: sql<number>`count(*)` })
        .from(chunk)
        .where(and(...conditions));
      return { chunks, total: Number(total[0]?.count ?? 0), limit, offset };
    } catch (err) {
      return dbError(set, "Failed to fetch chunks", err);
    }
  },
  {
    query: t.Object({
      type: t.Optional(t.String()),
      search: t.Optional(t.String()),
      limit: t.Optional(t.String()),
      offset: t.Optional(t.String()),
    }),
  },
)
```

**Step 2: Commit**

```bash
git add packages/api/src/index.ts
git commit -m "feat: add offset pagination to chunks list endpoint"
```

---

## Phase 3: Testing

### Task 8: Write API integration tests for chunk CRUD

**Files:**

- Create: `packages/api/src/index.test.ts`

**Step 1: Write test file**

```typescript
// packages/api/src/index.test.ts
import { describe, expect, it } from "vitest";
import { treaty } from "@elysiajs/eden";
import { Elysia } from "elysia";
import { api } from "./index";

// Mount API on a test server
const app = new Elysia().use(api);
const client = treaty(app);

describe("GET /api/health", () => {
    it("returns ok status", async () => {
        const { data, status } = await client.api.health.get();
        expect(status).toBe(200);
        expect(data?.status).toBe("ok");
    });
});

describe("Chunks API (unauthenticated)", () => {
    it("GET /api/chunks returns 401", async () => {
        const { status } = await client.api.chunks.get();
        expect(status).toBe(401);
    });

    it("POST /api/chunks returns 401", async () => {
        const { status } = await client.api.chunks.post({
            title: "Test"
        });
        expect(status).toBe(401);
    });
});
```

Note: Full authenticated tests require a test database and dev user. For now, test unauthenticated behavior and health. Authenticated tests
can use the DEV_SESSION bypass when NODE_ENV is not production.

**Step 2: Run tests**

Run: `cd packages/api && bun test` Expected: Tests pass

**Step 3: Commit**

```bash
git add packages/api/src/index.test.ts
git commit -m "test: add API integration tests for health and auth guards"
```

---

### Task 9: Write database schema tests

**Files:**

- Create: `packages/db/src/schema/chunk.test.ts`

**Step 1: Write schema validation tests**

```typescript
// packages/db/src/schema/chunk.test.ts
import { describe, expect, it } from "vitest";
import { getTableColumns } from "drizzle-orm";
import { chunk, chunkConnection } from "./chunk";

describe("chunk table", () => {
    it("has expected columns", () => {
        const columns = getTableColumns(chunk);
        expect(columns).toHaveProperty("id");
        expect(columns).toHaveProperty("title");
        expect(columns).toHaveProperty("content");
        expect(columns).toHaveProperty("type");
        expect(columns).toHaveProperty("tags");
        expect(columns).toHaveProperty("userId");
        expect(columns).toHaveProperty("createdAt");
        expect(columns).toHaveProperty("updatedAt");
    });
});

describe("chunkConnection table", () => {
    it("has expected columns", () => {
        const columns = getTableColumns(chunkConnection);
        expect(columns).toHaveProperty("id");
        expect(columns).toHaveProperty("sourceId");
        expect(columns).toHaveProperty("targetId");
        expect(columns).toHaveProperty("relation");
        expect(columns).toHaveProperty("createdAt");
    });
});
```

**Step 2: Add test script to db package**

In `packages/db/package.json`, add:

```json
"test": "vitest run --passWithNoTests"
```

And add vitest to devDependencies if missing.

**Step 3: Run tests**

Run: `cd packages/db && bun test` Expected: Tests pass

**Step 4: Commit**

```bash
git add packages/db/src/schema/chunk.test.ts packages/db/package.json
git commit -m "test: add database schema structure tests"
```

---

### Task 10: Remove --passWithNoTests from apps that now have tests

**Files:**

- Modify: `apps/server/package.json`
- Modify: `apps/web/package.json`

**Step 1: Keep --passWithNoTests for now**

Actually, keep this flag until each app has its own tests. This task is a reminder to remove it later. Skip for now.

---

## Phase 4: Frontend Hardening

### Task 11: Add error boundary to root layout

**Files:**

- Create: `apps/web/src/components/error-boundary.tsx`
- Modify: `apps/web/src/routes/__root.tsx`

**Step 1: Create error boundary component**

```tsx
// apps/web/src/components/error-boundary.tsx
import { Component, type ReactNode } from "react";
import { Button } from "@/components/ui/button";

interface Props {
    children: ReactNode;
    fallback?: ReactNode;
}

interface State {
    hasError: boolean;
    error: Error | null;
}

export class ErrorBoundary extends Component<Props, State> {
    constructor(props: Props) {
        super(props);
        this.state = { hasError: false, error: null };
    }

    static getDerivedStateFromError(error: Error): State {
        return { hasError: true, error };
    }

    render() {
        if (this.state.hasError) {
            return (
                this.props.fallback ?? (
                    <div className="flex min-h-[50vh] flex-col items-center justify-center gap-4">
                        <h2 className="text-lg font-semibold">Something went wrong</h2>
                        <p className="text-muted-foreground text-sm">{this.state.error?.message ?? "An unexpected error occurred."}</p>
                        <Button onClick={() => this.setState({ hasError: false, error: null })}>Try again</Button>
                    </div>
                )
            );
        }
        return this.props.children;
    }
}
```

**Step 2: Wrap the Outlet in \_\_root.tsx with ErrorBoundary**

In `apps/web/src/routes/__root.tsx`, wrap `<Outlet />`:

```tsx
import { ErrorBoundary } from "@/components/error-boundary";

// Inside the layout JSX, wrap <Outlet />:
<main className="min-h-0">
    <ErrorBoundary>
        <Outlet />
    </ErrorBoundary>
</main>;
```

**Step 3: Verify**

Run: `cd apps/web && bun run dev` Expected: App renders normally. Errors in routes show fallback UI instead of crashing.

**Step 4: Commit**

```bash
git add apps/web/src/components/error-boundary.tsx apps/web/src/routes/__root.tsx
git commit -m "feat: add error boundary to root layout"
```

---

### Task 12: Add form validation to chunk creation

**Files:**

- Modify: `apps/web/src/routes/chunks.new.tsx`

**Step 1: Add validation state and rules**

Add validation to the existing form in `chunks.new.tsx`. Keep using local state (TanStack Form would be a larger refactor). Add:

```tsx
const [errors, setErrors] = useState<Record<string, string>>({});

function validate() {
    const e: Record<string, string> = {};
    if (!title.trim()) e.title = "Title is required";
    else if (title.length > 200) e.title = "Title must be 200 characters or less";
    if (content.length > 50000) e.content = "Content must be 50,000 characters or less";
    setErrors(e);
    return Object.keys(e).length === 0;
}
```

**Step 2: Update the mutation call**

```tsx
onClick={() => {
  if (validate()) createMutation.mutate();
}}
```

**Step 3: Add error display below each field**

After each input, add:

```tsx
{
    errors.title && <p className="text-destructive text-xs mt-1">{errors.title}</p>;
}
```

**Step 4: Verify**

Run: `cd apps/web && bun run dev`, navigate to `/chunks/new` Expected: Submit with empty title shows error. Long title shows error.

**Step 5: Commit**

```bash
git add apps/web/src/routes/chunks.new.tsx
git commit -m "feat: add form validation to chunk creation"
```

---

### Task 13: Wire up dark mode toggle with next-themes

**Files:**

- Modify: `apps/web/src/routes/__root.tsx`
- Create: `apps/web/src/components/theme-provider.tsx`
- Create: `apps/web/src/components/theme-toggle.tsx`

**Step 1: Create theme provider**

```tsx
// apps/web/src/components/theme-provider.tsx
import { ThemeProvider as NextThemeProvider } from "next-themes";
import type { ReactNode } from "react";

export function ThemeProvider({ children }: { children: ReactNode }) {
    return (
        <NextThemeProvider attribute="class" defaultTheme="dark" enableSystem>
            {children}
        </NextThemeProvider>
    );
}
```

**Step 2: Create theme toggle button**

```tsx
// apps/web/src/components/theme-toggle.tsx
import { Moon, Sun } from "lucide-react";
import { useTheme } from "next-themes";
import { Button } from "@/components/ui/button";

export function ThemeToggle() {
    const { theme, setTheme } = useTheme();
    return (
        <Button variant="ghost" size="icon" onClick={() => setTheme(theme === "dark" ? "light" : "dark")}>
            <Sun className="size-4 rotate-0 scale-100 transition-all dark:-rotate-90 dark:scale-0" />
            <Moon className="absolute size-4 rotate-90 scale-0 transition-all dark:rotate-0 dark:scale-100" />
            <span className="sr-only">Toggle theme</span>
        </Button>
    );
}
```

**Step 3: Update root layout**

In `apps/web/src/routes/__root.tsx`:

- Remove hardcoded `className="dark"` from `<html>`
- Wrap the layout in `<ThemeProvider>`
- Add `<ThemeToggle />` to the header (next to UserMenu)

**Step 4: Verify**

Run: `cd apps/web && bun run dev` Expected: Toggle switches between light and dark mode.

**Step 5: Commit**

```bash
git add apps/web/src/components/theme-provider.tsx apps/web/src/components/theme-toggle.tsx apps/web/src/routes/__root.tsx
git commit -m "feat: add dark/light mode toggle with next-themes"
```

---

## Phase 5: Deployment Fixes

### Task 14: Fix docker-compose port mapping

**Files:**

- Modify: `docker-compose.yml`

**Step 1: Fix port conflicts**

The web and server services both use port 3000 internally. Update the docker-compose.yml:

```yaml
web:
    ports:
        - "3001:3000" # Web on host port 3001

server:
    ports:
        - "3000:3000" # API on host port 3000
```

**Step 2: Add healthchecks**

Add healthcheck to server service:

```yaml
server:
    healthcheck:
        test: ["CMD", "curl", "-f", "http://localhost:3000/api/health"]
        interval: 10s
        timeout: 5s
        retries: 3
```

**Step 3: Commit**

```bash
git add docker-compose.yml
git commit -m "fix: resolve docker-compose port conflicts and add healthchecks"
```

---

### Task 15: Add Swagger/OpenAPI documentation

**Files:**

- Modify: `apps/server/package.json` (add @elysiajs/swagger dependency)
- Modify: `apps/server/src/index.ts`

**Step 1: Install swagger plugin**

Run: `cd apps/server && bun add @elysiajs/swagger`

**Step 2: Add swagger middleware**

In `apps/server/src/index.ts`:

```typescript
import { swagger } from "@elysiajs/swagger";

new Elysia()
    .use(
        swagger({
            path: "/docs",
            documentation: {
                info: { title: "Fubbik API", version: "0.1.0" }
            }
        })
    )
    .use(cors(/* ... */));
// ... rest of server setup
```

**Step 3: Verify**

Run: `cd apps/server && bun run dev` Navigate to: `http://localhost:3000/docs` Expected: Swagger UI with all endpoints listed

**Step 4: Commit**

```bash
git add apps/server/package.json apps/server/src/index.ts bun.lock
git commit -m "feat: add Swagger API documentation at /docs"
```

---

## Phase 6: Documentation

### Task 16: Update CLAUDE.md and README

**Files:**

- Modify: `CLAUDE.md`
- Modify: `README.md` (if it references TRPC or has incorrect info)

**Step 1: Review both files for accuracy**

Read both files. Fix any references to TRPC (project uses Eden treaty, not TRPC). Update:

- Tech stack should mention Eden treaty, not TRPC
- Add `bun ci` to common commands
- Add `PORT` env var documentation
- Note that `tsgo` is used for type checking
- Add `/docs` Swagger endpoint mention

**Step 2: Commit**

```bash
git add CLAUDE.md README.md
git commit -m "docs: update project documentation to match current stack"
```

---

## Phase 7: Security Hardening

### Task 17: Add rate limiting to API

**Files:**

- Modify: `apps/server/package.json`
- Modify: `apps/server/src/index.ts`

**Step 1: Install rate limiter**

Run: `cd apps/server && bun add elysia-rate-limit`

**Step 2: Add rate limiting middleware**

In `apps/server/src/index.ts`:

```typescript
import { rateLimit } from "elysia-rate-limit";

new Elysia()
    .use(swagger(/* ... */))
    .use(
        rateLimit({
            max: 100,
            duration: 60_000 // 100 requests per minute
        })
    )
    .use(cors(/* ... */));
// ...
```

**Step 3: Verify**

Run: `cd apps/server && bun run dev` Expected: Normal requests work. Rapid requests eventually get 429.

**Step 4: Commit**

```bash
git add apps/server/package.json apps/server/src/index.ts bun.lock
git commit -m "feat: add rate limiting to API server"
```

---

### Task 18: Add unique constraint on chunk connections

**Files:**

- Modify: `packages/db/src/schema/chunk.ts`

**Step 1: Add unique index**

Add a unique constraint to prevent duplicate connections:

```typescript
import { pgTable, text, timestamp, jsonb, index, uniqueIndex } from "drizzle-orm/pg-core";

// In chunkConnection table definition, add to the index array:
uniqueIndex("connection_unique_idx").on(table.sourceId, table.targetId, table.relation),
```

**Step 2: Generate migration**

Run: `cd packages/db && bun run db:generate` Expected: New migration file created

**Step 3: Commit**

```bash
git add packages/db/src/schema/chunk.ts packages/db/src/migrations/
git commit -m "feat: add unique constraint on chunk connections"
```

---

## Summary

| Phase | Tasks | Focus                                            |
| ----- | ----- | ------------------------------------------------ |
| 1     | 1-3   | Foundation: config, env, migrations              |
| 2     | 4-7   | API: logging, error handling, health, pagination |
| 3     | 8-10  | Testing: API tests, schema tests                 |
| 4     | 11-13 | Frontend: error boundary, validation, dark mode  |
| 5     | 14-15 | Deployment: docker fix, Swagger                  |
| 6     | 16    | Documentation updates                            |
| 7     | 17-18 | Security: rate limiting, constraints             |

**Not included (separate plans):**

- CLI refactor (local-first vs API-connected — needs design decision)
- E2E tests with Playwright (needs CI pipeline first)
- OAuth provider setup (needs provider credentials)
- OpenTelemetry integration (partially set up, needs observability backend)
