# Effect Integration — Design

**Goal:** Replace all try-catch error handling on the server with Effect, giving typed errors from DB to route boundary.

**Decisions:**
- Effect in both DB repositories and API (not server entry)
- Global Elysia `.onError` handler maps `_tag` to HTTP status codes — routes have no error handling
- Error types defined where they originate: `DatabaseError` in db, `NotFoundError`/`AuthError` in api

---

## Error Types

### packages/db/src/errors.ts

```typescript
import { Data } from "effect";

export class DatabaseError extends Data.TaggedError("DatabaseError")<{
  cause: unknown;
}> {}
```

### packages/api/src/errors.ts

```typescript
import { Data } from "effect";

export class NotFoundError extends Data.TaggedError("NotFoundError")<{
  resource: string;
}> {}

export class AuthError extends Data.TaggedError("AuthError")<{}> {}
```

---

## Repositories

All repository functions return `Effect<T, DatabaseError>` instead of `Promise<T>`. DB calls wrapped in `Effect.tryPromise`:

```typescript
export function listChunks(params: ListChunksParams) {
  return Effect.tryPromise({
    try: async () => { /* existing DB logic */ },
    catch: (cause) => new DatabaseError({ cause }),
  });
}
```

Same pattern for all 9 functions across `chunk.ts`, `stats.ts`, `health.ts`.

Add `"effect"` dependency to `packages/db/package.json`.

---

## Services

Services compose repository Effects. Business logic errors added via `Effect.fail`:

- `listChunks` — pipes repo call, returns `Effect<T, DatabaseError>`
- `getChunkDetail` — composes two repo calls, fails with `NotFoundError` if missing
- `createChunk` — pipes to repo
- `updateChunk` — checks existence, fails with `NotFoundError`
- `deleteChunk` — checks result, fails with `NotFoundError`
- `getUserStats` — `Effect.all` (parallel) replaces `Promise.all`

Add `"effect"` dependency to `packages/api/package.json`.

---

## Routes

Routes become thin — extract params, pipe through service, `Effect.runPromise`.

Auth check as Effect:

```typescript
function requireSession(ctx: unknown) {
  const session = (ctx as unknown as { session: Session }).session;
  return session ? Effect.succeed(session) : Effect.fail(new AuthError());
}
```

Route pattern:

```typescript
.get("/chunks", (ctx) =>
  Effect.runPromise(
    requireSession(ctx).pipe(
      Effect.flatMap((s) => chunkService.listChunks(s.user.id, ctx.query))
    )
  ),
  { query: ... }
)
```

POST /chunks sets 201 via `Effect.tap(() => Effect.sync(() => { ctx.set.status = 201; }))`.

---

## Global Error Handler

In `packages/api/src/index.ts`, `.onError` maps `_tag` to HTTP:

- `AuthError` → 401
- `NotFoundError` → 404
- `DatabaseError` → 500 (log cause)

`error.ts` (dbError helper) is deleted.

---

## Health Route

Special case — catches `DatabaseError` to return 503 (degraded), not a thrown error:

```typescript
Effect.runPromise(
  checkDbConnectivity().pipe(
    Effect.match({
      onSuccess: () => ({ status: "ok", db: "connected" }),
      onFailure: () => ({ status: "degraded", db: "disconnected" }),
    })
  )
)
```

---

## Data Flow

```
Route (thin: requireSession → service call → runPromise)
  → Service (composes Effects, adds NotFoundError)
    → Repository (Effect.tryPromise wrapping DB, DatabaseError)
      → Drizzle → Postgres

On failure: Effect.runPromise throws → Elysia .onError → HTTP status by _tag
```

## What Doesn't Change

- Elysia route definitions, validation schemas (t.Object)
- Server entry point (apps/server/src/index.ts)
- Tests (same HTTP behavior, just different internals)
- Web app, CLI
