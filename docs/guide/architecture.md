---
tags:
  - guide
  - architecture
description: Technical architecture and project structure
---

# Architecture

Fubbik is a monorepo with apps and shared packages, built on TypeScript with Bun as the runtime.

## Project Structure

```
fubbik/
├── apps/
│   ├── web/         # Frontend (TanStack Start, React)
│   ├── server/      # API server entry point (Elysia)
│   ├── cli/         # CLI application (Commander.js)
│   └── vscode/      # VS Code extension
├── packages/
│   ├── api/         # API routes, services, business logic
│   ├── auth/        # Authentication (Better Auth)
│   ├── config/      # Shared TypeScript config
│   ├── db/          # Database schema + repositories (Drizzle)
│   ├── env/         # Environment validation
│   └── mcp/         # MCP server for AI agents
└── docs/
    └── guide/       # User documentation
```

## Backend Architecture

The backend follows a three-layer pattern:

**Repository** (`packages/db/src/repository/`) — Pure data access. Functions return `Effect<T, DatabaseError>`. No business logic.

**Service** (`packages/api/src/*/service.ts`) — Business logic. Composes repository Effects, validates inputs, introduces domain errors (`NotFoundError`, `AuthError`, `ValidationError`).

**Routes** (`packages/api/src/*/routes.ts`) — HTTP layer. Calls services via `Effect.runPromise()`. Uses Elysia's `t` schema for request validation. Errors propagate to the global error handler.

The global error handler in `packages/api/src/index.ts` maps Effect error tags to HTTP status codes: `ValidationError` > 400, `AuthError` > 401, `NotFoundError` > 404, `DatabaseError` > 500.

## Frontend Architecture

The web app uses TanStack Start (SSR) with file-based routing:

- **Routes** in `apps/web/src/routes/` — each file is a page
- **Features** in `apps/web/src/features/` — domain-specific components and hooks
- **Shared UI** in `apps/web/src/components/ui/` — shadcn-ui components (base-ui with `render` prop)
- **API calls** via Eden treaty client for type-safe requests
- **Data fetching** via React Query (`useQuery`, `useMutation`)

## Database

PostgreSQL with Drizzle ORM. Key extensions:

- **pgvector** — 768-dimensional embeddings for semantic search
- **pg_trgm** — trigram-based fuzzy text search

Schema is defined in TypeScript at `packages/db/src/schema/`. Migrations use `drizzle-kit push` in development.

## Tech Stack Summary

| Layer | Technology |
|-------|-----------|
| Runtime | Bun |
| Package Manager | pnpm |
| Frontend | TanStack Start, React, Tailwind, shadcn-ui |
| Backend | Elysia, Effect |
| Database | PostgreSQL, Drizzle ORM |
| Auth | Better Auth |
| AI | Ollama (local LLM + embeddings) |
| Testing | Vitest |
| CLI | Commander.js |
| MCP | Model Context Protocol SDK |
