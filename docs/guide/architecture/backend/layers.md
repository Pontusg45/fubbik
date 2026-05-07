---
tags:
  - guide
  - architecture
  - backend
  - patterns
description: The three-layer backend pattern — repository, service, and route
---

# Layer Pattern

## Repository Layer

Located at `packages/db/src/repository/`. Pure data access — functions return `Effect<T, DatabaseError>`. No business logic.

## Service Layer

Located at `packages/api/src/*/service.ts`. Business logic — composes repository Effects, validates inputs, introduces domain errors (`NotFoundError`, `AuthError`, `ValidationError`).

## Route Layer

Located at `packages/api/src/*/routes.ts`. HTTP layer — calls services via `Effect.runPromise()`. Uses Elysia's `t` schema for request validation. Errors propagate to the global error handler.

## Data Flow

```
HTTP Request → Route (validate) → Service (logic) → Repository (data) → PostgreSQL
                                                                        ↓
HTTP Response ← Route (format) ← Service (compose) ← Repository (query) ←
```
