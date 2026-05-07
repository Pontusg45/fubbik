---
tags:
  - guide
  - architecture
  - backend
  - errors
description: Effect-based typed errors and the global error handler
---

# Error Handling

The backend uses Effect for typed error handling. Each error type has a `_tag` discriminator.

## Error Types

- `ValidationError` — invalid input (400)
- `AuthError` — not authenticated or not authorized (401)
- `NotFoundError` — entity doesn't exist (404)
- `DatabaseError` — database operation failed (500)

## Global Error Handler

The global error handler in `packages/api/src/index.ts` extracts Effect errors from `FiberFailure` and maps `_tag` to HTTP status codes:

```typescript
.onError(({ error, set }) => {
  const effectError = extractEffectError(error);
  if (effectError) {
    switch (effectError._tag) {
      case "ValidationError": set.status = 400; break;
      case "AuthError":       set.status = 401; break;
      case "NotFoundError":   set.status = 404; break;
      case "DatabaseError":   set.status = 500; break;
    }
  }
})
```

## Pattern

Services create errors via tagged constructors:

```typescript
yield* Effect.fail(new NotFoundError({ message: "Chunk not found" }));
```

Routes call `Effect.runPromise()` — unhandled errors propagate to the global handler automatically.
