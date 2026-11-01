---
tags:
    - guide
    - architecture
    - backend
description: Backend architecture — repository, service, and route patterns
---

# Backend Architecture

The live backend is implemented in Rust. Axum routes in `crates/fubbik-api` call domain workflows in the same crate, which use SQLx repositories in `crates/fubbik-db`. The retired TypeScript server and API (once `apps/server` and `packages/api`, later `legacy/`) were removed and live only in git history.

## In This Section

- [Layer Pattern](./layers.md) — repository → service → route
- [Error Handling](./error-handling.md) — Rust domain errors and Axum response mapping
