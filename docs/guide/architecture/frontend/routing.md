---
tags:
    - guide
    - architecture
    - frontend
    - routing
description: File-based routing and React Query data fetching
---

# Routing and Data Fetching

## File-Based Routing

Routes live in `apps/web/src/routes/`. Each file maps to a URL path:

- `routes/index.tsx` → `/`
- `routes/chunks.index.tsx` → `/chunks`
- `routes/chunks.$chunkId.tsx` → `/chunks/:chunkId`
- `routes/plans.new.tsx` → `/plans/new`

## Data Fetching

API calls use the client generated from the Rust OpenAPI contract:

```typescript
const { data } = useQuery({
    queryKey: ["chunks", filters],
    queryFn: async () => unwrapEden(await api.api.chunks.get({ query: filters }))
});
```

Mutations use `useMutation` with optimistic updates and cache invalidation.

## Server-Side Rendering

TanStack Start provides SSR via `entry-server.ts`. Initial page loads are server-rendered, then hydrated on the client.
