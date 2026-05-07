---
tags:
  - guide
  - architecture
description: Architecture section index — backend, frontend, and database design patterns
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

## In This Section

- [Backend](./backend/) — repository, service, and route patterns
- [Frontend](./frontend/) — TanStack Start, React, and UI patterns
- [Database](./database/) — PostgreSQL schema and extensions
