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

Located at `crates/fubbik-db/src/repo/`. SQLx functions own scoped queries and database operations. Multi-write workflows can pass the same transaction connection through repositories.

## Service Layer

Located at `crates/fubbik-api/src/<domain>/service.rs`. Domain workflows validate inputs, compose repository operations, and return typed `AppResult` errors. Workflows that must succeed together own a transaction.

## Route Layer

Located at `crates/fubbik-api/src/<domain>/routes.rs`. Axum routes handle HTTP extraction and response shaping. DTOs in the neighboring `dto.rs` define the wire contract; the Rust OpenAPI document generates the web client.

## Data Flow

```
HTTP Request → Axum route → domain workflow → SQLx repository → PostgreSQL
HTTP Response ← DTO/serializer ← domain result ← query result ←
```
