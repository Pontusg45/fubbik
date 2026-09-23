---
tags:
    - guide
    - architecture
    - backend
    - errors
description: Rust domain errors and their HTTP mapping
---

# Error Handling

Rust domain workflows and SQLx repositories return `AppResult<T>` from `fubbik-core`. The `AppError` variants distinguish validation, authentication, missing resources, conflicts, database failures, unsupported media types, and external service failures.

Axum handlers return `ApiResult<T>`. The local `ApiError` adapter in `crates/fubbik-api/src/error.rs` converts `AppError` to an HTTP response: validation → 400, authentication → 401, missing resource → 404, conflict → 409, unsupported media type → 415, external dependency → 502, and database failure → 500 (or 503 for pool exhaustion). Database details are logged server-side and omitted from the response body.

Routes use `?` to propagate errors; domain workflows return an explicit error instead of formatting HTTP responses themselves. This keeps the HTTP mapping in one place.
