---
tags:
    - guide
    - architecture
    - frontend
description: Frontend architecture — TanStack Start, React, and UI patterns
---

# Frontend Architecture

The web app uses TanStack Start (SSR) with file-based routing. Route files identify the URL and compose a page; reusable data queries, mutations, form rules, and larger panels belong in `apps/web/src/features/<domain>/`. Feature query functions use the client generated from Rust OpenAPI and adapt optional wire fields before presenting them to UI components.

## In This Section

- [Routing and Data Fetching](./routing.md) — file-based routes and React Query
- [Component Patterns](./components.md) — feature structure, shared UI, and base-ui patterns
