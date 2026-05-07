---
tags:
  - guide
  - integrations
  - api
description: Programmatic access via the Elysia REST API
---

# REST API

The fubbik API is a REST API built with Elysia. Full OpenAPI/Swagger docs are available at `http://localhost:3000/docs`.

## Key Endpoints

```
GET    /api/chunks                    # List chunks
POST   /api/chunks                    # Create chunk
GET    /api/chunks/:id                # Get chunk detail
GET    /api/chunks/search/semantic    # Semantic search
GET    /api/chunks/search/federated   # Cross-codebase search
GET    /api/chunks/export/context     # Token-budgeted export
GET    /api/context/for-file          # Chunks relevant to a file
GET    /api/context/about             # Semantic concept search
GET    /api/graph                     # Graph data (nodes + edges)
POST   /api/plans                     # Create plan
GET    /api/requirements              # List requirements
```

## Authentication

All endpoints require authentication via Better Auth session cookies. The web app handles auth automatically. For API access from scripts, use the session token.

## Error Handling

The API uses Effect-based error handling. Errors map to HTTP status codes:
- `ValidationError` → 400
- `AuthError` → 401
- `NotFoundError` → 404
- `DatabaseError` → 500
