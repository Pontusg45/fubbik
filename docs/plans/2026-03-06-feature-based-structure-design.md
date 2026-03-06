# Feature-Based Folder Structure — Design

**Goal:** Restructure the monorepo to use feature-based folder organization and split the API into routes → services → repositories.

**Decisions:**
- Repositories (DB query functions) live in `packages/db/src/repository/`
- Services (business logic) + routes live in `packages/api/src/<feature>/`
- Web app uses `src/features/` for feature-specific components; routes stay flat (TanStack Router requirement)
- CLI gets a light restructure: `store.ts` → `src/lib/store.ts`

---

## packages/db — Schema + Repositories

```
packages/db/src/
├── index.ts              (db client export)
├── schema/
│   ├── index.ts
│   ├── auth.ts
│   └── chunk.ts
├── repository/
│   ├── index.ts          (re-exports all repositories)
│   ├── chunk.ts          (findChunksByUser, getChunkById, createChunk, updateChunk, deleteChunk, getChunkConnections)
│   ├── stats.ts          (getChunkCount, getConnectionCount, getTagCount)
│   └── health.ts         (checkDbConnectivity)
├── seed.ts
└── migrations/
```

Each repository exports plain async functions. No Elysia/HTTP awareness — pure data access. Functions take typed params and return typed results.

## packages/api — Routes + Services

```
packages/api/src/
├── index.ts              (Elysia plugin that composes all feature routes)
├── context.ts            (Session type, auth resolve middleware)
├── error.ts              (error helper)
├── chunks/
│   ├── routes.ts         (Elysia route definitions — thin, delegates to service)
│   └── service.ts        (business logic: calls repository, formats responses)
├── stats/
│   ├── routes.ts
│   └── service.ts
├── health/
│   └── routes.ts         (simple — calls repository directly, no service needed)
└── index.test.ts
```

`index.ts` composes feature routes via `.use()`. Route handlers are thin — they extract params and call services. Services handle business logic and call repositories. No direct DB imports in route files.

## apps/web — Feature folders + flat routes

```
apps/web/src/
├── routes/               (TanStack Router — stays flat)
│   ├── __root.tsx
│   ├── index.tsx
│   ├── login.tsx
│   ├── dashboard.tsx
│   ├── chunks.new.tsx
│   └── chunks.$chunkId.tsx
├── features/
│   ├── auth/
│   │   ├── sign-in-form.tsx
│   │   ├── sign-up-form.tsx
│   │   └── user-menu.tsx
│   ├── chunks/
│   │   └── (chunk-specific components as they grow)
│   └── dashboard/
│       └── (dashboard-specific components as they grow)
├── components/           (app-wide shared)
│   ├── error-boundary.tsx
│   ├── error-state.tsx
│   ├── fubbik-logo.tsx
│   ├── loader.tsx
│   ├── not-found.tsx
│   ├── theme-provider.tsx
│   ├── theme-toggle.tsx
│   └── ui/               (shadcn — untouched)
├── hooks/
├── lib/
├── middleware/
├── functions/
└── utils/
```

## apps/cli — Light restructure

```
apps/cli/src/
├── index.ts
├── commands/             (unchanged)
└── lib/
    └── store.ts          (moved from src/store.ts)
```

## Data flow

```
Route (thin) → Service (business logic) → Repository (DB queries) → Drizzle → Postgres
```
