---
tags:
    - guide
    - architecture
description: Architecture section index — backend, frontend, and database design patterns
---

# Architecture

Fubbik is a Rust API and CLI with a TanStack Start web app. Rust owns the live HTTP routes, authentication, data access, and PostgreSQL migrations. TypeScript packages provide the web app, generated client, tooling, and a retired backend reference.

## Project Structure

```
fubbik/
├── apps/
│   ├── web/         # Frontend (TanStack Start, React)
│   ├── server/      # Retired Elysia reference
│   ├── cli/         # TypeScript reference for local discovery
│   └── vscode/      # VS Code extension
├── packages/
│   ├── api/         # Retired Elysia reference
│   ├── auth/        # Retired Better Auth reference
│   ├── client/      # Client generated from Rust OpenAPI
│   ├── config/      # Shared TypeScript config
│   ├── db/          # Legacy schema reference and typed seed adapter
│   ├── env/         # Environment validation
│   └── mcp/         # MCP server for AI agents
├── crates/
│   ├── fubbik-api/  # Axum HTTP routes and domain workflows
│   ├── fubbik-db/   # SQLx repositories and migrations
│   ├── fubbik-core/ # Shared Rust domain types and errors
│   └── fubbik/      # Live server binary
└── docs/guide/      # User and architecture documentation
```

## In This Section

- [Backend](./backend/) — live Rust repository, service, and route patterns
- [Frontend](./frontend/) — TanStack Start, React, and UI patterns
- [Database](./database/) — PostgreSQL schema and extensions
